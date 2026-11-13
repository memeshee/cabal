//! The deal: a pure, seeded script of everything the house's wallets do during one table.
//!
//! Nothing in here touches the network. That keeps the game's fairness properties testable:
//! the cabal's side wallets buy *before* its call and dump into the crowd that follows it,
//! honest KOLs never buy before their own call, and the red herrings (an exchange-funded early whale, a
//! KOL-funded friend wallet that buys *after* the call) exist but cannot be mistaken for the real pattern
//! by someone who reads the funding links and the timestamps.

use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const N_KOLS: usize = 4;
pub const N_SIDES: usize = 3;
pub const N_CROWD: usize = 8;
pub const PLAY_SECS: u32 = 180;
pub const REVEAL_SECS: u32 = 90;

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
    pub cabal: usize,
    pub friend_of: usize,
    /// KOL seat index per call slot, in time order.
    pub call_order: Vec<usize>,
    pub call_ms: Vec<u64>,
    pub salt: String,
    pub steps: Vec<Step>,
}

const SLOT_MS: [u64; 4] = [36_000, 63_000, 90_000, 117_000];

impl Plan {
    pub fn generate(seed: u64, salt: String) -> Plan {
        let mut rng = StdRng::seed_from_u64(seed);
        let cabal = rng.gen_range(0..N_KOLS);
        let mut others: Vec<usize> = (0..N_KOLS).filter(|k| *k != cabal).collect();
        others.shuffle(&mut rng);
        let friend_of = others[0];
        let mut call_order: Vec<usize> = (0..N_KOLS).collect();
        call_order.shuffle(&mut rng);
        let call_ms: Vec<u64> = SLOT_MS.iter().map(|b| b + rng.gen_range(0..4_000)).collect();

        let mut steps: Vec<Step> = Vec::new();
        let mut push = |at_ms: u64, act: Act| steps.push(Step { at_ms, act });

        // Early whale: funded by the exchange, buys before every call, sells late.
        push(5_000, Act::Fund { from: Who::Exchange, to: Who::Whale, mnear: 160 });
        push(11_000 + rng.gen_range(0..2_000), Act::Buy { who: Who::Whale, mnear: 100 });
        push(146_000, Act::Sell { who: Who::Whale, pct: 100 });

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
                for j in 0..N_SIDES {
                    let j64 = j as u64;
                    push(t - 31_000 + j64 * 2_500, Act::Fund { from: Who::Kol(k), to: Who::Side(j), mnear: 110 });
                    push(t - 16_000 + j64 * 2_200 + rng.gen_range(0..800), Act::Buy { who: Who::Side(j), mnear: rng.gen_range(55..75) });
                    push(t + 6_500 + j64 * 3_000 + rng.gen_range(0..800), Act::Sell { who: Who::Side(j), pct: 100 });
                }
            }
            if k == friend_of {
                push(t - 20_000, Act::Fund { from: Who::Kol(k), to: Who::Friend, mnear: 90 });
                push(t + 12_000, Act::Buy { who: Who::Friend, mnear: 60 });
                push((t + 25_000).min(158_000), Act::Sell { who: Who::Friend, pct: 50 });
            }
        }
        // The crowd takes profit near the end.
        for c in 0..N_CROWD {
            push(143_000 + c as u64 * 1_600 + rng.gen_range(0..600), Act::Sell { who: Who::Crowd(c), pct: 100 });
        }
        drop(push);
        steps.sort_by_key(|s| s.at_ms);
        Plan { seed, cabal, friend_of, call_order, call_ms, salt, steps }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(seed: u64) -> Plan {
        Plan::generate(seed, "salt".into())
    }

    fn mn(m: u64) -> u128 {
        m as u128 * 1_000_000_000_000_000_000_000
    }

    #[test]
    fn deterministic_for_a_seed() {
        let a = plan(7);
        let b = plan(7);
        assert_eq!(a.cabal, b.cabal);
        assert_eq!(a.steps.len(), b.steps.len());
        assert!(a.steps.iter().zip(&b.steps).all(|(x, y)| x.at_ms == y.at_ms && x.act == y.act));
    }

    #[test]
    fn every_seat_can_be_the_cabal() {
        let mut seen = [false; N_KOLS];
        for s in 0..200 {
            seen[plan(s).cabal] = true;
        }
        assert!(seen.iter().all(|x| *x), "cabal seat must not be predictable");
    }

    #[test]
    fn cabal_can_be_first_middle_or_last_caller() {
        let mut slots = [false; 4];
        for s in 0..300 {
            let p = plan(s);
            slots[p.call_order.iter().position(|k| *k == p.cabal).unwrap()] = true;
        }
        assert!(slots.iter().all(|x| *x));
    }

    #[test]
    fn side_wallets_are_funded_then_buy_before_the_cabal_call_then_dump_after() {
        for seed in 0..200 {
            let p = plan(seed);
            let call = p.cabal_call_ms();
            for j in 0..N_SIDES {
                let fund = p.steps.iter().find(|s| matches!(&s.act, Act::Fund { to: Who::Side(x), .. } if *x == j)).unwrap();
                let buy = p.steps.iter().find(|s| s.act_is_buy(Who::Side(j))).unwrap();
                let sell = p.steps.iter().find(|s| s.act_is_sell(Who::Side(j))).unwrap();
                assert!(matches!(fund.act, Act::Fund { from: Who::Kol(k), .. } if k == p.cabal), "cabal pays for its own side wallets");
                assert!(fund.at_ms + 5_000 < buy.at_ms, "wallet is funded before it buys (seed {seed})");
                assert!(buy.at_ms + 8_000 < call, "side wallet buys clearly before the call (seed {seed})");
                assert!(sell.at_ms > call + 6_000, "side wallet dumps into the wave after the call (seed {seed})");
            }
        }
    }

    #[test]
    fn honest_kols_never_buy_before_their_own_call() {
        for seed in 0..200 {
            let p = plan(seed);
            for (slot, &k) in p.call_order.iter().enumerate() {
                if k == p.cabal {
                    continue;
                }
                let t = p.call_ms[slot];
                for s in p.steps.iter().filter(|s| s.act_is_buy(Who::Kol(k)) || s.act_is_buy(Who::Friend)) {
                    if s.act_is_buy(Who::Kol(k)) {
                        assert!(s.at_ms > t, "honest KOL buys only after calling (seed {seed})");
                    }
                }
            }
        }
    }

    #[test]
    fn red_herrings_are_distinguishable_from_the_pattern() {
        for seed in 0..200 {
            let p = plan(seed);
            let friend_call = p.call_ms[p.call_order.iter().position(|k| *k == p.friend_of).unwrap()];
            let friend_buy = p.steps.iter().find(|s| s.act_is_buy(Who::Friend)).unwrap();
            assert_ne!(p.friend_of, p.cabal);
            assert!(friend_buy.at_ms > friend_call, "friend wallet buys after the call, unlike a side wallet");
            let whale_fund = p.steps.iter().find(|s| matches!(&s.act, Act::Fund { to: Who::Whale, .. })).unwrap();
            assert!(matches!(whale_fund.act, Act::Fund { from: Who::Exchange, .. }), "whale is exchange-funded");
            let whale_buy = p.steps.iter().find(|s| s.act_is_buy(Who::Whale)).unwrap();
            assert!(whale_buy.at_ms < p.call_ms[0], "whale buys before everyone, which is why it needs the funding link to rule out");
        }
    }

    #[test]
    fn everything_finishes_inside_the_market_window() {
        for seed in 0..200 {
            let p = plan(seed);
            let last = p.steps.iter().map(|s| s.at_ms).max().unwrap();
            assert!(last < (PLAY_SECS as u64 - 3) * 1000, "last step at {last}ms (seed {seed})");
            assert!(p.steps.iter().map(|s| s.at_ms).min().unwrap() >= 1_000);
            assert!(p.steps.windows(2).all(|w| w[0].at_ms <= w[1].at_ms));
        }
    }

    #[test]
    fn exactly_one_call_per_kol() {
        let p = plan(3);
        for k in 0..N_KOLS {
            assert_eq!(p.steps.iter().filter(|s| s.act == Act::Call { kol: k }).count(), 1);
        }
    }

    #[test]
    fn the_cabal_actually_profits_on_the_real_curve() {
        // Replay the plan's market steps on the contract's own curve. The side wallets must come out ahead,
        // otherwise there is no motive for the pattern the player is hunting.
        use std::collections::HashMap;
        let mut ratios = vec![];
        for seed in 0..60 {
            let p = plan(seed);
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
                        let n = h * *pct as u64 / 100;
                        if n > 0 {
                            let v = cabal::curve_value(supply - n, supply);
                            supply -= n;
                            held.insert(key(who), h - n);
                            if matches!(who, Who::Side(_)) {
                                got += v;
                            }
                        }
                    }
                    _ => {}
                }
            }
            ratios.push(got as f64 / spent as f64);
        }
        let avg = ratios.iter().sum::<f64>() / ratios.len() as f64;
        let min = ratios.iter().cloned().fold(f64::MAX, f64::min);
        assert!(min > 1.05, "side wallets must profit in every deal, worst was {min:.2}x");
        assert!(avg > 1.2, "average side-wallet return should be a clear motive, was {avg:.2}x");
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

    #[test]
    fn demo_label_is_stable() {
        assert_eq!(seed_from_label("demo"), seed_from_label("demo"));
        assert_ne!(seed_from_label("demo"), seed_from_label("other"));
    }

    impl Step {
        fn act_is_buy(&self, w: Who) -> bool {
            matches!(&self.act, Act::Buy { who, .. } if *who == w)
        }
        fn act_is_sell(&self, w: Who) -> bool {
            matches!(&self.act, Act::Sell { who, .. } if *who == w)
        }
    }
}
