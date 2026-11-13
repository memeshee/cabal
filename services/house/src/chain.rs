use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Result};
use near_api::{
    types::{AccountId, NearGas, NearToken, SecretKey},
    Contract, NetworkConfig, RPCEndpoint, Signer,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::config::Config;

/// Thin client over the NEAR RPC for the CABAL contract and for every wallet the house controls.
pub struct Chain {
    pub network: NetworkConfig,
    pub contract: AccountId,
    pub house: AccountId,
    pub explorer: String,
    signers: std::sync::RwLock<HashMap<AccountId, Arc<Signer>>>,
    /// One in-flight transaction per signing key keeps that key's nonces ordered.
    /// Different keys run in parallel, so one slow wallet never stalls another.
    locks: std::sync::Mutex<HashMap<AccountId, Arc<tokio::sync::Mutex<()>>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TxOutcome {
    pub ok: bool,
    pub hash: String,
    pub explorer_url: String,
    /// The contract's panic message when `ok` is false.
    pub error: Option<String>,
    pub ret: Option<Value>,
    pub gas_tgas: f64,
}

pub const YOCTO_PER_MNEAR: u128 = 1_000_000_000_000_000_000_000;

pub fn mnear(m: u64) -> u128 {
    m as u128 * YOCTO_PER_MNEAR
}

impl Chain {
    pub fn new(cfg: &Config) -> Result<Self> {
        let mut network = NetworkConfig::testnet();
        network.rpc_endpoints = std::iter::once(cfg.rpc_url.as_str())
            .chain(cfg.rpc_fallbacks.iter().map(String::as_str))
            .filter_map(|u| u.parse().ok())
            .map(|u| RPCEndpoint::new(u).with_retries(2))
            .collect();
        let chain = Self {
            network,
            contract: cfg.contract.parse()?,
            house: cfg.house.parse()?,
            explorer: cfg.explorer.clone(),
            signers: Default::default(),
            locks: Default::default(),
        };
        chain.add_signer(&cfg.house.parse()?, &cfg.house_secret)?;
        Ok(chain)
    }

    pub fn add_signer(&self, id: &AccountId, secret: &str) -> Result<()> {
        let sk: SecretKey = secret.parse().map_err(|e| anyhow!("bad secret key for {id}: {e:?}"))?;
        let signer = Signer::from_secret_key(sk).map_err(|e| anyhow!("signer for {id}: {e:?}"))?;
        self.signers.write().unwrap().insert(id.clone(), signer);
        Ok(())
    }

    /// Derive a wallet from the master secret and a label, register its signer, and return its implicit account id
    /// (the hex of its public key). The account does not exist on-chain until it is first funded.
    /// Deterministic: the same master and label always give the same wallet, so nothing needs to be stored.
    pub fn wallet(&self, master: &[u8; 32], label: &str) -> Result<AccountId> {
        use near_api::types::crypto::secret_key::ED25519SecretKey;
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(b"cabal/wallet/v1|");
        h.update(master);
        h.update(b"|");
        h.update(label.as_bytes());
        let seed: [u8; 32] = h.finalize().into();
        let sk = SecretKey::ED25519(ED25519SecretKey::from_secret_key(seed));
        let id: AccountId = hex::encode(sk.public_key().key_data()).parse()?;
        if !self.can_sign(&id) {
            self.add_signer(&id, &sk.to_string())?;
        }
        Ok(id)
    }

    pub fn debug_secret(&self, seed: &[u8; 32]) -> String {
        use near_api::types::crypto::secret_key::ED25519SecretKey;
        SecretKey::ED25519(ED25519SecretKey::from_secret_key(*seed)).to_string()
    }

    pub fn can_sign(&self, who: &AccountId) -> bool {
        self.signers.read().unwrap().contains_key(who)
    }

    fn signer(&self, who: &AccountId) -> Result<Arc<Signer>> {
        self.signers.read().unwrap().get(who).cloned().ok_or_else(|| anyhow!("no key for {who}"))
    }

    fn lock_for(&self, who: &AccountId) -> Arc<tokio::sync::Mutex<()>> {
        self.locks.lock().unwrap().entry(who.clone()).or_default().clone()
    }

    pub async fn view<T>(&self, method: &str, args: impl Serialize) -> Result<T>
    where
        T: DeserializeOwned + Send + Sync,
    {
        let data = Contract(self.contract.clone())
            .call_function(method, args)
            .read_only::<T>()
            // Optimistic reads see a transaction's effects about a second after it lands.
            .at(near_api::types::Reference::Optimistic)
            .fetch_from(&self.network)
            .await
            .map_err(|e| anyhow!("view {method}: {e}"))?;
        Ok(data.data)
    }

    /// Native NEAR balance in yoctoNEAR, or 0 if the account does not exist yet.
    pub async fn balance(&self, account: &AccountId) -> u128 {
        near_api::Tokens::account(account.clone())
            .near_balance()
            .fetch_from(&self.network)
            .await
            .map(|b| b.total.as_yoctonear())
            .unwrap_or(0)
    }

    /// Send native NEAR from a wallet we hold. Creates implicit accounts on first transfer.
    pub async fn transfer(&self, from: &AccountId, to: &AccountId, yocto: u128) -> Result<String> {
        let key = self.signer(from)?;
        let lock = self.lock_for(from);
        let _g = lock.lock().await;
        let mut attempt = 0;
        let res = loop {
            let r = near_api::Tokens::account(from.clone())
                .send_to(to.clone())
                .near(NearToken::from_yoctonear(yocto))
                .with_signer(key.clone())
                .wait_until(near_api::types::TxExecutionStatus::ExecutedOptimistic)
                .send_to(&self.network)
                .await;
            match r {
                Ok(v) => break v,
                Err(e) if attempt < 3 && format!("{e:?}").contains("InvalidTransaction") => {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(1200 * attempt)).await;
                }
                Err(e) => return Err(anyhow!("transfer: {e}")),
            }
        };
        let full = res.into_full().ok_or_else(|| anyhow!("transfer still pending"))?;
        let hash = full.outcomes().first().map(|o| o.transaction_hash.to_string()).unwrap_or_default();
        full.into_result().map_err(|e| anyhow!("transfer failed: {}", clean_error(&format!("{e:?}"))))?;
        Ok(hash)
    }

    /// Sign and send a CABAL contract call as `signer`. Contract failures come back as `ok: false` with the panic
    /// message instead of an `Err`, because refusals are an expected part of the game.
    pub async fn call(
        &self,
        signer: &AccountId,
        method: &str,
        args: Value,
        deposit_yocto: u128,
        gas_tgas: u64,
    ) -> Result<TxOutcome> {
        let key = self.signer(signer)?;
        let lock = self.lock_for(signer);
        let _g = lock.lock().await;
        // A wallet that cannot cover the deposit plus gas is rejected the same way as a stale nonce, but retrying never
        // helps. Check first so the failure is reported as what it is.
        // Gas is reserved at (gas x price) before execution; assume a pessimistic 1.5e9 yocto per gas unit.
        let need = deposit_yocto + gas_tgas as u128 * 1_000_000_000_000 * 1_500_000_000;
        let have = self.balance(signer).await;
        if have != 0 && have < need {
            return Ok(TxOutcome {
                ok: false,
                hash: String::new(),
                explorer_url: String::new(),
                error: Some(format!("wallet {signer} holds {have} yocto, needs about {need}")),
                ret: None,
                gas_tgas: 0.0,
            });
        }
        // An `InvalidTransaction` rejection happens before inclusion (stale nonce or block hash on a lagging RPC node),
        // so the transaction never executed and sending a freshly signed one is safe. Nothing else is retried.
        let mut attempt = 0;
        let res = loop {
            let r = Contract(self.contract.clone())
                .call_function(method, args.clone())
                .transaction()
                .deposit(NearToken::from_yoctonear(deposit_yocto))
                .gas(NearGas::from_tgas(gas_tgas))
                .with_signer(signer.clone(), key.clone())
                .wait_until(near_api::types::TxExecutionStatus::ExecutedOptimistic)
                .send_to(&self.network)
                .await;
            match r {
                Ok(v) => break v,
                Err(e) if attempt < 3 && format!("{e:?}").contains("InvalidTransaction") => {
                    attempt += 1;
                    let detail: String = format!("{e:?}").chars().take(700).collect();
                    tracing::warn!(%signer, method, attempt, %detail, "transaction rejected before inclusion; retrying with a fresh signature");
                    tokio::time::sleep(std::time::Duration::from_millis(1200 * attempt)).await;
                }
                Err(e) => return Err(anyhow!("send {method}: {e}")),
            }
        };
        let res = res.into_full().ok_or_else(|| anyhow!("transaction still pending"))?;
        let hash = res.outcomes().first().map(|o| o.transaction_hash.to_string()).unwrap_or_default();
        let gas = res.total_gas_burnt.as_gas() as f64 / 1e12;
        tracing::debug!(method, burned_tgas = gas, attached_tgas = gas_tgas, "call burned");
        let explorer_url = format!("{}/txns/{}", self.explorer, hash);
        match res.into_result() {
            Ok(s) => Ok(TxOutcome { ok: true, hash, explorer_url, error: None, ret: s.json::<Value>().ok(), gas_tgas: gas }),
            Err(f) => Ok(TxOutcome {
                ok: false,
                hash,
                explorer_url,
                error: Some(clean_error(&format!("{f:?}"))),
                ret: None,
                gas_tgas: gas,
            }),
        }
    }
}

/// Pull the human message out of a NEAR failure dump.
pub fn clean_error(raw: &str) -> String {
    for marker in ["Contract panicked: ", "Smart contract panicked: "] {
        if let Some(i) = raw.find(marker) {
            let rest = &raw[i + marker.len()..];
            let end = rest.find(['"', '\\']).unwrap_or(rest.len());
            return rest[..end].trim().to_string();
        }
    }
    let one_line: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    one_line.chars().take(240).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_contract_panic() {
        let raw = r#"Failure(ActionError { kind: FunctionCallError(ExecutionError("Smart contract panicked: market closed")) })"#;
        assert_eq!(clean_error(raw), "market closed");
    }

    #[test]
    fn derived_wallets_are_stable_distinct_and_implicit() {
        let cfg_secret = near_api::signer::generate_secret_key().unwrap().to_string();
        let mut net = NetworkConfig::testnet();
        net.rpc_endpoints = vec![];
        let chain = Chain {
            network: net,
            contract: "c.testnet".parse().unwrap(),
            house: "h.testnet".parse().unwrap(),
            explorer: String::new(),
            signers: Default::default(),
            locks: Default::default(),
        };
        let _ = cfg_secret;
        let master = [7u8; 32];
        let a = chain.wallet(&master, "slot/0/kol/0").unwrap();
        let b = chain.wallet(&master, "slot/0/kol/0").unwrap();
        let c = chain.wallet(&master, "slot/0/kol/1").unwrap();
        let d = chain.wallet(&[8u8; 32], "slot/0/kol/0").unwrap();
        assert_eq!(a, b);
        assert!(a != c && a != d);
        assert_eq!(a.as_str().len(), 64, "implicit account is 64 hex characters");
        assert!(chain.can_sign(&a));
    }

    #[test]
    fn mnear_is_exact() {
        assert_eq!(mnear(1), 1_000_000_000_000_000_000_000);
        assert_eq!(mnear(1000), 1_000_000_000_000_000_000_000_000);
    }
}
