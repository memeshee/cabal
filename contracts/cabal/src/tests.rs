use super::*;
use near_sdk::test_utils::{accounts, VMContextBuilder};
use near_sdk::testing_env;

const NEAR: u128 = 1_000_000_000_000_000_000_000_000;

fn acc(s: &str) -> AccountId {
    s.parse().unwrap()
}

fn ctx(who: &str, deposit: u128, ms: u64) {
    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(acc(who))
        .signer_account_id(acc(who))
        .attached_deposit(NearToken::from_yoctonear(deposit))
        .block_timestamp(ms * 1_000_000)
        .build());
}

fn seats() -> Vec<Seat> {
    ["kaito", "mira", "dex", "oren"]
        .iter()
        .map(|h| Seat { handle: format!("@{h}"), wallet: acc(&format!("{h}.cabal.testnet")) })
        .collect()
}

fn funders() -> Vec<AccountId> {
    let mut f: Vec<AccountId> = seats().into_iter().map(|s| s.wallet).collect();
    f.push(acc("exchange.cabal.testnet"));
    f
}

fn sides() -> Vec<AccountId> {
    vec![acc("s1.cabal.testnet"), acc("s2.cabal.testnet")]
}

fn commit(seat: u8, sides: &[AccountId], salt: &str) -> String {
    hex(&near_sdk::env::sha256(preimage(seat, sides, salt).as_bytes()))
}

/// Opens round 1 at t=1000ms with a 60s market and a 60s reveal window. Cabal is seat 0 (kaito).
fn opened() -> Cabal {
    ctx("house.cabal.testnet", 0, 0);
    let mut c = Cabal::new(acc("house.cabal.testnet"));
    ctx("house.cabal.testnet", NEAR, 1_000);
    let id = c.open_round("FROG".into(), commit(0, &sides(), "pepper"), seats(), funders(), 60, 60);
    assert_eq!(id, 1);
    c
}

fn fund_sides(c: &mut Cabal) {
    for s in sides() {
        ctx("kaito.cabal.testnet", NEAR / 10, 5_000);
        c.fund(1, s);
    }
}

#[test]
fn curve_is_exact_and_solvent() {
    // Buying then selling the same chips returns exactly what was paid, never more.
    let (chips, cost) = quote_buy_chips(0, NEAR / 10);
    assert!(chips > 0 && cost <= NEAR / 10);
    assert_eq!(curve_value(0, chips), cost);
    // Selling in two halves returns the same total as selling at once.
    let half = chips / 2;
    assert_eq!(curve_value(chips - half, chips) + curve_value(0, chips - half), cost);
    // A tiny deposit never rounds up into free chips.
    let (c0, k0) = quote_buy_chips(1_000, 1);
    assert_eq!((c0, k0), (0, 0));
    // The supply cap holds.
    let (cmax, _) = quote_buy_chips(MAX_SUPPLY - 10, 1_000_000 * NEAR);
    assert_eq!(cmax, 10);
}

#[test]
fn buy_then_sell_round_trips() {
    let mut c = opened();
    ctx("alice.testnet", NEAR / 5, 2_000);
    let got = c.buy(1, 1);
    assert!(got > 0);
    let r = c.get_round(1).unwrap();
    assert_eq!(r.supply, got);
    assert_eq!(r.reserve.0, curve_value(0, got));
    ctx("alice.testnet", 0, 3_000);
    let back = c.sell(1, got, U128(0)).0;
    assert_eq!(back, curve_value(0, got));
    let r = c.get_round(1).unwrap();
    assert_eq!((r.supply, r.reserve.0), (0, 0));
}

#[test]
#[should_panic(expected = "you do not hold that many chips")]
fn cannot_sell_what_you_do_not_hold() {
    let mut c = opened();
    ctx("alice.testnet", NEAR / 5, 2_000);
    let got = c.buy(1, 1);
    ctx("bob.testnet", 0, 2_500);
    c.sell(1, got, U128(0));
}

#[test]
#[should_panic(expected = "price moved")]
fn slippage_guard_on_buy() {
    let mut c = opened();
    ctx("alice.testnet", NEAR / 5, 2_000);
    c.buy(1, 1_000_000);
}

#[test]
#[should_panic(expected = "market closed")]
fn no_buying_after_close() {
    let mut c = opened();
    ctx("alice.testnet", NEAR / 5, 62_000);
    c.buy(1, 1);
}

#[test]
fn selling_still_works_after_close() {
    let mut c = opened();
    ctx("alice.testnet", NEAR / 5, 2_000);
    let got = c.buy(1, 1);
    ctx("alice.testnet", 0, 70_000);
    assert!(c.sell(1, got, U128(0)).0 > 0);
}

#[test]
#[should_panic(expected = "only the seat's own wallet")]
fn a_seat_cannot_speak_for_another() {
    let mut c = opened();
    ctx("mira.cabal.testnet", 0, 2_000);
    c.post_call(1, 0, "buy $FROG".into());
}

#[test]
fn calls_land_in_the_feed_with_block_time() {
    let mut c = opened();
    ctx("kaito.cabal.testnet", 0, 2_000);
    c.post_call(1, 0, "$FROG is going to 10x".into());
    let feed = c.get_feed(1, 0, 50);
    assert_eq!(feed.len(), 2);
    assert_eq!(feed[1].kind, Kind::Call);
    assert_eq!(feed[1].ts_ms, 2_000);
    assert_eq!(feed[1].text.as_deref(), Some("$FROG is going to 10x"));
}

#[test]
#[should_panic(expected = "not a registered funder")]
fn players_cannot_fund_wallets() {
    let mut c = opened();
    ctx("alice.testnet", NEAR, 2_000);
    c.fund(1, acc("s1.cabal.testnet"));
}

#[test]
fn first_funder_is_recorded() {
    let mut c = opened();
    ctx("kaito.cabal.testnet", NEAR / 10, 2_000);
    c.fund(1, acc("s1.cabal.testnet"));
    ctx("mira.cabal.testnet", NEAR / 10, 2_100);
    c.fund(1, acc("s1.cabal.testnet"));
    assert_eq!(c.funded_by_of(1, acc("s1.cabal.testnet")), Some(acc("kaito.cabal.testnet")));
}

#[test]
#[should_panic(expected = "table wallets cannot accuse")]
fn table_wallets_cannot_accuse() {
    let mut c = opened();
    ctx("mira.cabal.testnet", NEAR / 10, 2_000);
    c.expose(1, 0);
}

#[test]
#[should_panic(expected = "you already accused someone")]
fn one_accusation_per_player() {
    let mut c = opened();
    ctx("alice.testnet", NEAR / 10, 2_000);
    c.expose(1, 0);
    ctx("alice.testnet", NEAR / 10, 2_500);
    c.expose(1, 1);
}

#[test]
#[should_panic(expected = "stake must be between")]
fn stake_is_bounded() {
    let mut c = opened();
    ctx("alice.testnet", 10 * NEAR, 2_000);
    c.expose(1, 0);
}

#[test]
#[should_panic(expected = "market still open")]
fn cannot_reveal_early() {
    let mut c = opened();
    fund_sides(&mut c);
    ctx("house.cabal.testnet", 0, 30_000);
    c.reveal(1, 0, sides(), "pepper".into());
}

#[test]
#[should_panic(expected = "reveal does not match the commitment")]
fn cannot_swap_the_cabal_after_the_fact() {
    let mut c = opened();
    fund_sides(&mut c);
    ctx("house.cabal.testnet", 0, 65_000);
    c.reveal(1, 1, sides(), "pepper".into());
}

#[test]
#[should_panic(expected = "reveal does not match the commitment")]
fn cannot_change_the_side_wallets_after_the_fact() {
    let mut c = opened();
    fund_sides(&mut c);
    ctx("house.cabal.testnet", 0, 65_000);
    c.reveal(1, 0, vec![acc("s1.cabal.testnet")], "pepper".into());
}

#[test]
#[should_panic(expected = "was never funded by the cabal wallet")]
fn side_wallets_must_be_funded_by_the_cabal() {
    let mut c = opened();
    // The honest KOL funds s1 and s2 first, so the cabal cannot claim them as its own.
    for s in sides() {
        ctx("mira.cabal.testnet", NEAR / 10, 2_000);
        c.fund(1, s);
    }
    ctx("house.cabal.testnet", 0, 65_000);
    c.reveal(1, 0, sides(), "pepper".into());
}

#[test]
fn caught_cabal_pays_correct_accusers_and_burns_the_bag() {
    let mut c = opened();
    fund_sides(&mut c);
    // A side wallet buys, then the cabal is accused by alice and bob; carol accuses the wrong seat.
    ctx("s1.cabal.testnet", NEAR / 5, 6_000);
    let bag = c.buy(1, 1);
    ctx("alice.testnet", NEAR / 10, 7_000);
    c.expose(1, 0);
    ctx("bob.testnet", NEAR / 5, 7_100);
    c.expose(1, 0);
    ctx("carol.testnet", NEAR / 10, 7_200);
    c.expose(1, 2);

    ctx("house.cabal.testnet", 0, 65_000);
    c.reveal(1, 0, sides(), "pepper".into());
    let r = c.get_round(1).unwrap();
    assert_eq!(r.status, Status::Revealed);
    assert_eq!(r.cabal_seat, Some(0));
    assert_eq!(c.chips_of(1, acc("s1.cabal.testnet")), 0, "the cabal's remaining bag is confiscated");
    assert_eq!(r.supply, 0);
    assert_eq!(r.reserve.0, 0, "burning the bag drains exactly its value from the reserve");
    let confiscated = curve_value(0, bag);
    // pot = bond + confiscated bag + carol's lost stake
    assert_eq!(r.payout.0, NEAR + confiscated + NEAR / 10);
    let feed = c.get_feed(1, 0, 100);
    assert!(feed.iter().any(|e| e.kind == Kind::Confiscate && e.chips == bag));
    assert!(feed.iter().any(|e| e.kind == Kind::Reveal));
}

#[test]
fn uncaught_cabal_keeps_its_bag() {
    let mut c = opened();
    fund_sides(&mut c);
    ctx("s1.cabal.testnet", NEAR / 5, 6_000);
    let bag = c.buy(1, 1);
    ctx("carol.testnet", NEAR / 10, 7_200);
    c.expose(1, 2);
    ctx("house.cabal.testnet", 0, 65_000);
    c.reveal(1, 0, sides(), "pepper".into());
    assert_eq!(c.chips_of(1, acc("s1.cabal.testnet")), bag);
    assert_eq!(c.get_round(1).unwrap().payout.0, 0);
}

#[test]
#[should_panic(expected = "reveal window passed")]
fn late_reveal_is_refused() {
    let mut c = opened();
    fund_sides(&mut c);
    ctx("house.cabal.testnet", 0, 130_000);
    c.reveal(1, 0, sides(), "pepper".into());
}

#[test]
fn silent_house_forfeits_its_bond() {
    let mut c = opened();
    ctx("alice.testnet", NEAR / 10, 2_000);
    c.expose(1, 0);
    ctx("anyone.testnet", 0, 130_000);
    c.void_unrevealed(1);
    let r = c.get_round(1).unwrap();
    assert_eq!(r.status, Status::Voided);
    assert_eq!(r.payout.0, NEAR);
    // A voided round can be followed by a new one.
    ctx("house.cabal.testnet", NEAR, 131_000);
    assert_eq!(c.open_round("TOAD".into(), commit(1, &sides(), "x"), seats(), funders(), 60, 60), 2);
}

#[test]
#[should_panic(expected = "reveal window still open")]
fn nobody_can_void_early() {
    let mut c = opened();
    ctx("anyone.testnet", 0, 100_000);
    c.void_unrevealed(1);
}

#[test]
#[should_panic(expected = "previous round is still open")]
fn one_open_table_at_a_time() {
    let mut c = opened();
    ctx("house.cabal.testnet", NEAR, 5_000);
    c.open_round("TOAD".into(), commit(1, &sides(), "x"), seats(), funders(), 60, 60);
}

#[test]
#[should_panic(expected = "house only")]
fn only_the_house_opens_tables() {
    ctx(&accounts(0).to_string(), 0, 0);
    let mut c = Cabal::new(acc("house.cabal.testnet"));
    ctx("mallory.testnet", NEAR, 1_000);
    c.open_round("FROG".into(), commit(0, &sides(), "p"), seats(), funders(), 60, 60);
}

#[test]
#[should_panic(expected = "bond too small")]
fn bond_is_required() {
    ctx("house.cabal.testnet", 0, 0);
    let mut c = Cabal::new(acc("house.cabal.testnet"));
    ctx("house.cabal.testnet", NEAR / 100, 1_000);
    c.open_round("FROG".into(), commit(0, &sides(), "p"), seats(), funders(), 60, 60);
}

#[test]
#[should_panic(expected = "commitment must be 64 lowercase hex")]
fn commitment_shape_is_checked() {
    ctx("house.cabal.testnet", 0, 0);
    let mut c = Cabal::new(acc("house.cabal.testnet"));
    ctx("house.cabal.testnet", NEAR, 1_000);
    c.open_round("FROG".into(), "nothex".into(), seats(), funders(), 60, 60);
}

#[test]
fn preimage_is_stable() {
    assert_eq!(
        preimage(2, &[acc("a.testnet"), acc("b.testnet")], "s"),
        "cabal=2;sides=a.testnet,b.testnet;salt=s"
    );
}
