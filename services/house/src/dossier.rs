//! Dossiers: what anyone can learn about a wallet from the public feed alone.
//!
//! Used three ways: as the evidence cards in the UI, as the solver that proves a table is winnable from the chain
//! (the self-test runs it against real testnet data and the plan tests run it over hundreds of seeds), and as the
//! post-reveal autopsy. It never sees the hidden role.
//!
//! Funding is a *trail*: each wallet's first funder, then that funder's first funder, up to a few hops. A trail that
//! passes through a lane (the exchange) is laundered: it has no owner, so it proves nothing by itself.

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
    /// The account that first paid for this wallet.
    pub funder: String,
    /// Seat index when that account is a caller's own wallet.
    pub funder_seat: Option<usize>,
    /// The caller at the end of the funding trail, when the trail does not pass through the exchange.
    /// Withheld from the live API; it is the answer for the Rookie and Hop tiers.
    pub root_seat: Option<usize>,
    pub funded_ms: u64,
    pub funded_idx: u32,
    pub first_buy_ms: Option<u64>,
    pub first_buy_idx: Option<u32>,
    pub chips_bought: u64,
    pub first_sell_ms: Option<u64>,
    pub chips_sold: u64,
    /// When the trail's root caller posted their call.
    pub funder_call_ms: Option<u64>,
    /// Positive when the wallet bought before that call.
    pub lead_ms: Option<i64>,
    /// Milliseconds after that call that the first sell landed.
    pub dumped_after_ms: Option<i64>,
}

const MAX_HOPS: usize = 4;

/// One dossier per wallet that someone paid for. Lanes (the exchange) and the callers' own wallets are not rows.
pub fn dossiers(feed: &[EntryV], seats: &[SeatV], lanes: &[String]) -> Vec<Dossier> {
    let seat_of: HashMap<&str, usize> = seats.iter().enumerate().map(|(i, s)| (s.wallet.as_str(), i)).collect();
    let is_lane = |a: &str| lanes.iter().any(|l| l == a);
    let mut call_ms: HashMap<usize, u64> = HashMap::new();
    for e in feed.iter().filter(|e| e.kind == "Call") {
        if let Some(&s) = seat_of.get(e.actor.as_str()) {
            call_ms.entry(s).or_insert(e.ts_ms);
        }
    }
    // first funder per wallet, like the contract records it
    let mut first: HashMap<&str, (&str, u64, u32)> = HashMap::new();
    let mut order: Vec<&str> = vec![];
    for e in feed.iter().filter(|e| e.kind == "Fund") {
        let Some(to) = e.other.as_deref() else { continue };
        if !first.contains_key(to) {
            first.insert(to, (e.actor.as_str(), e.ts_ms, e.idx));
            order.push(to);
        }
    }
    let root = |w: &str| -> Option<usize> {
        let mut cur = w;
        for _ in 0..MAX_HOPS {
            let (f, _, _) = first.get(cur)?;
            if let Some(&s) = seat_of.get(f) {
                return Some(s);
            }
            if is_lane(f) {
                return None;
            }
            cur = f;
        }
        None
    };
    let mut out = vec![];
    for to in order {
        if is_lane(to) || seat_of.contains_key(to) {
            continue;
        }
        let (funder, funded_ms, funded_idx) = first[to];
        let root_seat = root(to);
        let mut d = Dossier {
            wallet: to.to_string(),
            funder: funder.to_string(),
            funder_seat: seat_of.get(funder).copied(),
            root_seat,
            funded_ms,
            funded_idx,
            first_buy_ms: None,
            first_buy_idx: None,
            chips_bought: 0,
            first_sell_ms: None,
            chips_sold: 0,
            funder_call_ms: root_seat.and_then(|s| call_ms.get(&s).copied()),
            lead_ms: None,
            dumped_after_ms: None,
        };
        for t in feed.iter().filter(|t| t.actor == to) {
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

/// A wallet front-runs a call when it bought clearly before it (but not long before), sold into the wave after it,
/// and no other call fell in between, so the dump belongs to that one call.
pub const LEAD_MIN_MS: u64 = 6_000;
pub const LEAD_MAX_MS: u64 = 32_000;
pub const DUMP_MIN_MS: u64 = 3_000;
pub const DUMP_MAX_MS: u64 = 40_000;

fn timing_scores(feed: &[EntryV], seats: &[SeatV], lanes: &[String], score: &mut [i32]) {
    let seat_of: HashMap<&str, usize> = seats.iter().enumerate().map(|(i, s)| (s.wallet.as_str(), i)).collect();
    let calls: Vec<(usize, u64)> = feed
        .iter()
        .filter(|e| e.kind == "Call")
        .filter_map(|e| seat_of.get(e.actor.as_str()).map(|s| (*s, e.ts_ms)))
        .collect();
    for d in dossiers(feed, seats, lanes) {
        let (Some(buy), Some(sell)) = (d.first_buy_ms, d.first_sell_ms) else { continue };
        for &(k, c) in &calls {
            let lead_ok = buy + LEAD_MIN_MS <= c && c - buy <= LEAD_MAX_MS;
            let dump_ok = sell >= c + DUMP_MIN_MS && sell <= c + DUMP_MAX_MS;
            let alone = !calls.iter().any(|&(_, o)| o != c && o > buy && o < sell);
            if lead_ok && dump_ok && alone {
                score[k] += 3;
            }
        }
    }
}

fn funding_scores(feed: &[EntryV], seats: &[SeatV], lanes: &[String], score: &mut [i32]) {
    for d in dossiers(feed, seats, lanes) {
        if let Some(s) = d.root_seat {
            score[s] += 2;
        }
    }
}

fn leader(score: &[i32]) -> Option<Verdict> {
    let mut ranked: Vec<(usize, i32)> = score.iter().copied().enumerate().collect();
    ranked.sort_by_key(|r| std::cmp::Reverse(r.1));
    let (best, best_score) = *ranked.first()?;
    let runner = ranked.get(1).map(|r| r.1).unwrap_or(0);
    (best_score > 0 && best_score > runner).then_some(Verdict { seat: best, score: best_score, runner_up: runner })
}

/// The full reading a careful player does: funding trails and timing together.
pub fn solve(feed: &[EntryV], seats: &[SeatV], lanes: &[String]) -> Option<Verdict> {
    solve_with(feed, seats, lanes, true)
}

/// `use_funding = false` is the timing-only reading, the one that must still work on the Ghost tier.
pub fn solve_with(feed: &[EntryV], seats: &[SeatV], lanes: &[String], use_funding: bool) -> Option<Verdict> {
    let mut score = vec![0i32; seats.len()];
    timing_scores(feed, seats, lanes, &mut score);
    if use_funding {
        funding_scores(feed, seats, lanes, &mut score);
    }
    leader(&score)
}

/// Funding trails only. Names the cabal on the Rookie and Hop tiers and finds nothing on Ghost.
pub fn solve_funding_only(feed: &[EntryV], seats: &[SeatV], lanes: &[String]) -> Option<Verdict> {
    let mut score = vec![0i32; seats.len()];
    funding_scores(feed, seats, lanes, &mut score);
    leader(&score)
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

    fn lanes() -> Vec<String> {
        vec!["exchange".into()]
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
    fn builds_a_dossier_per_fresh_wallet_and_skips_lanes() {
        let mut f = feed();
        f.push(e(11, 1_000, "Fund", "bw", Some("exchange"), 0));
        let d = dossiers(&f, &seats(), &lanes());
        assert_eq!(d.len(), 3, "the exchange lane is not a wallet row");
        let s1 = d.iter().find(|x| x.wallet == "s1").unwrap();
        assert_eq!(s1.funder_seat, Some(1));
        assert_eq!(s1.root_seat, Some(1));
        assert_eq!(s1.lead_ms, Some(18_000));
        assert_eq!(s1.dumped_after_ms, Some(8_000));
        let f1 = d.iter().find(|x| x.wallet == "f1").unwrap();
        assert_eq!(f1.lead_ms, Some(-12_000), "friend bought after its funder's call");
        assert_eq!(f1.dumped_after_ms, None);
        let wh = d.iter().find(|x| x.wallet == "wh").unwrap();
        assert_eq!((wh.funder_seat, wh.root_seat, wh.lead_ms), (None, None, None));
    }

    #[test]
    fn a_relay_keeps_the_trail_but_the_exchange_launders_it() {
        let f = vec![
            e(0, 1_000, "Fund", "bw", Some("relay"), 0),
            e(1, 6_000, "Fund", "relay", Some("s1"), 0),
            e(2, 2_000, "Fund", "bw", Some("exchange"), 0),
            e(3, 7_000, "Fund", "exchange", Some("g1"), 0),
        ];
        let d = dossiers(&f, &seats(), &lanes());
        let s1 = d.iter().find(|x| x.wallet == "s1").unwrap();
        assert_eq!((s1.funder.as_str(), s1.funder_seat, s1.root_seat), ("relay", None, Some(1)), "two legs, still owned by the cabal");
        let g1 = d.iter().find(|x| x.wallet == "g1").unwrap();
        assert_eq!((g1.funder.as_str(), g1.root_seat), ("exchange", None), "laundered through the exchange");
    }

    #[test]
    fn solver_finds_the_cabal_from_the_chain_alone() {
        let v = solve(&feed(), &seats(), &lanes()).unwrap();
        assert_eq!(v.seat, 1);
        assert!(v.score > v.runner_up);
    }

    #[test]
    fn solver_refuses_to_guess_without_evidence() {
        // Only the exchange whale remains: it has no owner, no dump, and no call to front-run.
        let f: Vec<EntryV> = feed().into_iter().filter(|x| matches!(x.actor.as_str(), "exchange" | "wh" | "house")).collect();
        assert!(solve(&f, &seats(), &lanes()).is_none());
    }

    #[test]
    fn first_funder_wins_like_the_contract() {
        let mut f = feed();
        f.push(e(11, 70_000, "Fund", "dw", Some("s1"), 0));
        let d = dossiers(&f, &seats(), &lanes());
        assert_eq!(d.iter().find(|x| x.wallet == "s1").unwrap().funder_seat, Some(1));
    }

    #[test]
    fn a_wallet_that_sits_across_several_calls_is_not_a_front_runner() {
        // bought before every call and sold after all of them: the whale shape
        let f = vec![
            e(0, 5_000, "Fund", "exchange", Some("wh"), 0),
            e(1, 10_000, "Buy", "wh", None, 90),
            e(2, 30_000, "Call", "aw", None, 1),
            e(3, 52_000, "Call", "bw", None, 1),
            e(4, 70_000, "Sell", "wh", None, 90),
        ];
        assert!(solve_with(&f, &seats(), &lanes(), false).is_none());
    }

    #[test]
    fn a_wallet_that_never_sells_scores_nothing() {
        let f = vec![
            e(0, 5_000, "Fund", "exchange", Some("dc"), 0),
            e(1, 20_000, "Buy", "dc", None, 90),
            e(2, 30_000, "Call", "aw", None, 1),
        ];
        assert!(solve_with(&f, &seats(), &lanes(), false).is_none());
    }
}
