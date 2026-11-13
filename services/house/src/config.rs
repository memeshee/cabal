use std::{env, fs, path::PathBuf};

use anyhow::{anyhow, Context, Result};

/// Everything the house needs from the environment. Secrets are never logged.
#[derive(Clone)]
pub struct Config {
    pub network: String,
    pub rpc_url: String,
    pub rpc_fallbacks: Vec<String>,
    pub explorer: String,
    pub contract: String,
    pub house: String,
    pub house_secret: String,
    pub port: u16,
    pub data_dir: PathBuf,
    /// How many tables run at once. Each table leases its own set of wallets.
    pub slots: usize,
    /// Test-NEAR the house gives a new guest account.
    pub guest_grant_mnear: u64,
    pub max_tables_per_ip_hour: usize,
    pub max_guests_per_ip_hour: usize,
    pub max_guests_per_day: usize,
    pub max_tables_per_day: usize,
    pub llm_daily_cap: u32,
    pub near_ai_key: Option<String>,
    pub near_ai_model: String,
    pub fallback_llm: Option<LlmEndpoint>,
    /// Fixed seed for the public demo table.
    pub demo_seed: u64,
}

#[derive(Clone)]
pub struct LlmEndpoint {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

fn var(k: &str) -> Option<String> {
    env::var(k).ok().filter(|v| !v.trim().is_empty())
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let network = var("NEAR_NETWORK").unwrap_or_else(|| "testnet".into());
        if network != "testnet" {
            return Err(anyhow!("CABAL only runs on testnet (NEAR_NETWORK={network})"));
        }
        let contract = var("CABAL_CONTRACT").context("CABAL_CONTRACT is required")?;
        let house = var("HOUSE_ACCOUNT").context("HOUSE_ACCOUNT is required")?;
        let house_secret = match var("HOUSE_SECRET_KEY") {
            Some(k) => k,
            None => {
                let home = env::var("HOME").unwrap_or_default();
                let p = format!("{home}/.near-credentials/testnet/{house}.json");
                let raw = fs::read_to_string(&p).with_context(|| format!("HOUSE_SECRET_KEY unset and {p} unreadable"))?;
                let v: serde_json::Value = serde_json::from_str(&raw)?;
                v["private_key"].as_str().context("private_key missing in credentials file")?.to_string()
            }
        };
        let rpc_url = var("NEAR_RPC").unwrap_or_else(|| "https://test.rpc.fastnear.com".into());
        let rpc_fallbacks = match var("NEAR_RPC_FALLBACKS") {
            Some(s) => s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
            None => vec!["https://rpc.testnet.near.org".into(), "https://near-testnet.drpc.org".into()],
        };
        let fallback_llm = match (var("LLM_BASE_URL"), var("LLM_API_KEY"), var("LLM_MODEL")) {
            (Some(base_url), Some(api_key), Some(model)) => Some(LlmEndpoint { base_url, api_key, model }),
            _ => None,
        };
        Ok(Self {
            network,
            rpc_url,
            rpc_fallbacks,
            explorer: var("EXPLORER_BASE").unwrap_or_else(|| "https://testnet.nearblocks.io".into()),
            contract,
            house,
            house_secret,
            port: var("PORT").and_then(|p| p.parse().ok()).unwrap_or(8080),
            data_dir: PathBuf::from(var("DATA_DIR").unwrap_or_else(|| "./data".into())),
            slots: var("SLOTS").and_then(|p| p.parse().ok()).unwrap_or(3),
            guest_grant_mnear: var("GUEST_GRANT_MNEAR").and_then(|p| p.parse().ok()).unwrap_or(350),
            max_tables_per_ip_hour: var("MAX_TABLES_PER_IP_HOUR").and_then(|p| p.parse().ok()).unwrap_or(8),
            max_guests_per_ip_hour: var("MAX_GUESTS_PER_IP_HOUR").and_then(|p| p.parse().ok()).unwrap_or(4),
            max_guests_per_day: var("MAX_GUESTS_PER_DAY").and_then(|p| p.parse().ok()).unwrap_or(250),
            max_tables_per_day: var("MAX_TABLES_PER_DAY").and_then(|p| p.parse().ok()).unwrap_or(150),
            llm_daily_cap: var("LLM_DAILY_CAP").and_then(|p| p.parse().ok()).unwrap_or(4000),
            near_ai_key: var("NEAR_AI_API_KEY"),
            near_ai_model: var("NEAR_AI_MODEL").unwrap_or_else(|| "Qwen/Qwen3-30B-A3B-Instruct-2507".into()),
            fallback_llm,
            demo_seed: var("DEMO_SEED").and_then(|p| p.parse().ok()).unwrap_or(20261169),
        })
    }
}
