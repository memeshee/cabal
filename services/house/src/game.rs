//! The table runner: deals a table on-chain, plays every house wallet from the seeded plan, serves the public view,
//! reveals at the end, and sweeps up.
//!
//! Secrecy: the plan, the salt and the side-wallet ids exist only inside [`Table`]. Everything the API serves is
//! built from chain data and off-chain chat, so there is no path by which the cabal can leak before the reveal.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use near_api::types::AccountId;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::time::{sleep, sleep_until, Instant as TInstant};

use crate::{
    chain::{mnear, Chain},
    config::Config,
    dossier::{dossiers, solve, Dossier, EntryV, SeatV, Verdict},
    kols::{build_prompt, claim, clean_question, scripted, Facts, Intent, Public, Tone, PERSONAS, QUESTIONS},
    llm::{Llm, Via},
    model::{ActionResult, ChatLine, Phase, RoundV},
    plan::{self, Act, Plan, Tier, Who, N_CROWD, N_KOLS, N_SIDES, PLAY_SECS, REVEAL_SECS},
};

/// Mirrors the contract's `MAX_TEXT`.
const MAX_CALL_TEXT: usize = 200;
/// Prepaid gas per call, in Tgas. NEAR reserves `gas x gas price` up front and refunds what is unused, and on testnet
/// the price has been high enough that 40 Tgas reserves ~0.04 NEAR. These calls burn only a few Tgas, so reserve little:
/// a small wallet must still be able to afford the call it needs next (a side wallet that just bought has to be able to sell).
const GAS_TX: u64 = 20;
const GAS_CALL: u64 = 15;
const TICKERS: [&str; 10] = ["FROG", "TOAD", "MOON", "NEKO", "GIGA", "SMOL", "WOJAK", "DEGEN", "PEPE2", "BONKER"];

// Warm-up targets come from the plan, which proves in a test that no wallet is asked for more than it holds.
/// A wallet is refilled whenever it holds less than this share of its target.
const REFILL_BELOW_PCT: u128 = 85;
/// A wallet holding more than this share of its target (and 0.1 NEAR more in absolute terms) is trimmed back to target.
const TRIM_ABOVE_PCT: u128 = 150;

pub struct Wallets {
    pub kols: Vec<AccountId>,
    pub crowd: Vec<AccountId>,
    pub whale: AccountId,
    pub friend: AccountId,
    pub decoy: AccountId,
    pub relay: AccountId,
    pub exchange: AccountId,
    pub sides: Vec<AccountId>,
}

impl Wallets {
    fn id(&self, w: Who) -> AccountId {
        match w {
            Who::Kol(i) => self.kols[i].clone(),
            Who::Side(i) => self.sides[i].clone(),
            Who::Crowd(i) => self.crowd[i].clone(),
            Who::Whale => self.whale.clone(),
            Who::Friend => self.friend.clone(),
            Who::Decoy => self.decoy.clone(),
            Who::Relay => self.relay.clone(),
            Who::Exchange => self.exchange.clone(),
        }
    }
}

struct TState {
    phase: Phase,
    round: Option<u32>,
    rv: Option<RoundV>,
    feed: Vec<EntryV>,
    chat: Vec<ChatLine>,
    error: Option<String>,
    note: String,
    supply_hist: Vec<(Instant, u64)>,
    calls_ready: HashMap<usize, (String, Via)>,
    asked: HashMap<String, u32>,
    last_ask: HashMap<String, Instant>,
    opened_at: Option<Instant>,
    reveal_hash: Option<String>,
    ended: Option<Instant>,
    /// When each player last touched this table, for the "n at this table" count.
    seen: HashMap<String, Instant>,
    /// (player, seat, question) already asked, so the same question cannot be spammed.
    asked_q: std::collections::HashSet<(String, usize, u8)>,
}

pub struct Table {
    pub key: String,
    pub slot: usize,
    pub nonce: u64,
    pub seed: u64,
    pub tier: Tier,
    pub demo: bool,
    pub ticker: String,
    owner: String,
    plan: Plan,
    wallets: Wallets,
    st: Mutex<TState>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Saved {
    key: String,
    slot: usize,
    nonce: u64,
    seed: u64,
    #[serde(default = "default_tier")]
    tier: Tier,
    demo: bool,
    ticker: String,
    round: Option<u32>,
}

fn default_tier() -> Tier {
    Tier::Rookie
}

#[derive(Default)]
struct Limits {
    tables: HashMap<String, Vec<Instant>>,
    guests: HashMap<String, Vec<Instant>>,
    refills: HashMap<String, Instant>,
    day: u64,
    tables_today: usize,
    guests_today: usize,
}

pub struct Game {
    pub cfg: Config,
    pub chain: Arc<Chain>,
    pub llm: Llm,
    master: [u8; 32],
    tables: Mutex<HashMap<String, Arc<Table>>>,
    slot_busy: Mutex<Vec<bool>>,
    limits: Mutex<Limits>,
    store: PathBuf,
    pub started: Instant,
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn hexs(b: &[u8]) -> String {
    hex::encode(b)
}

fn prune_window(v: &mut Vec<Instant>, window: Duration) {
    v.retain(|t| t.elapsed() < window);
}

#[derive(Serialize)]
pub struct SeatPublic {
    pub handle: String,
    pub wallet: String,
    pub tagline: &'static str,
    pub color: &'static str,
}

#[derive(Serialize)]
pub struct RevealV {
    pub cabal_seat: u8,
    pub cabal_handle: String,
    pub sides: Vec<String>,
    pub salt: String,
    pub preimage: String,
    pub commitment: String,
    pub recomputed: String,
    pub matches: bool,
    pub hash: Option<String>,
}

#[derive(Serialize)]
pub struct TableView {
    pub key: String,
    pub phase: Phase,
    pub round: Option<u32>,
    pub ticker: String,
    pub tier: Tier,
    pub players: usize,
    pub demo: bool,
    pub note: String,
    pub error: Option<String>,
    pub seats: Vec<SeatPublic>,
    pub labels: HashMap<String, String>,
    pub round_view: Option<RoundV>,
    pub feed: Vec<EntryV>,
    pub feed_total: usize,
    pub chat: Vec<ChatLine>,
    pub chat_total: usize,
    pub dossiers: Vec<Dossier>,
    pub verdict: Option<Verdict>,
    pub reveal: Option<RevealV>,
    pub elapsed_ms: Option<u64>,
    pub play_ms: u64,
    pub reveal_ms: u64,
    pub contract: String,
    pub explorer: String,
}

impl Game {
    pub fn new(cfg: Config) -> Result<Arc<Self>> {
        let chain = Arc::new(Chain::new(&cfg)?);
        let master: [u8; 32] = Sha256::digest(
            format!("cabal/master/v1|{}|{}", std::env::var("MASTER_SECRET").unwrap_or_default(), cfg.house_secret).as_bytes(),
        )
        .into();
        std::fs::create_dir_all(&cfg.data_dir).ok();
        let store = cfg.data_dir.join("active.json");
        let slots = cfg.slots;
        Ok(Arc::new(Self {
            llm: Llm::new(&cfg),
            chain,
            master,
            tables: Default::default(),
            slot_busy: Mutex::new(vec![false; slots]),
            limits: Default::default(),
            store,
            started: Instant::now(),
            cfg,
        }))
    }

    // ───────────────────────── identities ─────────────────────────

    fn w(&self, label: &str) -> AccountId {
        self.chain.wallet(&self.master, label).expect("wallet derivation")
    }

    /// Debug aid for `house derive <label>` (requires CABAL_DEBUG). Testnet throwaway keys only.
    pub fn debug_wallet(&self, label: &str) -> String {
        use sha2::Digest as _;
        let id = self.w(label);
        let mut h = Sha256::new();
        h.update(b"cabal/wallet/v1|");
        h.update(self.master);
        h.update(b"|");
        h.update(label.as_bytes());
        let seed: [u8; 32] = h.finalize().into();
        format!("{id} {}", self.chain.debug_secret(&seed))
    }

    fn slot_wallets(&self, slot: usize) -> Wallets {
        Wallets {
            kols: (0..N_KOLS).map(|i| self.w(&format!("slot{slot}/kol/{i}"))).collect(),
            crowd: (0..N_CROWD).map(|i| self.w(&format!("slot{slot}/crowd/{i}"))).collect(),
            whale: self.w(&format!("slot{slot}/whale")),
            friend: self.w(&format!("slot{slot}/friend")),
            decoy: self.w(&format!("slot{slot}/decoy")),
            relay: self.w(&format!("slot{slot}/relay")),
            exchange: self.w(&format!("slot{slot}/exchange")),
            sides: vec![],
        }
    }

    fn wallets_for(&self, slot: usize, nonce: u64) -> Wallets {
        let mut w = self.slot_wallets(slot);
        w.sides = (0..N_SIDES).map(|j| self.w(&format!("slot{slot}/side/{nonce}/{j}"))).collect();
        w
    }

    fn salt_for(&self, nonce: u64) -> String {
        let h = Sha256::digest([b"cabal/salt/v1|".as_slice(), &self.master, &nonce.to_le_bytes()].concat());
        hexs(&h[..12])
    }

    fn tag(&self, id: &str) -> String {
        let h = Sha256::digest([b"cabal/token/v1|".as_slice(), &self.master, id.as_bytes()].concat());
        hexs(&h[..8])
    }

    pub fn parse_token(&self, token: &str) -> Option<AccountId> {
        let (id, tag) = token.split_once('.')?;
        if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) || tag != self.tag(id) {
            return None;
        }
        Some(self.w(&format!("guest/{id}")))
    }

    // ───────────────────────── guests ─────────────────────────

    /// Create a throwaway testnet account for a visitor, or restore the one their token points at.
    pub async fn guest(&self, token: Option<&str>, ip: &str) -> Result<(String, AccountId, u128)> {
        let (token, acct, is_new) = match token.and_then(|t| self.parse_token(t).map(|a| (t.to_string(), a))) {
            Some((t, a)) => (t, a, false),
            None => {
                let id = hexs(&rand::random::<[u8; 16]>());
                let t = format!("{id}.{}", self.tag(&id));
                let a = self.parse_token(&t).ok_or_else(|| anyhow!("token"))?;
                (t, a, true)
            }
        };
        let bal = self.chain.balance(&acct).await;
        let low = bal < mnear(40);
        if low {
            {
                let mut l = self.limits.lock().unwrap();
                let day = now_ms() / 86_400_000;
                if l.day != day {
                    l.day = day;
                    l.tables_today = 0;
                    l.guests_today = 0;
                }
                if is_new {
                    let v = l.guests.entry(ip.to_string()).or_default();
                    prune_window(v, Duration::from_secs(3600));
                    if v.len() >= self.cfg.max_guests_per_ip_hour {
                        return Err(anyhow!("too many new players from your network this hour; try again later"));
                    }
                    v.push(Instant::now());
                } else if l.refills.get(acct.as_str()).map(|t| t.elapsed() < Duration::from_secs(3600)).unwrap_or(false) {
                    return Err(anyhow!("your play money was topped up recently; give it a while"));
                }
                if l.guests_today >= self.cfg.max_guests_per_day {
                    return Err(anyhow!("today's play-money budget is used up; come back tomorrow"));
                }
                l.guests_today += 1;
                l.refills.insert(acct.to_string(), Instant::now());
            }
            self.chain.transfer(&self.chain.house, &acct, mnear(self.cfg.guest_grant_mnear)).await?;
        }
        let bal = self.chain.balance(&acct).await;
        Ok((token, acct, bal))
    }

    // ───────────────────────── tables ─────────────────────────

    pub fn table(&self, key: &str) -> Option<Arc<Table>> {
        self.tables.lock().unwrap().get(key).cloned()
    }

    pub fn free_slots(&self) -> usize {
        self.slot_busy.lock().unwrap().iter().filter(|b| !**b).count()
    }

    /// Start dealing a table. Returns immediately; the table is `dealing` until it is open on-chain.
    pub fn start_table(self: &Arc<Self>, tier: Tier, demo: bool, ip: &str) -> Result<Arc<Table>> {
        {
            let mut l = self.limits.lock().unwrap();
            let day = now_ms() / 86_400_000;
            if l.day != day {
                l.day = day;
                l.tables_today = 0;
                l.guests_today = 0;
            }
            if l.tables_today >= self.cfg.max_tables_per_day {
                return Err(anyhow!("the house has dealt its limit for today; come back tomorrow"));
            }
            let v = l.tables.entry(ip.to_string()).or_default();
            prune_window(v, Duration::from_secs(3600));
            if v.len() >= self.cfg.max_tables_per_ip_hour {
                return Err(anyhow!("that's a lot of tables from one network; give it a few minutes"));
            }
        }
        let slot = {
            let mut busy = self.slot_busy.lock().unwrap();
            match busy.iter().position(|b| !*b) {
                Some(i) => {
                    busy[i] = true;
                    i
                }
                None => return Err(anyhow!("all tables are in play right now; the next seat opens in about a minute")),
            }
        };
        {
            let mut l = self.limits.lock().unwrap();
            l.tables_today += 1;
            l.tables.entry(ip.to_string()).or_default().push(Instant::now());
        }
        let nonce = now_ms();
        let seed = if demo { plan::demo_seed(tier) } else { rand::random::<u64>() };
        let t = Arc::new(self.make_table(slot, nonce, seed, tier, demo, ip.to_string(), None));
        self.tables.lock().unwrap().insert(t.key.clone(), t.clone());
        let g = self.clone();
        let tt = t.clone();
        tokio::spawn(async move {
            if let Err(e) = g.run(&tt).await {
                tracing::error!(table = %tt.key, "table failed: {e:#}");
                let mut s = tt.st.lock().unwrap();
                if !s.phase.is_final() {
                    s.phase = Phase::Failed;
                }
                s.error = Some("The house had trouble dealing this table. Nothing was lost; try again.".into());
            }
            g.release(&tt);
        });
        Ok(t)
    }

    fn make_table(&self, slot: usize, nonce: u64, seed: u64, tier: Tier, demo: bool, owner: String, ticker: Option<String>) -> Table {
        let mut r = StdRng::seed_from_u64(seed ^ 0x7ac3);
        let ticker = ticker.unwrap_or_else(|| TICKERS[r.gen_range(0..TICKERS.len())].to_string());
        Table {
            key: format!("{nonce:x}"),
            slot,
            nonce,
            seed,
            tier,
            demo,
            ticker,
            owner,
            plan: Plan::generate(seed, self.salt_for(nonce), tier),
            wallets: self.wallets_for(slot, nonce),
            st: Mutex::new(TState {
                phase: Phase::Dealing,
                round: None,
                rv: None,
                feed: vec![],
                chat: vec![],
                error: None,
                note: "Shuffling the wallets…".into(),
                supply_hist: vec![],
                calls_ready: HashMap::new(),
                asked: HashMap::new(),
                last_ask: HashMap::new(),
                opened_at: None,
                reveal_hash: None,
                ended: None,
                seen: HashMap::new(),
                asked_q: Default::default(),
            }),
        }
    }

    fn release(self: &Arc<Self>, t: &Arc<Table>) {
        t.st.lock().unwrap().ended = Some(Instant::now());
        self.slot_busy.lock().unwrap()[t.slot] = false;
        self.unsave(&t.key);
        let g = self.clone();
        let slot = t.slot;
        tokio::spawn(async move {
            let back = g.trim_slot(slot).await;
            if back > 0 {
                tracing::info!(slot, returned_near = back as f64 / 1e24, "trimmed surplus back to the house");
            }
            g.warm_slot(slot).await;
        });
    }

    fn save(&self, t: &Table) {
        let mut all: Vec<Saved> = std::fs::read_to_string(&self.store).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        all.retain(|s| s.key != t.key);
        all.push(Saved { key: t.key.clone(), slot: t.slot, nonce: t.nonce, seed: t.seed, tier: t.tier, demo: t.demo, ticker: t.ticker.clone(), round: t.st.lock().unwrap().round });
        let _ = std::fs::write(&self.store, serde_json::to_string(&all).unwrap_or_default());
    }

    fn unsave(&self, key: &str) {
        let mut all: Vec<Saved> = std::fs::read_to_string(&self.store).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        all.retain(|s| s.key != key);
        let _ = std::fs::write(&self.store, serde_json::to_string(&all).unwrap_or_default());
    }

    /// After a restart: finish tables that were mid-flight. Their wallets, plan and salt all re-derive.
    pub async fn recover(self: &Arc<Self>) {
        let saved: Vec<Saved> = std::fs::read_to_string(&self.store).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        for s in saved {
            let Some(round) = s.round else {
                self.unsave(&s.key);
                continue;
            };
            if s.slot >= self.cfg.slots {
                continue;
            }
            self.slot_busy.lock().unwrap()[s.slot] = true;
            let t = Arc::new(self.make_table(s.slot, s.nonce, s.seed, s.tier, s.demo, "recovered".into(), Some(s.ticker.clone())));
            {
                let mut st = t.st.lock().unwrap();
                st.round = Some(round);
                st.phase = Phase::Revealing;
                st.note = "The house restarted. Finishing the table.".into();
            }
            self.tables.lock().unwrap().insert(t.key.clone(), t.clone());
            let g = self.clone();
            tokio::spawn(async move {
                g.poll_once(&t).await;
                g.settle(&t).await;
                g.release(&t);
            });
        }
    }

    // ───────────────────────── warm-up and upkeep ─────────────────────────

    /// Top up every persistent wallet in a slot so a table never stalls on gas or buying power.
    pub async fn warm_slot(&self, slot: usize) {
        let w = self.slot_wallets(slot);
        let mut wanted: Vec<(AccountId, u64)> = vec![];
        wanted.extend(w.kols.iter().map(|a| (a.clone(), plan::BUDGET_KOL)));
        wanted.extend(w.crowd.iter().map(|a| (a.clone(), plan::BUDGET_CROWD)));
        wanted.push((w.exchange.clone(), plan::BUDGET_EXCHANGE));
        wanted.push((w.whale.clone(), plan::BUDGET_WHALE));
        wanted.push((w.friend.clone(), plan::BUDGET_FRIEND));
        wanted.push((w.decoy.clone(), plan::BUDGET_DECOY));
        wanted.push((w.relay.clone(), plan::BUDGET_RELAY));
        let house_bal = self.chain.balance(&self.chain.house).await;
        for (acct, target) in wanted {
            let bal = self.chain.balance(&acct).await;
            if bal < mnear(target) * REFILL_BELOW_PCT / 100 {
                let need = mnear(target) - bal;
                if house_bal < need + mnear(2_000) {
                    tracing::warn!("house balance is low; skipping warm-up of {acct}");
                    continue;
                }
                if let Err(e) = self.chain.transfer(&self.chain.house, &acct, need).await {
                    tracing::warn!("warm-up transfer to {acct} failed: {e}");
                }
            }
        }
    }

    /// Return surplus to the house. Copy-traders sell into a rising curve, so over many tables their wallets net-gain
    /// NEAR; without this the house slowly drains into wallets that already hold more than they will ever spend.
    pub async fn trim_slot(&self, slot: usize) -> u128 {
        let w = self.slot_wallets(slot);
        let mut all: Vec<(AccountId, u64)> = vec![];
        all.extend(w.kols.iter().map(|a| (a.clone(), plan::BUDGET_KOL)));
        all.extend(w.crowd.iter().map(|a| (a.clone(), plan::BUDGET_CROWD)));
        all.push((w.exchange.clone(), plan::BUDGET_EXCHANGE));
        all.push((w.whale.clone(), plan::BUDGET_WHALE));
        all.push((w.friend.clone(), plan::BUDGET_FRIEND));
        all.push((w.decoy.clone(), plan::BUDGET_DECOY));
        all.push((w.relay.clone(), plan::BUDGET_RELAY));
        let mut returned = 0u128;
        for (acct, target) in all {
            let bal = self.chain.balance(&acct).await;
            let keep = mnear(target) * TRIM_ABOVE_PCT / 100;
            if bal > keep + mnear(100) {
                let give = bal - mnear(target);
                match self.chain.transfer(&acct, &self.chain.house, give).await {
                    Ok(_) => returned += give,
                    Err(e) => tracing::warn!("trim from {acct} failed: {e}"),
                }
            }
        }
        returned
    }

    /// Keep the contract's balance above its storage cost, and free storage held by old rounds.
    pub async fn janitor(self: Arc<Self>) {
        loop {
            sleep(Duration::from_secs(90)).await;
            // Finished tables stay readable for a while (they live on the chain anyway), then leave memory.
            self.tables.lock().unwrap().retain(|_, t| t.st.lock().unwrap().ended.map(|e| e.elapsed() < Duration::from_secs(20 * 60)).unwrap_or(true));
            let c = &self.chain.contract;
            if self.chain.balance(c).await < mnear(7_000) {
                let _ = self.chain.transfer(&self.chain.house, c, mnear(3_000)).await;
            }
            if let Ok(latest) = self.chain.view::<u32>("latest_round", json!({})).await {
                let keep = 30u32;
                if latest > keep {
                    let old = latest - keep;
                    if let Ok(Some(r)) = self.chain.view::<Option<RoundV>>("get_round", json!({ "round": old })).await {
                        if r.pruned < r.feed_len && r.status != "Open" {
                            let _ = self.chain.call(&self.chain.house, "prune", json!({ "round": old, "max": 100 }), 0, 80).await;
                        }
                    }
                }
            }
        }
    }

    // ───────────────────────── the run ─────────────────────────

    fn say(&self, t: &Table, who: &str, text: String, kind: &'static str, via: Option<Via>) {
        let mut s = t.st.lock().unwrap();
        let id = s.chat.len() as u32;
        s.chat.push(ChatLine { id, ts_ms: now_ms(), who: who.to_string(), text, kind, via, claim_q: None, claim_seat: None, claim_tag: None, claim_true: None });
    }

    fn set_note(&self, t: &Table, note: &str) {
        t.st.lock().unwrap().note = note.to_string();
    }

    async fn run(self: &Arc<Self>, t: &Arc<Table>) -> Result<()> {
        // 1. Deal: make sure every wallet can act, and the contract has room.
        self.set_note(t, "Topping up the wallets…");
        self.warm_slot(t.slot).await;
        let c = &self.chain.contract;
        if self.chain.balance(c).await < mnear(6_000) {
            self.chain.transfer(&self.chain.house, c, mnear(3_000)).await?;
        }

        // 2. Commit and open.
        self.set_note(t, "Committing the hidden roles on-chain…");
        let sides: Vec<String> = t.wallets.sides.iter().map(|a| a.to_string()).collect();
        let commitment = plan::commitment(t.plan.cabal, &sides, &t.plan.salt);
        let seats: Vec<Value> = (0..N_KOLS).map(|i| json!({ "handle": PERSONAS[i].handle, "wallet": t.wallets.kols[i] })).collect();
        let mut funders: Vec<String> = t.wallets.kols.iter().map(|a| a.to_string()).collect();
        // The order matters: the views read index N_KOLS as the exchange lane and N_KOLS + 1 as the relay.
        funders.push(t.wallets.exchange.to_string());
        funders.push(t.wallets.relay.to_string());
        let open = self
            .chain
            .call(
                &self.chain.house,
                "open_round",
                json!({ "ticker": t.ticker, "commitment": commitment, "seats": seats, "funders": funders, "play_secs": PLAY_SECS, "reveal_secs": REVEAL_SECS }),
                mnear(100),
                80,
            )
            .await?;
        if !open.ok {
            return Err(anyhow!("open_round refused: {:?}", open.error));
        }
        let round = open.ret.as_ref().and_then(|v| v.as_u64()).ok_or_else(|| anyhow!("no round id returned"))? as u32;
        let t0 = Instant::now();
        {
            let mut s = t.st.lock().unwrap();
            s.round = Some(round);
            s.phase = Phase::Playing;
            s.opened_at = Some(t0);
            s.note = String::new();
        }
        self.save(t);
        self.say(
            t,
            "house",
            format!("Table #{round} is open. The cabal's identity is committed on-chain as {}…{}. It cannot change. Find the cabal.", &commitment[..8], &commitment[56..]),
            "house",
            None,
        );

        // 3. Poll the chain, prefetch call lines, and run the plan.
        let g = self.clone();
        let tp = t.clone();
        tokio::spawn(async move {
            while !tp.st.lock().unwrap().phase.is_final() {
                g.poll_once(&tp).await;
                sleep(Duration::from_millis(1400)).await;
            }
        });
        for step in t.plan.steps.clone() {
            let g = self.clone();
            let tt = t.clone();
            tokio::spawn(async move {
                if let Act::Call { kol } = step.act {
                    // Write the line a few seconds early so the call lands on time.
                    sleep_until(TInstant::from_std(t0 + Duration::from_millis(step.at_ms.saturating_sub(7_000)))).await;
                    g.prefetch_call(&tt, kol).await;
                }
                sleep_until(TInstant::from_std(t0 + Duration::from_millis(step.at_ms))).await;
                g.exec(&tt, round, step.act).await;
            });
        }

        // 4. Close, reveal, settle.
        sleep_until(TInstant::from_std(t0 + Duration::from_millis(PLAY_SECS as u64 * 1000 + 1_500))).await;
        t.st.lock().unwrap().phase = Phase::Revealing;
        self.say(t, "house", "Market closed. The house must now reveal the committed roles. The contract will reject anything that doesn't match.".into(), "house", None);
        self.settle(t).await;
        Ok(())
    }

    /// Reveal (retrying until the chain agrees the market is closed), then settle and sweep.
    async fn settle(self: &Arc<Self>, t: &Arc<Table>) {
        let Some(round) = t.st.lock().unwrap().round else { return };
        let sides: Vec<String> = t.wallets.sides.iter().map(|a| a.to_string()).collect();
        let deadline = Instant::now() + Duration::from_secs(REVEAL_SECS as u64 + 5);
        let mut last_err = String::new();
        let mut revealed = false;
        while Instant::now() < deadline {
            self.poll_once(t).await;
            let already = t.st.lock().unwrap().rv.as_ref().map(|r| r.status != "Open").unwrap_or(false);
            if already {
                revealed = true;
                break;
            }
            match self
                .chain
                .call(&self.chain.house, "reveal", json!({ "round": round, "cabal_seat": t.plan.cabal, "sides": sides, "salt": t.plan.salt }), 0, 250)
                .await
            {
                Ok(o) if o.ok => {
                    t.st.lock().unwrap().reveal_hash = Some(o.hash.clone());
                    revealed = true;
                    break;
                }
                Ok(o) => {
                    last_err = o.error.unwrap_or_default();
                    if !last_err.contains("market still open") {
                        tracing::error!(round, "reveal refused: {last_err}");
                        break;
                    }
                }
                Err(e) => last_err = e.to_string(),
            }
            sleep(Duration::from_millis(900)).await;
        }
        if !revealed {
            // The house never revealed correctly: let the contract's own rule take over.
            tracing::error!(round, "reveal failed ({last_err}); waiting to void");
            self.say(t, "house", "The reveal did not go through. Once the window passes, the contract voids the table and pays the bond to accusers.".into(), "house", None);
            sleep(Duration::from_secs(REVEAL_SECS as u64 + 3)).await;
            let _ = self.chain.call(&self.chain.house, "void_unrevealed", json!({ "round": round }), 0, 100).await;
        }
        self.poll_once(t).await;
        {
            let mut s = t.st.lock().unwrap();
            s.phase = match s.rv.as_ref().map(|r| r.status.as_str()) {
                Some("Revealed") => Phase::Revealed,
                Some("Voided") => Phase::Voided,
                _ => Phase::Failed,
            };
        }
        let (phase, cabal, payout) = {
            let s = t.st.lock().unwrap();
            (s.phase, s.rv.as_ref().and_then(|r| r.cabal_seat), s.rv.as_ref().map(|r| r.payout.clone()).unwrap_or_default())
        };
        match (phase, cabal) {
            (Phase::Revealed, Some(c)) => {
                let caught = t.st.lock().unwrap().rv.as_ref().map(|r| r.accusers.iter().any(|a| a.seat == c)).unwrap_or(false);
                let msg = if caught {
                    format!("{} was the cabal. Their side wallets are burned and the pot is paid out to everyone who called it.", PERSONAS[c as usize].handle)
                } else {
                    format!("{} was the cabal. Nobody caught them, and they keep the bag.", PERSONAS[c as usize].handle)
                };
                let _ = payout;
                self.say(t, "house", msg, "house", None);
            }
            (Phase::Voided, _) => self.say(t, "house", "The house failed to reveal in time. The contract voided the table and paid its bond to the accusers.".into(), "house", None),
            _ => {}
        }
        // Sweep the fresh wallets back into the house.
        for s in &t.wallets.sides {
            let bal = self.chain.balance(s).await;
            if bal > mnear(40) {
                let _ = self.chain.transfer(s, &self.chain.house, bal - mnear(20)).await;
            }
        }
    }

    // ───────────────────────── polling ─────────────────────────

    pub async fn poll_once(&self, t: &Table) {
        let Some(round) = t.st.lock().unwrap().round else { return };
        let Ok(Some(rv)) = self.chain.view::<Option<RoundV>>("get_round", json!({ "round": round })).await else { return };
        let from = t.st.lock().unwrap().feed.len() as u32;
        let from = from.max(rv.pruned);
        let mut new: Vec<EntryV> = vec![];
        if rv.feed_len > from {
            if let Ok(v) = self.chain.view::<Vec<EntryV>>("get_feed", json!({ "round": round, "from": from, "limit": 200 })).await {
                new = v;
            }
        }
        let mut s = t.st.lock().unwrap();
        for e in new {
            if e.idx as usize == s.feed.len() {
                s.feed.push(e);
            }
        }
        s.supply_hist.push((Instant::now(), rv.supply));
        if s.supply_hist.len() > 60 {
            s.supply_hist.remove(0);
        }
        s.rv = Some(rv);
    }

    // ───────────────────────── acting ─────────────────────────

    fn public_facts(&self, t: &Table, kol: usize, about: Option<usize>) -> Public {
        let s = t.st.lock().unwrap();
        let cur = s.rv.as_ref().map(|r| r.supply).unwrap_or(0);
        let ref_supply = s.supply_hist.iter().find(|(at, _)| at.elapsed() < Duration::from_secs(25)).map(|(_, v)| *v).unwrap_or(cur);
        let pct = if ref_supply == 0 { 0 } else { ((cur as i64 - ref_supply as i64) * 100 / ref_supply as i64) as i32 };
        let me = t.wallets.kols[kol].as_str();
        let own: Vec<String> = s
            .feed
            .iter()
            .filter(|e| e.actor == me && (e.kind == "Buy" || e.kind == "Sell"))
            .map(|e| format!("{} {} chips", if e.kind == "Buy" { "bought" } else { "sold" }, e.chips))
            .collect();
        Public { ticker: t.ticker.clone(), price_change_pct: pct.clamp(-99, 999), own_trades: own, about_handle: about.map(|a| PERSONAS[a].handle.to_string()) }
    }

    async fn line(&self, t: &Table, kol: usize, intent: Intent, about: Option<usize>, question: Option<&str>) -> (String, Via) {
        let facts = self.public_facts(t, kol, about);
        let msgs = build_prompt(kol, intent, &facts, question);
        if let Some(r) = self.llm.line(&msgs).await {
            return r;
        }
        let mut rng = StdRng::seed_from_u64(now_ms() ^ (kol as u64) << 7);
        (scripted(kol, intent, &t.ticker, &mut rng), Via::Scripted)
    }

    async fn prefetch_call(&self, t: &Table, kol: usize) {
        let r = tokio::time::timeout(Duration::from_secs(6), self.line(t, kol, Intent::Call, None, None)).await;
        if let Ok(v) = r {
            t.st.lock().unwrap().calls_ready.insert(kol, v);
        }
    }

    /// Run one plan step, retrying transient failures. Each attempt re-reads chain state first, so a retry can never
    /// repeat something that already happened (a fund is skipped if the contract recorded it; a sell re-reads chips).
    async fn exec(&self, t: &Arc<Table>, round: u32, act: Act) {
        let mut attempt = 0;
        loop {
            match self.exec_once(t, round, act.clone()).await {
                Ok(()) => return,
                Err(e) if attempt < 3 && !matches!(act, Act::Call { .. } | Act::React { .. }) && !format!("{e:#}").contains("still pending") => {
                    attempt += 1;
                    tracing::warn!(round, ?act, attempt, "step failed, will retry: {e:#}");
                    sleep(Duration::from_millis(2_000 * attempt as u64)).await;
                }
                Err(e) => {
                    // Expected sometimes (a price that moved, a market that just closed); the table carries on.
                    tracing::warn!(round, ?act, "step failed: {e:#}");
                    return;
                }
            }
        }
    }

    async fn exec_once(&self, t: &Arc<Table>, round: u32, act: Act) -> Result<()> {
        let w = &t.wallets;
        {
            match act {
                Act::Fund { from, to, mnear: m } => {
                    let already: Option<String> = self.chain.view("funded_by_of", json!({ "round": round, "account_id": w.id(to) })).await.unwrap_or(None);
                    if already.is_some() {
                        return Ok(());
                    }
                    let o = self.chain.call(&w.id(from), "fund", json!({ "round": round, "to": w.id(to) }), mnear(m), GAS_TX).await?;
                    if !o.ok {
                        return Err(anyhow!("fund refused: {:?}", o.error));
                    }
                }
                Act::Buy { who, mnear: m } => {
                    let o = self.chain.call(&w.id(who), "buy", json!({ "round": round, "min_chips": 1 }), mnear(m), GAS_TX).await?;
                    if !o.ok {
                        return Err(anyhow!("buy refused: {:?}", o.error));
                    }
                }
                Act::Sell { who, pct } => {
                    let me = w.id(who);
                    let chips: u64 = self.chain.view("chips_of", json!({ "round": round, "account_id": me })).await?;
                    let n = chips * pct as u64 / 100;
                    if n > 0 {
                        let o = self.chain.call(&me, "sell", json!({ "round": round, "chips": n, "min_proceeds": "0" }), 0, GAS_TX).await?;
                        if !o.ok {
                            return Err(anyhow!("sell refused: {:?}", o.error));
                        }
                    }
                }
                Act::Call { kol } => {
                    let ready = t.st.lock().unwrap().calls_ready.remove(&kol);
                    let (text, via) = match ready {
                        Some(r) => r,
                        None => {
                            let mut rng = StdRng::seed_from_u64(t.seed ^ kol as u64);
                            (scripted(kol, Intent::Call, &t.ticker, &mut rng), Via::Scripted)
                        }
                    };
                    let text = if text.to_uppercase().contains(&format!("${}", t.ticker)) { text } else { format!("${} - {}", t.ticker, text) };
                    let text: String = text.chars().take(MAX_CALL_TEXT).collect();
                    let o = self.chain.call(&w.kols[kol], "post_call", json!({ "round": round, "seat": kol, "text": text }), 0, GAS_CALL).await?;
                    if !o.ok {
                        return Err(anyhow!("call refused: {:?}", o.error));
                    }
                    let _ = via;
                }
                Act::React { kol, about } => {
                    let mut rng = StdRng::seed_from_u64(t.seed ^ ((kol as u64) << 8) ^ about as u64);
                    let (intent, about_opt) = if kol == about {
                        (Intent::Hype, None)
                    } else {
                        let tone = match rng.gen_range(0..3) {
                            0 => Tone::Supportive,
                            1 => Tone::Skeptical,
                            _ => Tone::Joking,
                        };
                        (Intent::Comment { tone }, Some(about))
                    };
                    let (text, via) = self.line(t, kol, intent, about_opt, None).await;
                    self.say(t, PERSONAS[kol].handle, text, "kol", Some(via));
                }
            }
            Ok(())
        }
    }

    // ───────────────────────── player actions ─────────────────────────

    fn playing(&self, t: &Table) -> Result<u32, ActionResult> {
        let s = t.st.lock().unwrap();
        match (s.phase, s.round) {
            (Phase::Playing, Some(r)) => Ok(r),
            (Phase::Dealing, _) => Err(ActionResult::fail("The table is still being dealt.")),
            _ => Err(ActionResult::fail("The market is closed.")),
        }
    }

    fn outcome(o: crate::chain::TxOutcome, detail: Option<Value>) -> ActionResult {
        ActionResult { ok: o.ok, hash: Some(o.hash), explorer_url: Some(o.explorer_url), error: o.error, detail }
    }

    pub async fn player_buy(&self, t: &Table, who: &AccountId, m: u64) -> ActionResult {
        let round = match self.playing(t) { Ok(r) => r, Err(e) => return e };
        if !(5..=100).contains(&m) {
            return ActionResult::fail("Pick between 0.005 and 0.1 NEAR.");
        }
        match self.chain.call(who, "buy", json!({ "round": round, "min_chips": 1 }), mnear(m), GAS_TX).await {
            Ok(o) => {
                let d = o.ret.clone().map(|c| json!({ "chips": c }));
                Self::outcome(o, d)
            }
            Err(e) => ActionResult::fail(format!("Could not reach the chain: {e}")),
        }
    }

    pub async fn player_sell(&self, t: &Table, who: &AccountId, pct: u8) -> ActionResult {
        let round = {
            let s = t.st.lock().unwrap();
            match (s.phase, s.round) {
                (Phase::Playing | Phase::Revealing | Phase::Revealed | Phase::Voided, Some(r)) => r,
                _ => return ActionResult::fail("The table is still being dealt."),
            }
        };
        if !(1..=100).contains(&pct) {
            return ActionResult::fail("Pick 1 to 100 percent.");
        }
        let chips: u64 = match self.chain.view("chips_of", json!({ "round": round, "account_id": who })).await {
            Ok(c) => c,
            Err(e) => return ActionResult::fail(format!("Could not read your chips: {e}")),
        };
        let n = (chips * pct as u64 / 100).max(if chips > 0 { 1 } else { 0 });
        if n == 0 {
            return ActionResult::fail("You don't hold any chips on this table.");
        }
        match self.chain.call(who, "sell", json!({ "round": round, "chips": n, "min_proceeds": "0" }), 0, GAS_TX).await {
            Ok(o) => {
                let d = o.ret.clone().map(|v| json!({ "chips": n, "proceeds": v }));
                Self::outcome(o, d)
            }
            Err(e) => ActionResult::fail(format!("Could not reach the chain: {e}")),
        }
    }

    pub async fn player_expose(&self, t: &Table, who: &AccountId, seat: u8, m: u64) -> ActionResult {
        let round = match self.playing(t) { Ok(r) => r, Err(e) => return e };
        if (seat as usize) >= N_KOLS {
            return ActionResult::fail("No such seat.");
        }
        if !(10..=100).contains(&m) {
            return ActionResult::fail("Stake between 0.01 and 0.1 NEAR.");
        }
        match self.chain.call(who, "expose", json!({ "round": round, "seat": seat }), mnear(m), GAS_TX).await {
            Ok(o) => Self::outcome(o, None),
            Err(e) => ActionResult::fail(format!("Could not reach the chain: {e}")),
        }
    }

    /// Record that a player is at this table, for the presence count and the lobby.
    pub fn touch(&self, t: &Table, who: &AccountId) {
        let mut s = t.st.lock().unwrap();
        s.seen.insert(who.to_string(), Instant::now());
    }

    fn players_at(s: &TState) -> usize {
        s.seen.values().filter(|t| t.elapsed() < Duration::from_secs(30)).count()
    }

    /// A player asks a KOL something at the table. With `q` set it is one of the standard questions, answered by a
    /// claim the code decides; with only `text` it is free chat, answered in character from public facts.
    pub async fn player_ask(&self, t: &Table, who: &AccountId, seat: usize, q: Option<u8>, text: &str) -> Result<Vec<ChatLine>, String> {
        {
            let s = t.st.lock().unwrap();
            if s.phase != Phase::Playing {
                return Err("The market is closed.".into());
            }
        }
        if seat >= N_KOLS {
            return Err("No such seat.".into());
        }
        if let Some(qi) = q {
            if qi as usize >= QUESTIONS.len() {
                return Err("No such question.".into());
            }
        }
        let free = clean_question(text);
        if q.is_none() && free.chars().count() < 2 {
            return Err("Ask something.".into());
        }
        {
            let mut s = t.st.lock().unwrap();
            s.seen.insert(who.to_string(), Instant::now());
            let n = s.asked.entry(who.to_string()).or_default();
            if *n >= 14 {
                return Err("You've asked plenty. Read the board.".into());
            }
            if s.last_ask.get(who.as_str()).map(|x| x.elapsed() < Duration::from_millis(1500)).unwrap_or(false) {
                return Err("One question at a time.".into());
            }
            if let Some(qi) = q {
                if !s.asked_q.insert((who.to_string(), seat, qi)) {
                    return Err(format!("You already asked {} that.", PERSONAS[seat].handle));
                }
            }
            *s.asked.get_mut(who.as_str()).unwrap() += 1;
            s.last_ask.insert(who.to_string(), Instant::now());
        }
        let tag = &who.as_str()[..4];
        let asker = format!("you ({tag}) → {}", PERSONAS[seat].handle);
        match q {
            Some(qi) => {
                self.say(t, &asker, QUESTIONS[qi as usize].ask.to_string(), "you", None);
                let out = claim(qi, facts_for(&t.plan, seat));
                let (mut reply, mut via) = self.line(t, seat, Intent::Claim { q: qi }, None, Some(&out.text)).await;
                if via == Via::Scripted || !consistent(&out.text, &reply) {
                    reply = out.text.clone();
                    via = Via::Scripted;
                }
                let mut st = t.st.lock().unwrap();
                let id = st.chat.len() as u32;
                st.chat.push(ChatLine {
                    id,
                    ts_ms: now_ms(),
                    who: PERSONAS[seat].handle.to_string(),
                    text: reply,
                    kind: "kol",
                    via: Some(via),
                    claim_q: Some(qi),
                    claim_seat: Some(seat as u8),
                    claim_tag: Some(out.tag),
                    claim_true: None,
                });
            }
            None => {
                self.say(t, &asker, free.clone(), "you", None);
                let (reply, via) = self.line(t, seat, Intent::Answer, None, Some(&free)).await;
                self.say(t, PERSONAS[seat].handle, reply, "kol", Some(via));
            }
        }
        let s = t.st.lock().unwrap();
        let n = s.chat.len();
        Ok(s.chat[n.saturating_sub(2)..].to_vec())
    }

    // ───────────────────────── lobby ─────────────────────────

    /// Tables a visitor can still join.
    pub fn lobby(&self) -> Vec<LobbyRow> {
        let tables = self.tables.lock().unwrap();
        let mut rows: Vec<LobbyRow> = tables
            .values()
            .filter_map(|t| {
                let s = t.st.lock().unwrap();
                if s.phase != Phase::Playing {
                    return None;
                }
                let elapsed = s.opened_at.map(|o| o.elapsed().as_millis() as u64)?;
                if elapsed + 20_000 > PLAY_SECS as u64 * 1000 {
                    return None;
                }
                Some(LobbyRow { key: t.key.clone(), ticker: t.ticker.clone(), tier: t.tier, elapsed_ms: elapsed, players: Self::players_at(&s), demo: t.demo })
            })
            .collect();
        rows.sort_by_key(|r| r.elapsed_ms);
        rows
    }

    // ───────────────────────── views ─────────────────────────

    /// The exchange lane and the relay, as the contract's `funders` list orders them: callers, then exchange, then relay.
    fn lanes(rv: &RoundV) -> Vec<String> {
        rv.funders.get(N_KOLS).cloned().into_iter().collect()
    }

    fn labels(seats: &[SeatV], rv: &RoundV) -> HashMap<String, String> {
        let mut m = HashMap::new();
        for s in seats {
            m.insert(s.wallet.clone(), s.handle.clone());
        }
        if let Some(x) = rv.funders.get(N_KOLS) {
            m.insert(x.clone(), "exchange".to_string());
        }
        if let Some(r) = rv.funders.get(N_KOLS + 1) {
            m.insert(r.clone(), "relay".to_string());
        }
        m
    }

    /// The tier of a past table, read off the chain: the contract does not store it.
    fn infer_tier(rv: &RoundV, feed: &[EntryV]) -> Tier {
        let relay = rv.funders.get(N_KOLS + 1);
        let exchange = rv.funders.get(N_KOLS);
        let seat_wallets: Vec<&String> = rv.seats.iter().map(|s| &s.wallet).collect();
        if feed.iter().any(|e| e.kind == "Fund" && e.other.as_ref() == relay) {
            Tier::Hop
        } else if feed.iter().any(|e| e.kind == "Fund" && e.other.as_ref() == exchange && seat_wallets.contains(&&e.actor)) {
            Tier::Ghost
        } else {
            Tier::Rookie
        }
    }

    /// Build the public view from chain data. Used for live tables (with chat) and for any past round (without).
    pub fn view_from(&self, c: ViewIn) -> TableView {
        let ViewIn { spoil, key, phase, ticker, tier, players, demo, note, error, rv, feed, chat, since_feed, since_chat, elapsed_ms, reveal_hash } = c;
        let seats_v: Vec<SeatV> = rv.as_ref().map(|r| r.seats.clone()).unwrap_or_default();
        let seats: Vec<SeatPublic> = seats_v
            .iter()
            .enumerate()
            .map(|(i, s)| SeatPublic { handle: s.handle.clone(), wallet: s.wallet.clone(), tagline: PERSONAS[i.min(3)].tagline, color: PERSONAS[i.min(3)].color })
            .collect();
        let labels = rv.as_ref().map(|r| Self::labels(&seats_v, r)).unwrap_or_default();
        let lanes = rv.as_ref().map(Self::lanes).unwrap_or_default();
        // While a table is live the API serves raw facts only. Derived timing, the trail's root and the solver's verdict
        // would hand the answer to anyone reading the JSON, so they appear after the reveal (or to the self-test).
        let open = !phase.is_final() && !spoil;
        let mut dos = dossiers(feed, &seats_v, &lanes);
        if open {
            for d in dos.iter_mut() {
                d.lead_ms = None;
                d.dumped_after_ms = None;
                d.funder_call_ms = None;
                d.root_seat = None;
            }
        }
        let verdict = if seats_v.is_empty() || open { None } else { solve(feed, &seats_v, &lanes) };
        let reveal = rv.as_ref().and_then(|r| {
            let (cs, salt) = (r.cabal_seat?, r.salt.clone()?);
            let pre = plan::preimage(cs as usize, &r.sides, &salt);
            let re = hex::encode(Sha256::digest(pre.as_bytes()));
            Some(RevealV {
                cabal_seat: cs,
                cabal_handle: r.seats.get(cs as usize).map(|s| s.handle.clone()).unwrap_or_default(),
                sides: r.sides.clone(),
                salt,
                matches: re == r.commitment,
                preimage: pre,
                commitment: r.commitment.clone(),
                recomputed: re,
                hash: reveal_hash,
            })
        });
        TableView {
            key: key.to_string(),
            phase,
            round: rv.as_ref().map(|r| r.id),
            ticker: ticker.to_string(),
            tier: tier.unwrap_or_else(|| rv.as_ref().map(|r| Self::infer_tier(r, feed)).unwrap_or(Tier::Rookie)),
            players,
            demo,
            note,
            error,
            seats,
            labels,
            round_view: rv,
            feed: feed.iter().skip(since_feed).cloned().collect(),
            feed_total: feed.len(),
            chat: chat.iter().skip(since_chat).cloned().collect(),
            chat_total: chat.len(),
            dossiers: dos,
            verdict,
            reveal,
            elapsed_ms,
            play_ms: PLAY_SECS as u64 * 1000,
            reveal_ms: REVEAL_SECS as u64 * 1000,
            contract: self.chain.contract.to_string(),
            explorer: self.chain.explorer.clone(),
        }
    }

    pub fn table_view(&self, t: &Table, since_feed: usize, since_chat: usize) -> TableView {
        self.table_view_inner(t, since_feed, since_chat, false)
    }

    /// Includes the solver's verdict while the table is live. For the self-test only; never routed to the API.
    pub fn table_view_full(&self, t: &Table, since_feed: usize, since_chat: usize) -> TableView {
        self.table_view_inner(t, since_feed, since_chat, true)
    }

    fn table_view_inner(&self, t: &Table, since_feed: usize, since_chat: usize, spoil: bool) -> TableView {
        let s = t.st.lock().unwrap();
        let mut v = self.view_from(ViewIn {
            spoil,
            key: &t.key,
            phase: s.phase,
            ticker: &t.ticker,
            tier: Some(t.tier),
            players: Self::players_at(&s),
            demo: t.demo,
            note: s.note.clone(),
            error: s.error.clone(),
            rv: s.rv.clone(),
            feed: &s.feed,
            chat: &s.chat,
            since_feed,
            since_chat,
            elapsed_ms: s.opened_at.map(|o| o.elapsed().as_millis() as u64),
            reveal_hash: s.reveal_hash.clone(),
        });
        // The runner knows the round id as soon as the table opens, before the first chain poll lands.
        v.round = s.round.or(v.round);
        // Whether a claim was true is only told once the table is settled.
        if s.phase.is_final() {
            for l in v.chat.iter_mut() {
                if let (Some(q), Some(seat)) = (l.claim_q, l.claim_seat) {
                    l.claim_true = Some(claim(q, facts_for(&t.plan, seat as usize)).truthful);
                }
            }
        }
        v
    }

    /// Any round on the contract, rebuilt purely from chain data.
    pub async fn round_view(&self, id: u32) -> Result<TableView> {
        let rv: Option<RoundV> = self.chain.view("get_round", json!({ "round": id })).await?;
        let rv = rv.ok_or_else(|| anyhow!("no such table"))?;
        let mut feed: Vec<EntryV> = vec![];
        let mut from = rv.pruned;
        while from < rv.feed_len {
            let page: Vec<EntryV> = self.chain.view("get_feed", json!({ "round": id, "from": from, "limit": 200 })).await?;
            if page.is_empty() {
                break;
            }
            from += page.len() as u32;
            feed.extend(page);
        }
        let phase = match rv.status.as_str() {
            "Revealed" => Phase::Revealed,
            "Voided" => Phase::Voided,
            _ => Phase::Playing,
        };
        let ticker = rv.ticker.clone();
        // A past table's feed may have been pruned from the front; indexes are then not contiguous from zero.
        Ok(self.view_from(ViewIn {
            spoil: false,
            key: &format!("r{id}"),
            phase,
            ticker: &ticker,
            tier: None,
            players: 0,
            demo: false,
            note: String::new(),
            error: None,
            rv: Some(rv),
            feed: &feed,
            chat: &[],
            since_feed: 0,
            since_chat: 0,
            elapsed_ms: None,
            reveal_hash: None,
        }))
    }
}

pub struct ViewIn<'a> {
    pub spoil: bool,
    pub key: &'a str,
    pub phase: Phase,
    pub ticker: &'a str,
    /// `None` for a past round: the tier is then read off the chain.
    pub tier: Option<Tier>,
    pub players: usize,
    pub demo: bool,
    pub note: String,
    pub error: Option<String>,
    pub rv: Option<RoundV>,
    pub feed: &'a [EntryV],
    pub chat: &'a [ChatLine],
    pub since_feed: usize,
    pub since_chat: usize,
    pub elapsed_ms: Option<u64>,
    pub reveal_hash: Option<String>,
}

#[derive(Serialize)]
pub struct LobbyRow {
    pub key: String,
    pub ticker: String,
    pub tier: Tier,
    pub elapsed_ms: u64,
    pub players: usize,
    pub demo: bool,
}

/// What a caller's answers are checked against: the plan, and nothing a model said.
fn facts_for(p: &Plan, seat: usize) -> Facts {
    Facts { is_cabal: seat == p.cabal, funds_a_friend: seat == p.friend_of, buys_early_openly: p.conviction == Some(seat) }
}

/// A model's rephrasing must keep the claim's polarity. If it flips a "no" into a "yes", use the claim as written.
fn consistent(claim_text: &str, reply: &str) -> bool {
    let c = claim_text.trim().to_lowercase();
    let words: Vec<String> = reply.to_lowercase().split(|ch: char| !ch.is_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect();
    let has = |w: &str| words.iter().any(|x| x == w);
    if c.starts_with("no") {
        !has("yes") && !has("yep") && !has("sure")
    } else if c.starts_with("yes") || c.starts_with("one") {
        !words.first().map(|w| w == "no" || w == "nope" || w == "nah").unwrap_or(false)
    } else if c.starts_with("still holding") {
        !has("sold") || has("haven") || has("not") || has("never")
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rephrasing_that_flips_the_claim_is_rejected() {
        assert!(consistent("No. Just my own wallet.", "Nope, only this one, ser."));
        assert!(!consistent("No. Just my own wallet.", "Yes, I run a couple of alts."));
        assert!(consistent("Yes. A small position from my own wallet.", "Yeah, a small one from my main."));
        assert!(!consistent("Yes. A small position from my own wallet.", "No way, I bought after."));
        assert!(!consistent("One. A friend's wallet I topped up.", "Nah, nothing like that."));
    }

    #[test]
    fn claim_facts_come_from_the_plan_only() {
        let p = Plan::generate(20261169, "s".into(), Tier::Hop);
        for seat in 0..N_KOLS {
            let f = facts_for(&p, seat);
            assert_eq!(f.is_cabal, seat == p.cabal);
            assert_eq!(f.funds_a_friend, seat == p.friend_of);
            assert_eq!(f.buys_early_openly, p.conviction == Some(seat));
        }
        // the cabal's answer to "any other wallets?" is false whichever tier it plays
        for tier in Tier::ALL {
            let p = Plan::generate(5, "s".into(), tier);
            assert!(!claim(0, facts_for(&p, p.cabal)).truthful);
            assert!(!claim(1, facts_for(&p, p.cabal)).truthful);
            for seat in (0..N_KOLS).filter(|s| *s != p.cabal) {
                assert!(claim(0, facts_for(&p, seat)).truthful && claim(1, facts_for(&p, seat)).truthful);
            }
        }
    }
}
