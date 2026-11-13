//! `house selftest`: play one full table against the live testnet and check the game's promises.
//!
//! 1. A table opens and its commitment is on-chain.
//! 2. The public feed alone is enough for the solver to name a suspect before the market closes.
//! 3. A guest who accuses that suspect is paid after the reveal; a guest who accuses someone else is not.
//! 4. The revealed preimage hashes to the commitment recorded before play.

use std::{sync::Arc, time::{Duration, Instant}};

use anyhow::{bail, Result};
use tokio::time::sleep;

use crate::{
    chain::YOCTO_PER_MNEAR,
    game::Game,
    model::Phase,
};

fn near(y: u128) -> f64 {
    y as f64 / (YOCTO_PER_MNEAR as f64 * 1000.0)
}

pub async fn run(g: Arc<Game>) -> Result<()> {
    let demo = std::env::args().any(|a| a == "--demo");
    let (tok_a, a, _) = g.guest(None, "selftest").await?;
    let (_tok_b, b, _) = g.guest(None, "selftest").await?;
    println!("guest A {}…  guest B {}…", &a.as_str()[..8], &b.as_str()[..8]);
    let bal_a0 = g.chain.balance(&a).await;
    let bal_b0 = g.chain.balance(&b).await;

    let t = g.start_table(demo, "selftest")?;
    println!("table {} dealing (demo={demo})", t.key);
    let started = Instant::now();
    loop {
        let v = g.table_view(&t, 0, 0);
        if v.phase == Phase::Playing || v.phase.is_final() {
            break;
        }
        if started.elapsed() > Duration::from_secs(120) {
            bail!("table did not open in time: {:?}", v.error);
        }
        sleep(Duration::from_secs(1)).await;
    }
    let v = g.table_view(&t, 0, 0);
    let Some(round) = v.round else { bail!("table failed before opening: phase {:?} error {:?} note {:?}", v.phase, v.error, v.note) };
    let commitment = loop {
        if let Some(rv) = g.table_view(&t, 0, 0).round_view {
            break rv.commitment;
        }
        sleep(Duration::from_millis(500)).await;
    };
    println!("round {round} open, commitment {}…", &commitment[..16]);

    // Wait for the solver to have a clear suspect, accusing as soon as it does.
    let mut suspect: Option<usize> = None;
    let mut decoy: usize = 0;
    let mut accused_at = 0u64;
    while started.elapsed() < Duration::from_secs(230) {
        sleep(Duration::from_secs(2)).await;
        let v = g.table_view_full(&t, 0, 0);
        if v.phase != Phase::Playing {
            break;
        }
        if suspect.is_none() {
            if let Some(verdict) = v.verdict.clone() {
                if verdict.score >= 5 {
                    suspect = Some(verdict.seat);
                    decoy = (verdict.seat + 1) % 4;
                    accused_at = v.elapsed_ms.unwrap_or(0);
                    println!("solver names {} (score {} vs {}) at {}s", v.seats[verdict.seat].handle, verdict.score, verdict.runner_up, accused_at / 1000);
                    let ra = g.player_expose(&t, &a, verdict.seat as u8, 50).await;
                    println!("  A accuses suspect: ok={} {:?}", ra.ok, ra.error);
                    let rb = g.player_expose(&t, &b, decoy as u8, 50).await;
                    println!("  B accuses {}: ok={} {:?}", v.seats[decoy].handle, rb.ok, rb.error);
                    if !ra.ok || !rb.ok {
                        bail!("accusations were refused");
                    }
                }
            }
        }
        // Let A trade as well, to exercise the player path.
        if suspect.is_none() && v.elapsed_ms.unwrap_or(0) > 20_000 && v.feed_total > 4 {
            let _ = g.player_buy(&t, &a, 20).await;
        }
    }
    let suspect = match suspect {
        Some(s) => s,
        None => bail!("the solver never found a suspect from the public feed: the table is not solvable"),
    };

    println!("waiting for the reveal…");
    let wait = Instant::now();
    loop {
        let v = g.table_view(&t, 0, 0);
        if v.phase.is_final() {
            break;
        }
        if wait.elapsed() > Duration::from_secs(200) {
            bail!("no reveal after 200s");
        }
        sleep(Duration::from_secs(2)).await;
    }
    sleep(Duration::from_secs(4)).await;
    let v = g.table_view(&t, 0, 0);
    let Some(r) = v.reveal.as_ref() else { bail!("no reveal in the final view (phase {:?})", v.phase) };
    println!("revealed: {} was the cabal; sides {}", r.cabal_handle, r.sides.len());
    if !r.matches {
        bail!("revealed preimage does not hash to the commitment");
    }
    if r.commitment != commitment {
        bail!("commitment changed between open and reveal");
    }
    if r.cabal_seat as usize != suspect {
        bail!("solver was WRONG: it said seat {suspect}, cabal was seat {}", r.cabal_seat);
    }
    let _ = tok_a;
    sleep(Duration::from_secs(3)).await;
    let (bal_a1, bal_b1) = (g.chain.balance(&a).await, g.chain.balance(&b).await);
    println!("A: {:.3} -> {:.3} NEAR   B: {:.3} -> {:.3} NEAR", near(bal_a0), near(bal_a1), near(bal_b0), near(bal_b1));
    let payout = v.round_view.as_ref().map(|r| r.payout.clone()).unwrap_or_default();
    println!("pot paid: {payout} yocto; accusations at {}s", accused_at / 1000);
    if bal_b1 >= bal_b0 {
        bail!("the wrong accuser did not lose their stake");
    }
    if bal_a1 <= bal_a0.saturating_sub(near_to_yocto(0.12)) {
        bail!("the correct accuser was not paid");
    }
    // Everything the table did is replayable from the chain alone.
    let replay = g.round_view(round).await?;
    if replay.feed_total != v.feed_total {
        bail!("chain replay has {} entries but the live view had {}", replay.feed_total, v.feed_total);
    }
    println!("SELFTEST PASSED: solver found the cabal from public data, payouts correct, replay matches ({} feed entries)", replay.feed_total);
    Ok(())
}

fn near_to_yocto(n: f64) -> u128 {
    (n * 1_000.0) as u128 * YOCTO_PER_MNEAR
}
