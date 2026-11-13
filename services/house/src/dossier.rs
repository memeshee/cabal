//! Dossiers: what anyone can learn about a wallet from the public feed alone.
//!
//! Used three ways: as the evidence cards in the UI, as the solver that proves a table is winnable from the chain
//! (the self-test runs it against real testnet data), and as the post-reveal autopsy. It never sees the hidden role.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntryV {
    pub idx: u32,
    pub ts_ms: u64,
    pub block: u64,
    pub kind: String,
    pub actor: String,
    pub other: Option<String>,
    pub chips: u64,
    pub near: String,
    pub text: Option<String>,
}

impl EntryV {
    pub fn near_yocto(&self) -> u128 {
        self.near.parse().unwrap_or(0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeatV {
    pub handle: String,
    pub wallet: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Dossier {
    pub wallet: String,
    /// Seat index when the funder is a KOL wallet, `None` for the exchange or anything else.
    pub funder_seat: Option<usize>,
    pub funder: String,
    pub funded_ms: u64,
    pub funded_idx: u32,
    pub first_buy_ms: Option<u64>,
    pub first_buy_idx: Option<u32>,
    pub chips_bought: u64,
    pub first_sell_ms: Option<u64>,
    pub chips_sold: u64,
    /// When the funder's own call was posted, if it was a KOL.
    pub funder_call_ms: Option<u64>,
    /// Positive when the wallet bought before its funder's call.
    pub lead_ms: Option<i64>,
    /// Seconds after the funder's call that the first sell landed, when it did.
    pub dumped_after_ms: Option<i64>,
}

/// One dossier per wallet that first appears as the target of a `Fund`.
pub fn dossiers(feed: &[EntryV], seats: &[SeatV]) -> Vec<Dossier> {
    let seat_of: HashMap<&str, usize> = seats.iter().enumerate().map(|(i, s)| (s.wallet.as_str(), i)).collect();
    let mut call_ms: HashMap<usize, u64> = HashMap::new();
    for e in feed.iter().filter(|e| e.kind == "Call") {
        if let Some(&s) = seat_of.get(e.actor.as_str()) {
            call_ms.entry(s).or_insert(e.ts_ms);
        }
    }
    let mut out: Vec<Dossier> = vec![];
    for e in feed.iter().filter(|e| e.kind == "Fund") {
        let Some(to) = e.other.as_ref() else { continue };
        if out.iter().any(|d| &d.wallet == to) {
            continue; // the first funder is the one the contract records
        }
        let funder_seat = seat_of.get(e.actor.as_str()).copied();
        let mut d = Dossier {
            wallet: to.clone(),
            funder_seat,
            funder: e.actor.clone(),
            funded_ms: e.ts_ms,
            funded_idx: e.idx,
            first_buy_ms: None,
            first_buy_idx: None,
            chips_bought: 0,
            first_sell_ms: None,
            chips_sold: 0,
            funder_call_ms: funder_seat.and_then(|s| call_ms.get(&s).copied()),
            lead_ms: None,
            dumped_after_ms: None,
        };
        for t in feed.iter().filter(|t| &t.actor == to) {
            match t.kind.as_str() {
                "Buy" => {
                    d.chips_bought += t.chips;
                    if d.first_buy_ms.is_none() {
                        d.first_buy_ms = Some(t.ts_ms);
                        d.first_buy_idx = Some(t.idx);
                    }
                }
                "Sell" => {
                    d.chips_sold += t.chips;
                    if d.first_sell_ms.is_none() {
                        d.first_sell_ms = Some(t.ts_ms);
                    }
                }
                _ => {}
            }
        }
        if let (Some(c), Some(b)) = (d.funder_call_ms, d.first_buy_ms) {
            d.lead_ms = Some(c as i64 - b as i64);
        }
        if let (Some(c), Some(s)) = (d.funder_call_ms, d.first_sell_ms) {
            d.dumped_after_ms = Some(s as i64 - c as i64);
        }
        out.push(d);
    }
    out
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Verdict {
    pub seat: usize,
    pub score: i32,
    pub runner_up: i32,
}

/// Score each seat by the pattern a careful reader would look for: fresh wallets that this seat funded, that bought
/// clearly before its call, and that sold into the crowd after it. Returns the clear leader, if there is one.
pub fn solve(feed: &[EntryV], seats: &[SeatV]) -> Option<Verdict> {
    let mut score = vec![0i32; seats.len()];
    for d in dossiers(feed, seats) {
        let Some(s) = d.funder_seat else { continue };
        if let Some(lead) = d.lead_ms {
            if lead > 6_000 {
                score[s] += 3;
            }
        }
        if let Some(after) = d.dumped_after_ms {
            if (0..40_000).contains(&after) {
                score[s] += 2;
            }
        }
    }
    let mut ranked: Vec<(usize, i32)> = score.iter().copied().enumerate().collect();
    ranked.sort_by_key(|r| std::cmp::Reverse(r.1));
    let (best, best_score) = ranked[0];
    let runner = ranked.get(1).map(|r| r.1).unwrap_or(0);
    if best_score > 0 && best_score > runner {
        Some(Verdict { seat: best, score: best_score, runner_up: runner })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(idx: u32, ts: u64, kind: &str, actor: &str, other: Option<&str>, chips: u64) -> EntryV {
        EntryV { idx, ts_ms: ts, block: idx as u64, kind: kind.into(), actor: actor.into(), other: other.map(Into::into), chips, near: "0".into(), text: None }
    }

    fn seats() -> Vec<SeatV> {
        ["a", "b", "c", "d"].iter().map(|h| SeatV { handle: format!("@{h}"), wallet: format!("{h}w") }).collect()
    }

    fn feed() -> Vec<EntryV> {
        vec![
            e(0, 0, "Open", "house", None, 0),
            // cabal = seat 1 ("bw"): funds s1 then s1 buys, THEN bw calls, THEN s1 dumps
            e(1, 5_000, "Fund", "bw", Some("s1"), 0),
            e(2, 12_000, "Buy", "s1", None, 100),
            e(3, 30_000, "Call", "bw", None, 1),
            e(4, 33_000, "Buy", "cw1", None, 200),
            e(5, 38_000, "Sell", "s1", None, 100),
            // honest seat 2 ("cw"): funds a friend who buys only after the call
            e(6, 40_000, "Fund", "cw", Some("f1"), 0),
            e(7, 50_000, "Call", "cw", None, 2),
            e(8, 62_000, "Buy", "f1", None, 50),
            // exchange-funded whale: early buy, no KOL link
            e(9, 6_000, "Fund", "exchange", Some("wh"), 0),
            e(10, 9_000, "Buy", "wh", None, 80),
        ]
    }

    #[test]
    fn builds_a_dossier_per_fresh_wallet() {
        let d = dossiers(&feed(), &seats());
        assert_eq!(d.len(), 3);
        let s1 = d.iter().find(|x| x.wallet == "s1").unwrap();
        assert_eq!(s1.funder_seat, Some(1));
        assert_eq!(s1.lead_ms, Some(18_000));
        assert_eq!(s1.dumped_after_ms, Some(8_000));
        let f1 = d.iter().find(|x| x.wallet == "f1").unwrap();
        assert_eq!(f1.lead_ms, Some(-12_000), "friend bought after its funder's call");
        assert_eq!(f1.dumped_after_ms, None);
        let wh = d.iter().find(|x| x.wallet == "wh").unwrap();
        assert_eq!(wh.funder_seat, None);
        assert_eq!(wh.lead_ms, None);
    }

    #[test]
    fn solver_finds_the_cabal_from_the_chain_alone() {
        let v = solve(&feed(), &seats()).unwrap();
        assert_eq!(v.seat, 1);
        assert!(v.score > v.runner_up);
    }

    #[test]
    fn solver_refuses_to_guess_without_evidence() {
        let f: Vec<EntryV> = feed().into_iter().filter(|x| !(x.kind == "Fund" && x.actor == "bw")).collect();
        assert!(solve(&f, &seats()).is_none());
    }

    #[test]
    fn first_funder_wins_like_the_contract() {
        let mut f = feed();
        f.push(e(11, 70_000, "Fund", "dw", Some("s1"), 0));
        let d = dossiers(&f, &seats());
        assert_eq!(d.iter().find(|x| x.wallet == "s1").unwrap().funder_seat, Some(1));
    }
}
