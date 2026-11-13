//! The deal: a pure, seeded script of everything the house's wallets do during one table.
//!
//! Nothing in here touches the network. That keeps the game's fairness properties testable. Three tiers raise the
//! difficulty without changing the rule the player is hunting:
//!
//! * **Rookie**: the cabal pays for three fresh wallets directly. They buy before its call and dump after it.
//! * **Hop**: the cabal pays a relay wallet and the relay pays the three, so the string has two legs.
//! * **Ghost**: every caller tops up the exchange (the cabal first) and the exchange pays the three wallets, so the
//!   funding trail is drowned in noise and only the timing gives the cabal away.
//!
//! Hop and Ghost also add honest decoys: a caller's own wallet that buys before its call out of conviction, and an
//! exchange-funded wallet that buys before an honest call but never sells. The tests below prove, over hundreds of
//! seeds, that a solver reading only the public feed finds exactly one answer on every tier, and that the lazy
//! "who paid for the wallet" reading stops working on the harder ones.

use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const N_KOLS: usize = 4;
pub const N_SIDES: usize = 3;
pub const N_CROWD: usize = 8;
pub const PLAY_SECS: u32 = 130;
pub const REVEAL_SECS: u32 = 60;
/// What the cabal pays into each side wallet: enough for its buy (up to 0.075), then a sell and a sweep, with gas reserved.
pub const SIDE_FUND_MNEAR: u64 = 170;
/// What the cabal pays the relay in the Hop tier: the three side wallets plus gas.
pub const RELAY_FUND_MNEAR: u64 = 3 * SIDE_FUND_MNEAR + 40;
/// What the cabal tops the exchange up with in the Ghost tier, and what each honest caller adds as noise.
pub const EXCHANGE_TOPUP_MNEAR: u64 = 500;
pub const EXCHANGE_NOISE_MNEAR: u64 = 60;

// Balances (milliNEAR) the persistent wallets are topped up to before a table. `every_wallet_can_afford_its_plan`
// proves the plan never asks a wallet for more than this plus what it is paid during the table.
pub const BUDGET_KOL: u64 = 900;
pub const BUDGET_CROWD: u64 = 700;
pub const BUDGET_EXCHANGE: u64 = 400;
pub const BUDGET_WHALE: u64 = 200;
pub const BUDGET_FRIEND: u64 = 200;
pub const BUDGET_RELAY: u64 = 60;
pub const BUDGET_DECOY: u64 = 200;
/// Milli-NEAR kept back per transaction for gas when checking affordability.
const GAS_RESERVE_MNEAR: u64 = 5;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Rookie,
    Hop,
    Ghost,
}

impl Tier {
    pub const ALL: [Tier; 3] = [Tier::Rookie, Tier::Hop, Tier::Ghost];

    pub fn index(self) -> usize {
        match self {
            Tier::Rookie => 0,
            Tier::Hop => 1,
            Tier::Ghost => 2,
        }
    }

    pub fn parse(s: &str) -> Option<Tier> {
        match s.to_ascii_lowercase().as_str() {
            "rookie" => Some(Tier::Rookie),
            "hop" => Some(Tier::Hop),
            "ghost" => Some(Tier::Ghost),
            _ => None,
        }
    }

    pub fn has_decoys(self) -> bool {
        self != Tier::Rookie
    }
}

/// A wallet the house controls, by role. Concrete accounts are leased per table.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
pub enum Who {
    Kol(usize),
    Side(usize),
    Crowd(usize),
    /// Early buyer funded by the exchange wallet, not by any KOL. A red herring.
    Whale,
    /// A fresh wallet funded by an honest KOL that buys only after that KOL's call. A red herring.
    Friend,
    /// An exchange-funded wallet that buys shortly before an honest call and never sells. A red herring (Hop, Ghost).
    Decoy,
    /// The cabal's intermediate wallet in the Hop tier.
    Relay,
    Exchange,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Fund { from: Who, to: Who, mnear: u64 },
    Buy { who: Who, mnear: u64 },
    /// Sell `pct` percent of whatever the wallet holds.
    Sell { who: Who, pct: u8 },
    Call { kol: usize },
    /// A chat reaction (off-chain). `about` is the KOL whose call it responds to.
    React { kol: usize, about: usize },
}

#[derive(Clone, Debug)]
pub struct Step {
    pub at_ms: u64,
    pub act: Act,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub seed: u64,
    pub tier: Tier,
    pub cabal: usize,
    pub friend_of: usize,
    /// An honest caller whose own wallet buys before its call (Hop, Ghost).
    pub conviction: Option<usize>,
    /// The honest caller whose call the exchange-funded decoy buys ahead of (Hop, Ghost).
    pub decoy_for: Option<usize>,
    /// KOL seat index per call slot, in time order.
    pub call_order: Vec<usize>,
    pub call_ms: Vec<u64>,
    pub salt: String,
    pub steps: Vec<Step>,
}

const SLOT_MS: [u64; 4] = [28_000, 48_000, 68_000, 88_000];

impl Plan {
    pub fn generate(seed: u64, salt: String, tier: Tier) -> Plan {
        let mut rng = StdRng::seed_from_u64(seed);
        let cabal = rng.gen_range(0..N_KOLS);
        let mut others: Vec<usize> = (0..N_KOLS).filter(|k| *k != cabal).collect();
        others.shuffle(&mut rng);
        let friend_of = others[0];
        let conviction = tier.has_decoys().then(|| others[1]);
        let decoy_for = tier.has_decoys().then(|| others[2]);
        let mut call_order: Vec<usize> = (0..N_KOLS).collect();
        call_order.shuffle(&mut rng);
        let call_ms: Vec<u64> = SLOT_MS.iter().map(|b| b + rng.gen_range(0..3_000)).collect();

        let mut steps: Vec<Step> = Vec::new();
        let mut push = |at_ms: u64, act: Act| steps.push(Step { at_ms, act });

        // Ghost: every caller tops up the exchange. The cabal goes first; the contract records only the first funder.
        if tier == Tier::Ghost {
            push(2_000, Act::Fund { from: Who::Kol(cabal), to: Who::Exchange, mnear: EXCHANGE_TOPUP_MNEAR });
            for (n, &h) in others.iter().enumerate() {
                push(3_500 + n as u64 * 1_500, Act::Fund { from: Who::Kol(h), to: Who::Exchange, mnear: EXCHANGE_NOISE_MNEAR });
            }
        }

        // Early whale: funded by the exchange, buys before every call, sells late.
        let whale_fund = if tier == Tier::Ghost { 9_000 } else { 5_000 };
        push(whale_fund, Act::Fund { from: Who::Exchange, to: Who::Whale, mnear: 160 });
        push(whale_fund + 5_500 + rng.gen_range(0..1_500), Act::Buy { who: Who::Whale, mnear: 100 });
        push(120_000, Act::Sell { who: Who::Whale, pct: 100 });

        let mut crowd_next = 0usize;
        for (slot, &k) in call_order.iter().enumerate() {
            let t = call_ms[slot];
            push(t, Act::Call { kol: k });
            push(t + 2_500 + rng.gen_range(0..1_500), Act::Buy { who: Who::Kol(k), mnear: rng.gen_range(60..110) });
            // Two copy-traders follow every call, cabal or not.
            for n in 0..2u64 {
                let c = crowd_next % N_CROWD;
                crowd_next += 1;
                push(t + 4_000 + n * 2_400 + rng.gen_range(0..1_400), Act::Buy { who: Who::Crowd(c), mnear: rng.gen_range(130..210) });
            }
            // Chat: the caller hypes, another KOL reacts.
            push(t + 1_200, Act::React { kol: k, about: k });
            let other = *others_than(k).choose(&mut rng).unwrap();
            push(t + 7_000, Act::React { kol: other, about: k });

            if k == cabal {
                // How the money reaches the three wallets differs by tier; what they do with it does not.
                let base_fund = match tier {
                    Tier::Rookie => t.saturating_sub(26_000).max(4_000),
                    Tier::Hop => {
                        let relay_at = t.saturating_sub(34_000).max(3_000);
                        push(relay_at, Act::Fund { from: Who::Kol(k), to: Who::Relay, mnear: RELAY_FUND_MNEAR });
                        t.saturating_sub(26_000).max(relay_at + 5_000)
                    }
                    Tier::Ghost => t.saturating_sub(26_000).max(10_000),
                };
                let payer = match tier {
                    Tier::Rookie => Who::Kol(k),
                    Tier::Hop => Who::Relay,
                    Tier::Ghost => Who::Exchange,
                };
                for j in 0..N_SIDES {
                    let j64 = j as u64;
                    let fund_at = base_fund + j64 * 2_200;
                    push(fund_at, Act::Fund { from: payer, to: Who::Side(j), mnear: SIDE_FUND_MNEAR });
                    let buy_at = (t.saturating_sub(13_000) + j64 * 1_800 + rng.gen_range(0..800)).max(fund_at + 6_000);
                    push(buy_at, Act::Buy { who: Who::Side(j), mnear: rng.gen_range(55..75) });
                    push(t + 6_500 + j64 * 3_000 + rng.gen_range(0..800), Act::Sell { who: Who::Side(j), pct: 100 });
                }
            }
            if k == friend_of {
                push(t.saturating_sub(20_000).max(6_000), Act::Fund { from: Who::Kol(k), to: Who::Friend, mnear: 90 });
                push(t + 12_000, Act::Buy { who: Who::Friend, mnear: 60 });
            }
            if conviction == Some(k) {
                // An honest caller's own, public wallet buys a little ahead of its call. It never sells.
                push(t.saturating_sub(9_000), Act::Buy { who: Who::Kol(k), mnear: 70 });
            }
            if decoy_for == Some(k) {
                let fund_at = t.saturating_sub(24_000).max(12_000);
                push(fund_at, Act::Fund { from: Who::Exchange, to: Who::Decoy, mnear: 130 });
                push(t.saturating_sub(10_000).max(fund_at + 6_000), Act::Buy { who: Who::Decoy, mnear: 80 });
            }
        }
        // The crowd takes profit once the cabal is done dumping.
        for c in 0..N_CROWD {
            push(call_ms[3] + 16_000 + c as u64 * 1_400 + rng.gen_range(0..500), Act::Sell { who: Who::Crowd(c), pct: 100 });
        }
        drop(push);
        steps.sort_by_key(|s| s.at_ms);
        Plan { seed, tier, cabal, friend_of, conviction, decoy_for, call_order, call_ms, salt, steps }
    }

    /// Time of the cabal's own call.
    pub fn cabal_call_ms(&self) -> u64 {
        let slot = self.call_order.iter().position(|k| *k == self.cabal).unwrap();
        self.call_ms[slot]
    }
}

fn others_than(k: usize) -> Vec<usize> {
    (0..N_KOLS).filter(|x| *x != k).collect()
}

/// The string the contract hashes. Must match `cabal::preimage` exactly.
pub fn preimage(cabal_seat: usize, sides: &[String], salt: &str) -> String {
    format!("cabal={};sides={};salt={}", cabal_seat, sides.join(","), salt)
}

pub fn commitment(cabal_seat: usize, sides: &[String], salt: &str) -> String {
    hex::encode(Sha256::digest(preimage(cabal_seat, sides, salt).as_bytes()))
}

/// Deterministic seed from a short label, so "demo" always deals the same table.
pub fn seed_from_label(label: &str) -> u64 {
    let h = Sha256::digest(label.as_bytes());
    u64::from_le_bytes(h[..8].try_into().unwrap())
}

/// The pinned demo deal per tier. The README walks a judge through these, and a test pins what each one contains.
pub const DEMO_SEEDS: [u64; 3] = [20261169, 20261169, 20261169];

pub fn demo_seed(tier: Tier) -> u64 {
    DEMO_SEEDS[tier.index()]
}

#[cfg(test)]
pub mod sim {
    //! Replays a plan into the feed the contract would record, so the public solver can be tested over many seeds
    //! without a network.
    use super::*;
    use crate::dossier::{EntryV, SeatV};
    use std::collections::HashMap;

    pub fn id(w: Who) -> String {
        match w {
            Who::Kol(i) => format!("k{i}"),
            Who::Side(i) => format!("side{i}"),
            Who::Crowd(i) => format!("c{i}"),
            Who::Whale => "whale".into(),
            Who::Friend => "friend".into(),
            Who::Decoy => "decoy".into(),
            Who::Relay => "relay".into(),
            Who::Exchange => "exchange".into(),
        }
    }

    pub fn seats() -> Vec<SeatV> {
        (0..N_KOLS).map(|i| SeatV { handle: format!("@k{i}"), wallet: format!("k{i}") }).collect()
    }

    pub fn lanes() -> Vec<String> {
        vec!["exchange".into()]
    }

    pub fn feed(p: &Plan) -> Vec<EntryV> {
        let mut out: Vec<EntryV> = vec![];
        let mut supply = 0u64;
        let mut held: HashMap<Who, u64> = HashMap::new();
        let mn = |m: u64| m as u128 * 1_000_000_000_000_000_000_000u128;
        let mut push = |ts: u64, kind: &str, actor: String, other: Option<String>, chips: u64| {
            let idx = out.len() as u32;
            out.push(EntryV { idx, ts_ms: ts, block: idx as u64, kind: kind.into(), actor, other, chips, near: "0".into(), text: None });
        };
        for s in &p.steps {
            match &s.act {
                Act::Fund { from, to, .. } => push(s.at_ms, "Fund", id(*from), Some(id(*to)), 0),
                Act::Call { kol } => push(s.at_ms, "Call", id(Who::Kol(*kol)), None, 0),
                Act::Buy { who, mnear } => {
                    let (chips, _) = cabal::quote_buy_chips(supply, mn(*mnear));
                    supply += chips;
                    *held.entry(*who).or_default() += chips;
                    push(s.at_ms, "Buy", id(*who), None, chips);
                }
                Act::Sell { who, pct } => {
                    let h = held.get(who).copied().unwrap_or(0);
                    let n = h * *pct as u64 / 100;
                    if n > 0 {
                        supply -= n;
                        held.insert(*who, h - n);
                        push(s.at_ms, "Sell", id(*who), None, n);
                    }
                }
                Act::React { .. } => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dossier::{dossiers, solve_with};
    use std::collections::HashMap;

    fn plan(seed: u64, tier: Tier) -> Plan {
        Plan::generate(seed, "salt".into(), tier)
    }

    fn mn(m: u64) -> u128 {
        m as u128 * 1_000_000_000_000_000_000_000
    }

    fn sell_of(p: &Plan, w: Who) -> Option<&Step> {
        p.steps.iter().find(|s| matches!(&s.act, Act::Sell { who, .. } if *who == w))
    }
    fn buy_of(p: &Plan, w: Who) -> Option<&Step> {
        p.steps.iter().find(|s| matches!(&s.act, Act::Buy { who, .. } if *who == w))
    }
    fn fund_to(p: &Plan, w: Who) -> Option<&Step> {
        p.steps.iter().find(|s| matches!(&s.act, Act::Fund { to, .. } if *to == w))
    }

    #[test]
    fn deterministic_for_a_seed_and_tier() {
        for tier in Tier::ALL {
            let a = plan(7, tier);
            let b = plan(7, tier);
            assert_eq!(a.cabal, b.cabal);
            assert_eq!(a.steps.len(), b.steps.len());
            assert!(a.steps.iter().zip(&b.steps).all(|(x, y)| x.at_ms == y.at_ms && x.act == y.act));
        }
        assert_ne!(plan(7, Tier::Rookie).steps.len(), plan(7, Tier::Ghost).steps.len());
    }

    #[test]
    fn every_seat_can_be_the_cabal_and_call_in_any_slot() {
        for tier in Tier::ALL {
            let mut seat = [false; N_KOLS];
            let mut slot = [false; 4];
            for s in 0..300 {
                let p = plan(s, tier);
                seat[p.cabal] = true;
                slot[p.call_order.iter().position(|k| *k == p.cabal).unwrap()] = true;
            }
            assert!(seat.iter().all(|x| *x) && slot.iter().all(|x| *x), "{tier:?} must not be predictable");
        }
    }

    #[test]
    fn side_wallets_buy_before_the_cabal_call_then_dump_after_on_every_tier() {
        for tier in Tier::ALL {
            for seed in 0..200 {
                let p = plan(seed, tier);
                let call = p.cabal_call_ms();
                for j in 0..N_SIDES {
                    let fund = fund_to(&p, Who::Side(j)).unwrap();
                    let buy = buy_of(&p, Who::Side(j)).unwrap();
                    let sell = sell_of(&p, Who::Side(j)).unwrap();
                    assert!(fund.at_ms + 5_500 < buy.at_ms, "{tier:?} {seed}: funded before it buys");
                    assert!(buy.at_ms + 6_000 < call, "{tier:?} {seed}: buys clearly before the call ({} vs {call})", buy.at_ms);
                    assert!(call - buy.at_ms <= 32_000, "{tier:?} {seed}: the lead stays inside the window a player can see");
                    assert!(sell.at_ms > call + 3_000 && sell.at_ms < call + 40_000, "{tier:?} {seed}: dumps into the wave after the call");
                }
            }
        }
    }

    #[test]
    fn the_funding_trail_matches_the_tier() {
        for seed in 0..200 {
            // Rookie: the cabal pays the three wallets itself.
            let p = plan(seed, Tier::Rookie);
            for j in 0..N_SIDES {
                assert!(matches!(fund_to(&p, Who::Side(j)).unwrap().act, Act::Fund { from: Who::Kol(k), .. } if k == p.cabal));
            }
            assert!(fund_to(&p, Who::Relay).is_none() && fund_to(&p, Who::Decoy).is_none());

            // Hop: cabal -> relay -> sides, and the relay is paid before it pays.
            let p = plan(seed, Tier::Hop);
            let relay_paid = fund_to(&p, Who::Relay).unwrap();
            assert!(matches!(relay_paid.act, Act::Fund { from: Who::Kol(k), .. } if k == p.cabal));
            for j in 0..N_SIDES {
                let f = fund_to(&p, Who::Side(j)).unwrap();
                assert!(matches!(f.act, Act::Fund { from: Who::Relay, .. }));
                assert!(f.at_ms > relay_paid.at_ms + 4_000, "{seed}: relay has been paid before it pays");
            }

            // Ghost: the cabal tops the exchange up first, everyone else after, and the exchange pays the wallets.
            let p = plan(seed, Tier::Ghost);
            let tops: Vec<&Step> = p.steps.iter().filter(|s| matches!(&s.act, Act::Fund { to: Who::Exchange, .. })).collect();
            assert_eq!(tops.len(), N_KOLS, "all four callers leave a trail to the exchange");
            assert!(matches!(tops[0].act, Act::Fund { from: Who::Kol(k), .. } if k == p.cabal), "the cabal is the first to pay the exchange");
            assert!(tops.windows(2).all(|w| w[0].at_ms < w[1].at_ms));
            for j in 0..N_SIDES {
                let f = fund_to(&p, Who::Side(j)).unwrap();
                assert!(matches!(f.act, Act::Fund { from: Who::Exchange, .. }));
                assert!(f.at_ms > tops[3].at_ms, "wallets are paid after the noise is in place");
            }
        }
    }

    #[test]
    fn decoys_exist_only_on_the_harder_tiers_and_look_different_from_the_pattern() {
        for seed in 0..200 {
            let r = plan(seed, Tier::Rookie);
            assert!(r.conviction.is_none() && r.decoy_for.is_none());
            for tier in [Tier::Hop, Tier::Ghost] {
                let p = plan(seed, tier);
                let (c, d) = (p.conviction.unwrap(), p.decoy_for.unwrap());
                let distinct = [p.cabal, p.friend_of, c, d];
                for a in 0..4 {
                    for b in a + 1..4 {
                        assert_ne!(distinct[a], distinct[b], "{tier:?} {seed}: each role is a different caller");
                    }
                }
                // The conviction buyer's own wallet buys before its call and never sells.
                let slot = p.call_order.iter().position(|k| *k == c).unwrap();
                let cbuy = p.steps.iter().filter(|s| matches!(&s.act, Act::Buy { who: Who::Kol(k), .. } if *k == c)).map(|s| s.at_ms).min().unwrap();
                assert!(cbuy + 8_000 < p.call_ms[slot]);
                assert!(sell_of(&p, Who::Kol(c)).is_none());
                // The decoy buys ahead of an honest call, is funded by the exchange, and holds.
                let dslot = p.call_order.iter().position(|k| *k == d).unwrap();
                let dbuy = buy_of(&p, Who::Decoy).unwrap().at_ms;
                assert!(dbuy + 6_000 < p.call_ms[dslot] && dbuy + 15_000 > p.call_ms[dslot]);
                assert!(matches!(fund_to(&p, Who::Decoy).unwrap().act, Act::Fund { from: Who::Exchange, .. }));
                assert!(sell_of(&p, Who::Decoy).is_none());
            }
        }
    }

    #[test]
    fn honest_wallets_never_front_run_except_the_declared_conviction_buy() {
        for tier in Tier::ALL {
            for seed in 0..200 {
                let p = plan(seed, tier);
                for (slot, &k) in p.call_order.iter().enumerate() {
                    if k == p.cabal {
                        continue;
                    }
                    let t = p.call_ms[slot];
                    let early = p.steps.iter().any(|s| s.at_ms < t && matches!(&s.act, Act::Buy { who: Who::Kol(x), .. } if *x == k));
                    assert_eq!(early, p.conviction == Some(k), "{tier:?} {seed}: only the conviction caller buys early");
                }
                if let Some(f) = buy_of(&p, Who::Friend) {
                    let fslot = p.call_order.iter().position(|k| *k == p.friend_of).unwrap();
                    assert!(f.at_ms > p.call_ms[fslot], "the friend wallet buys only after its funder's call");
                }
            }
        }
    }

    #[test]
    fn everything_finishes_inside_the_market_window() {
        for tier in Tier::ALL {
            for seed in 0..200 {
                let p = plan(seed, tier);
                let last = p.steps.iter().map(|s| s.at_ms).max().unwrap();
                assert!(last < (PLAY_SECS as u64 - 3) * 1000, "{tier:?} {seed}: last step at {last}ms");
                assert!(p.steps.iter().map(|s| s.at_ms).min().unwrap() >= 1_000);
                assert!(p.steps.windows(2).all(|w| w[0].at_ms <= w[1].at_ms));
            }
        }
    }

    #[test]
    fn the_first_tell_lands_quickly() {
        // Pacing: the cabal's wallets are all bought in and visible long before half the market has passed.
        for tier in Tier::ALL {
            for seed in 0..200 {
                let p = plan(seed, tier);
                let first_call = p.call_ms[0];
                assert!(first_call <= 31_000);
                let last_side_sell = (0..N_SIDES).filter_map(|j| sell_of(&p, Who::Side(j))).map(|s| s.at_ms).max().unwrap();
                assert!(last_side_sell < 106_000, "{tier:?} {seed}: evidence is complete by {last_side_sell}ms");
            }
        }
    }

    #[test]
    fn exactly_one_call_per_kol() {
        let p = plan(3, Tier::Ghost);
        for k in 0..N_KOLS {
            assert_eq!(p.steps.iter().filter(|s| s.act == Act::Call { kol: k }).count(), 1);
        }
    }

    #[test]
    fn the_cabal_profits_on_the_real_curve_on_every_tier() {
        let mut worst = f64::MAX;
        let mut sum = 0.0;
        let mut n = 0.0;
        for tier in Tier::ALL {
            for seed in 0..60 {
                let p = plan(seed, tier);
                let mut supply = 0u64;
                let mut held: HashMap<String, u64> = HashMap::new();
                let (mut spent, mut got) = (0u128, 0u128);
                let key = |w: &Who| format!("{w:?}");
                for s in &p.steps {
                    match &s.act {
                        Act::Buy { who, mnear } => {
                            let (chips, cost) = cabal::quote_buy_chips(supply, mn(*mnear));
                            supply += chips;
                            *held.entry(key(who)).or_default() += chips;
                            if matches!(who, Who::Side(_)) {
                                spent += cost;
                            }
                        }
                        Act::Sell { who, pct } => {
                            let h = held.get(&key(who)).copied().unwrap_or(0);
                            let k = h * *pct as u64 / 100;
                            if k > 0 {
                                let v = cabal::curve_value(supply - k, supply);
                                supply -= k;
                                held.insert(key(who), h - k);
                                if matches!(who, Who::Side(_)) {
                                    got += v;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                let ratio = got as f64 / spent as f64;
                worst = worst.min(ratio);
                sum += ratio;
                n += 1.0;
            }
        }
        assert!(worst > 1.03, "side wallets must profit in every deal, worst was {worst:.2}x");
        assert!(sum / n > 1.2, "average side-wallet return should be a clear motive, was {:.2}x", sum / n);
    }

    #[test]
    fn side_wallets_can_afford_their_sell_after_their_buy() {
        // Regression: NEAR reserves gas x price up front; wallets funded too thinly could buy but not sell.
        let reserve_for_sell = 20u128 * 1_000_000_000_000 * 1_500_000_000;
        for seed in 0..200 {
            let p = plan(seed, Tier::Rookie);
            let buy = p.steps.iter().filter_map(|s| match &s.act { Act::Buy { who: Who::Side(_), mnear } => Some(*mnear), _ => None }).max().unwrap();
            let left = (SIDE_FUND_MNEAR - buy) as u128 * 1_000_000_000_000_000_000_000;
            assert!(left > reserve_for_sell * 2, "seed {seed}: {left} left after the buy, sell reserves {reserve_for_sell}");
        }
    }

    #[test]
    fn every_wallet_can_afford_its_plan() {
        let start = |w: &Who| -> i64 {
            (match w {
                Who::Kol(_) => BUDGET_KOL,
                Who::Crowd(_) => BUDGET_CROWD,
                Who::Exchange => BUDGET_EXCHANGE,
                Who::Whale => BUDGET_WHALE,
                Who::Friend => BUDGET_FRIEND,
                Who::Relay => BUDGET_RELAY,
                Who::Decoy => BUDGET_DECOY,
                Who::Side(_) => 0,
            }) as i64
        };
        for tier in Tier::ALL {
            for seed in 0..200 {
                let p = plan(seed, tier);
                let mut bal: HashMap<Who, i64> = HashMap::new();
                for s in &p.steps {
                    let (spender, spend, receiver, recv) = match &s.act {
                        Act::Fund { from, to, mnear } => (Some(*from), *mnear as i64, Some(*to), *mnear as i64),
                        Act::Buy { who, mnear } => (Some(*who), *mnear as i64, None, 0),
                        Act::Call { kol } => (Some(Who::Kol(*kol)), 0, None, 0),
                        _ => (None, 0, None, 0),
                    };
                    if let Some(r) = receiver {
                        *bal.entry(r).or_insert_with(|| start(&r)) += recv;
                    }
                    if let Some(w) = spender {
                        let b = bal.entry(w).or_insert_with(|| start(&w));
                        *b -= spend + GAS_RESERVE_MNEAR as i64;
                        assert!(*b >= 0, "{tier:?} {seed}: {w:?} runs out of money at {}ms ({b} mNEAR short)", s.at_ms);
                    }
                }
            }
        }
    }

    #[test]
    fn commitment_matches_the_contracts_preimage() {
        let sides = vec!["aa".repeat(32), "bb".repeat(32)];
        let ours = preimage(2, &sides, "pepper");
        let theirs = cabal::preimage(
            2,
            &sides.iter().map(|s| s.parse::<near_sdk::AccountId>().unwrap()).collect::<Vec<_>>(),
            "pepper",
        );
        assert_eq!(ours, theirs);
        assert_eq!(commitment(2, &sides, "pepper").len(), 64);
        assert_ne!(commitment(1, &sides, "pepper"), commitment(2, &sides, "pepper"));
    }

    // ───────────── the solver: one clear answer from the public feed, on every tier ─────────────

    #[test]
    fn the_public_solver_finds_exactly_the_cabal_on_every_tier() {
        let seats = sim::seats();
        let lanes = sim::lanes();
        for tier in Tier::ALL {
            for seed in 0..300 {
                let p = plan(seed, tier);
                let feed = sim::feed(&p);
                let v = solve_with(&feed, &seats, &lanes, true).unwrap_or_else(|| panic!("{tier:?} {seed}: solver found nothing"));
                assert_eq!(v.seat, p.cabal, "{tier:?} {seed}: solver named the wrong caller");
                assert!(v.score >= v.runner_up + 3, "{tier:?} {seed}: the answer must be clearly ahead ({} vs {})", v.score, v.runner_up);
            }
        }
    }

    #[test]
    fn timing_alone_is_enough_so_the_ghost_tier_is_fair() {
        let seats = sim::seats();
        let lanes = sim::lanes();
        for tier in Tier::ALL {
            for seed in 0..300 {
                let p = plan(seed, tier);
                let v = solve_with(&sim::feed(&p), &seats, &lanes, false).unwrap_or_else(|| panic!("{tier:?} {seed}: timing found nothing"));
                assert_eq!(v.seat, p.cabal, "{tier:?} {seed}");
                assert_eq!(v.runner_up, 0, "{tier:?} {seed}: no honest caller scores on timing alone");
            }
        }
    }

    #[test]
    fn the_lazy_reading_works_on_rookie_and_fails_on_the_harder_tiers() {
        // Lazy reading: blame whoever directly paid for a fresh wallet that bought before that caller's call.
        let seats = sim::seats();
        let lanes = sim::lanes();
        let lazy = |p: &Plan| -> Option<usize> {
            let feed = sim::feed(p);
            let calls: HashMap<usize, u64> = feed.iter().filter(|e| e.kind == "Call").filter_map(|e| seats.iter().position(|s| s.wallet == e.actor).map(|i| (i, e.ts_ms))).collect();
            let mut hits: Vec<usize> = vec![];
            for d in dossiers(&feed, &seats, &lanes) {
                if let (Some(s), Some(b)) = (seats.iter().position(|s| s.wallet == d.funder), d.first_buy_ms) {
                    if calls.get(&s).map(|c| b + 6_000 < *c).unwrap_or(false) {
                        hits.push(s);
                    }
                }
            }
            hits.sort();
            hits.dedup();
            if hits.len() == 1 { Some(hits[0]) } else { None }
        };
        for seed in 0..300 {
            assert_eq!(lazy(&plan(seed, Tier::Rookie)), Some(plan(seed, Tier::Rookie).cabal), "rookie {seed}: the lazy reading works");
            assert_ne!(lazy(&plan(seed, Tier::Hop)), Some(plan(seed, Tier::Hop).cabal), "hop {seed}: the lazy reading must fail");
            assert_ne!(lazy(&plan(seed, Tier::Ghost)), Some(plan(seed, Tier::Ghost).cabal), "ghost {seed}: the lazy reading must fail");
        }
    }

    #[test]
    fn funding_trails_alone_never_name_the_cabal_on_ghost() {
        // The funding half of the solver (no timing) names the cabal on Rookie and Hop but has nothing on Ghost.
        let seats = sim::seats();
        let lanes = sim::lanes();
        for seed in 0..200 {
            for tier in [Tier::Rookie, Tier::Hop] {
                let p = plan(seed, tier);
                let v = crate::dossier::solve_funding_only(&sim::feed(&p), &seats, &lanes).unwrap();
                assert_eq!(v.seat, p.cabal, "{tier:?} {seed}");
            }
            // On Ghost the only trail left belongs to the honest friend wallet, so reading funding alone points at an
            // innocent caller (a trap) or at nobody. It must never name the cabal.
            let p = plan(seed, Tier::Ghost);
            let v = crate::dossier::solve_funding_only(&sim::feed(&p), &seats, &lanes);
            assert_ne!(v.map(|v| v.seat), Some(p.cabal), "ghost {seed}: the trail is laundered");
        }
    }

    #[test]
    fn a_human_has_time_to_act_once_the_evidence_is_in() {
        // The evidence is complete when the cabal's second wallet has sold. A player then needs time to read it and
        // file an accusation, so that moment must leave a real margin before the market closes, in the worst seed.
        let seats = sim::seats();
        let lanes = sim::lanes();
        let mut worst = 0u64;
        for tier in Tier::ALL {
            for seed in 0..300 {
                let p = plan(seed, tier);
                let feed = sim::feed(&p);
                // first moment the solver would clear the 6-point bar a careful player needs, replaying the feed in time order
                let mut found = None;
                for n in 1..=feed.len() {
                    if let Some(v) = solve_with(&feed[..n], &seats, &lanes, true) {
                        if v.score >= 6 && v.seat == p.cabal {
                            found = Some(feed[n - 1].ts_ms);
                            break;
                        }
                    }
                }
                let at = found.unwrap_or_else(|| panic!("{tier:?} {seed}: never reaches a clear answer"));
                worst = worst.max(at);
                assert!(at + 25_000 <= PLAY_SECS as u64 * 1000, "{tier:?} {seed}: clear only at {at}ms, leaving under 20s to act");
            }
        }
        eprintln!("worst-case moment the answer is clear: {worst}ms of {}ms", PLAY_SECS * 1000);
    }

    #[test]
    #[ignore]
    fn print_pinned_plans() {
        for tier in Tier::ALL {
            let p = plan(demo_seed(tier), tier);
            eprintln!("TIER {tier:?} calls {:?}", p.call_ms);
            for s in &p.steps {
                if matches!(&s.act, Act::Call { .. } | Act::Fund { to: Who::Side(_) | Who::Relay | Who::Friend | Who::Decoy, .. } | Act::Buy { who: Who::Side(_) | Who::Friend | Who::Decoy, .. } | Act::Sell { who: Who::Side(_), .. })
                    || matches!(&s.act, Act::Buy { who: Who::Kol(_), .. }) && s.at_ms < 90_000 && p.conviction.is_some() && s.at_ms % 1 == 0
                {
                    eprintln!("PLAN {:>6}ms {:?}", s.at_ms, s.act);
                }
            }
        }
    }

    #[test]
    fn the_pinned_demo_tables_are_what_the_walkthrough_says() {
        // The README walks a judge through the demo table: @mira is the cabal on every tier, @dex calls first, and the
        // honest friend wallet belongs to @oren. If this fails, update the walkthrough before updating this test.
        for tier in Tier::ALL {
            let p = plan(demo_seed(tier), tier);
            assert_eq!(p.cabal, 1, "{tier:?}: @mira is the cabal");
            assert_eq!(p.call_order, vec![2, 1, 0, 3], "{tier:?}: @dex, @mira, @kaito, @oren");
            assert_eq!(p.friend_of, 3, "{tier:?}: the friend wallet is @oren's");
            if tier.has_decoys() {
                assert_eq!((p.conviction, p.decoy_for), (Some(0), Some(2)), "{tier:?}: @kaito buys early openly; the decoy runs ahead of @dex's call");
            }
        }
    }

    #[test]
    fn demo_label_is_stable() {
        assert_eq!(seed_from_label("demo"), seed_from_label("demo"));
        assert_ne!(seed_from_label("demo"), seed_from_label("other"));
    }

    #[test]
    fn tiers_parse() {
        assert_eq!(Tier::parse("Hop"), Some(Tier::Hop));
        assert_eq!(Tier::parse("nope"), None);
        assert_eq!(Tier::ALL.map(|t| t.index()), [0, 1, 2]);
    }
}
