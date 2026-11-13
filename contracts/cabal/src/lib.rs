//! CABAL game contract.
//!
//! One memecoin market per round, plus the evidence the player needs to catch the insider.
//!
//! * **Market.** A linear bonding curve (`price = K * supply`) paid in native NEAR. The reserve always equals the
//!   curve's integral, so every holder can sell at any time and the contract stays solvent.
//! * **Evidence.** Every buy, sell, call and funding transfer is appended to an on-chain feed. Funding goes through
//!   `fund`, so "who paid for this fresh wallet" is a contract fact, not an off-chain claim.
//! * **Hidden role.** Before the market opens the house commits to `sha256(cabal seat, side wallets, salt)`. After the
//!   market closes it must reveal the preimage. The contract rejects a reveal that does not match the commitment or
//!   whose side wallets were never funded by the cabal wallet. A house that stays silent loses its bond.
//! * **Accusations.** Players stake NEAR on a seat. Correct accusers share the house bond, the wrong stakes, and the
//!   proceeds of the cabal's remaining bag, which the contract sells and burns.

use near_sdk::json_types::U128;
use near_sdk::store::LookupMap;
use near_sdk::{env, near, require, AccountId, NearToken, PanicOnDefault, Promise};

/// Curve slope in yoctoNEAR per chip². `price(s) = K * s`, so 1,000 chips in circulation means 0.001 NEAR per chip.
/// Must be even so `K * (b² - a²) / 2` is exact.
pub const K: u128 = 1_000_000_000_000_000_000;
pub const MAX_SUPPLY: u64 = 5_000_000;
pub const MIN_TRADE: u128 = 1_000_000_000_000_000_000_000; // 0.001 NEAR
pub const MIN_BOND: u128 = 100_000_000_000_000_000_000_000; // 0.1 NEAR
pub const MIN_STAKE: u128 = 10_000_000_000_000_000_000_000; // 0.01 NEAR
pub const MAX_STAKE: u128 = 100_000_000_000_000_000_000_000; // 0.1 NEAR
/// Stakes are weighed in units of 1e15 yocto so the proportional split cannot overflow u128.
pub const UNIT: u128 = 1_000_000_000_000_000;
pub const MAX_FEED: u32 = 400;
pub const MAX_ACCUSERS: usize = 40;
pub const MAX_SEATS: usize = 6;
pub const MAX_SIDES: usize = 6;
pub const MAX_TEXT: usize = 200;

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Open,
    Revealed,
    Voided,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Open,
    Buy,
    Sell,
    Call,
    Fund,
    Expose,
    Reveal,
    Confiscate,
    Void,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug)]
pub struct Seat {
    pub handle: String,
    pub wallet: AccountId,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug)]
pub struct Accuser {
    pub who: AccountId,
    pub seat: u8,
    pub stake: U128,
}

/// One line of the public evidence feed.
#[near(serializers = [borsh, json])]
#[derive(Clone, Debug)]
pub struct Entry {
    pub idx: u32,
    pub ts_ms: u64,
    pub block: u64,
    pub kind: Kind,
    pub actor: AccountId,
    /// Counterparty: the funded wallet for `Fund`.
    pub other: Option<AccountId>,
    /// Chips moved (`Buy`, `Sell`, `Confiscate`) or the accused seat (`Expose`).
    pub chips: u64,
    /// NEAR moved, in yoctoNEAR.
    pub near: U128,
    /// Call text for `Call`.
    pub text: Option<String>,
}

#[near(serializers = [borsh])]
#[derive(Clone)]
struct RoundData {
    id: u32,
    ticker: String,
    commitment: String,
    seats: Vec<Seat>,
    funders: Vec<AccountId>,
    opened_ms: u64,
    closes_ms: u64,
    reveal_by_ms: u64,
    supply: u64,
    reserve: u128,
    bond: u128,
    status: Status,
    cabal_seat: Option<u8>,
    sides: Vec<AccountId>,
    salt: Option<String>,
    accusers: Vec<Accuser>,
    feed_len: u32,
    pruned: u32,
    payout: u128,
}

#[near(serializers = [json])]
pub struct RoundView {
    pub id: u32,
    pub ticker: String,
    pub commitment: String,
    pub seats: Vec<Seat>,
    pub funders: Vec<AccountId>,
    pub opened_ms: u64,
    pub closes_ms: u64,
    pub reveal_by_ms: u64,
    pub supply: u64,
    pub reserve: U128,
    pub bond: U128,
    pub price: U128,
    pub status: Status,
    pub cabal_seat: Option<u8>,
    pub sides: Vec<AccountId>,
    pub salt: Option<String>,
    pub accusers: Vec<Accuser>,
    pub feed_len: u32,
    pub pruned: u32,
    pub payout: U128,
}

#[near(serializers = [json])]
pub struct BuyQuote {
    pub chips: u64,
    pub cost: U128,
    pub refund: U128,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Cabal {
    house: AccountId,
    latest: u32,
    rounds: LookupMap<u32, RoundData>,
    feed: LookupMap<(u32, u32), Entry>,
    chips: LookupMap<(u32, AccountId), u64>,
    funded_by: LookupMap<(u32, AccountId), AccountId>,
}

/// NEAR value of the curve between two supplies: `K * (hi² - lo²) / 2`.
pub fn curve_value(lo: u64, hi: u64) -> u128 {
    let (lo, hi) = (lo as u128, hi as u128);
    K * (hi * hi - lo * lo) / 2
}

/// Largest purchase of chips that `deposit` can pay for when `supply` chips already exist.
/// Returns `(chips, cost)`; `cost <= deposit` always.
pub fn quote_buy_chips(supply: u64, deposit: u128) -> (u64, u128) {
    let s0 = supply as u128;
    let s1 = (s0 * s0 + 2 * deposit / K).isqrt();
    let s1 = s1.min(MAX_SUPPLY as u128);
    let chips = (s1 - s0) as u64;
    (chips, curve_value(supply, supply + chips))
}

pub fn preimage(cabal_seat: u8, sides: &[AccountId], salt: &str) -> String {
    let list: Vec<&str> = sides.iter().map(|a| a.as_str()).collect();
    format!("cabal={};sides={};salt={}", cabal_seat, list.join(","), salt)
}

fn hex(bytes: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(H[(b >> 4) as usize] as char);
        s.push(H[(b & 15) as usize] as char);
    }
    s
}

fn pay(to: &AccountId, yocto: u128) {
    if yocto > 0 {
        let _ = Promise::new(to.clone()).transfer(NearToken::from_yoctonear(yocto));
    }
}

#[near]
impl Cabal {
    #[init]
    pub fn new(house: AccountId) -> Self {
        require!(!env::state_exists(), "already initialised");
        Self {
            house,
            latest: 0,
            rounds: LookupMap::new(b"r".to_vec()),
            feed: LookupMap::new(b"f".to_vec()),
            chips: LookupMap::new(b"c".to_vec()),
            funded_by: LookupMap::new(b"u".to_vec()),
        }
    }

    // ───────────────────────── house actions ─────────────────────────

    /// Open a table. The attached deposit is the house bond, paid to correct accusers if the cabal is caught
    /// (or to every accuser if the house never reveals).
    ///
    /// `funders` are the only accounts allowed to call `fund` (the seat wallets and the house's "exchange" wallet),
    /// which stops a player from pre-funding a side wallet to make the house's reveal fail.
    #[payable]
    pub fn open_round(
        &mut self,
        ticker: String,
        commitment: String,
        seats: Vec<Seat>,
        funders: Vec<AccountId>,
        play_secs: u32,
        reveal_secs: u32,
    ) -> u32 {
        require!(env::predecessor_account_id() == self.house, "house only");
        let bond = env::attached_deposit().as_yoctonear();
        require!(bond >= MIN_BOND, "bond too small");
        require!(seats.len() >= 2 && seats.len() <= MAX_SEATS, "2 to 6 seats");
        require!(ticker.len() >= 2 && ticker.len() <= 10, "ticker length");
        require!(
            commitment.len() == 64 && commitment.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "commitment must be 64 lowercase hex characters"
        );
        require!((30..=900).contains(&play_secs), "play_secs 30..900");
        require!((20..=600).contains(&reveal_secs), "reveal_secs 20..600");
        for (i, s) in seats.iter().enumerate() {
            require!(!s.handle.is_empty() && s.handle.len() <= 24, "handle length");
            require!(
                !seats[..i].iter().any(|o| o.wallet == s.wallet || o.handle == s.handle),
                "duplicate seat"
            );
        }
        let now = env::block_timestamp_ms();
        self.latest += 1;
        let id = self.latest;
        let mut r = RoundData {
            id,
            ticker,
            commitment,
            seats,
            funders,
            opened_ms: now,
            closes_ms: now + play_secs as u64 * 1000,
            reveal_by_ms: now + (play_secs + reveal_secs) as u64 * 1000,
            supply: 0,
            reserve: 0,
            bond,
            status: Status::Open,
            cabal_seat: None,
            sides: vec![],
            salt: None,
            accusers: vec![],
            feed_len: 0,
            pruned: 0,
            payout: 0,
        };
        self.push(&mut r, Kind::Open, env::predecessor_account_id(), None, 0, bond, None);
        self.rounds.insert(id, r);
        id
    }

    /// A seat posts a public call. Only that seat's wallet may sign it.
    pub fn post_call(&mut self, round: u32, seat: u8, text: String) {
        let mut r = self.round(round);
        require!(r.status == Status::Open && env::block_timestamp_ms() < r.closes_ms, "market closed");
        let s = r.seats.get(seat as usize).expect("no such seat");
        require!(env::predecessor_account_id() == s.wallet, "only the seat's own wallet can post its call");
        require!(!text.is_empty() && text.len() <= MAX_TEXT, "call text length");
        let actor = s.wallet.clone();
        self.push(&mut r, Kind::Call, actor, None, seat as u64, 0, Some(text));
        self.rounds.insert(round, r);
    }

    /// Send NEAR to a wallet and record who paid for it. First funder wins; only registered funders may call.
    #[payable]
    pub fn fund(&mut self, round: u32, to: AccountId) {
        let mut r = self.round(round);
        let who = env::predecessor_account_id();
        require!(r.status == Status::Open && env::block_timestamp_ms() < r.closes_ms, "market closed");
        require!(who == self.house || r.funders.contains(&who), "not a registered funder");
        let amount = env::attached_deposit().as_yoctonear();
        require!(amount > 0, "attach NEAR to fund");
        let key = (round, to.clone());
        if self.funded_by.get(&key).is_none() {
            self.funded_by.insert(key, who.clone());
        }
        self.push(&mut r, Kind::Fund, who, Some(to.clone()), 0, amount, None);
        self.rounds.insert(round, r);
        pay(&to, amount);
    }

    /// After the market closes, the house reveals the committed roles. A reveal that does not match the commitment,
    /// or names side wallets the cabal never funded, is rejected.
    pub fn reveal(&mut self, round: u32, cabal_seat: u8, sides: Vec<AccountId>, salt: String) {
        require!(env::predecessor_account_id() == self.house, "house only");
        let mut r = self.round(round);
        require!(r.status == Status::Open, "round already settled");
        let now = env::block_timestamp_ms();
        require!(now >= r.closes_ms, "market still open");
        require!(now <= r.reveal_by_ms, "reveal window passed; anyone may void the round");
        require!((cabal_seat as usize) < r.seats.len(), "no such seat");
        require!(!sides.is_empty() && sides.len() <= MAX_SIDES, "1 to 6 side wallets");
        for (i, s) in sides.iter().enumerate() {
            require!(!sides[..i].contains(s), "duplicate side wallet");
            require!(!r.seats.iter().any(|x| &x.wallet == s), "a seat wallet cannot be a side wallet");
        }
        require!(
            hex(&env::sha256(preimage(cabal_seat, &sides, &salt).as_bytes())) == r.commitment,
            "reveal does not match the commitment"
        );
        let cabal_wallet = r.seats[cabal_seat as usize].wallet.clone();
        for s in &sides {
            require!(
                self.funded_by.get(&(round, s.clone())) == Some(&cabal_wallet),
                "a side wallet was never funded by the cabal wallet"
            );
        }

        r.status = Status::Revealed;
        r.cabal_seat = Some(cabal_seat);
        r.sides = sides.clone();
        r.salt = Some(salt);
        self.push(&mut r, Kind::Reveal, self.house.clone(), Some(cabal_wallet), cabal_seat as u64, 0, None);

        let mut win_stake = 0u128;
        let mut lose_stake = 0u128;
        for a in &r.accusers {
            if a.seat == cabal_seat {
                win_stake += a.stake.0;
            } else {
                lose_stake += a.stake.0;
            }
        }

        if win_stake == 0 {
            // Nobody caught the cabal: it keeps its bag and the house keeps its bond.
            pay(&self.house, r.bond + lose_stake);
        } else {
            // Caught: sell and burn what the side wallets still hold; the proceeds join the pot.
            let mut confiscated = 0u128;
            for s in &sides {
                let c = self.chips.get(&(round, s.clone())).copied().unwrap_or(0);
                if c > 0 {
                    let value = curve_value(r.supply - c, r.supply);
                    r.supply -= c;
                    r.reserve -= value;
                    confiscated += value;
                    self.chips.insert((round, s.clone()), 0);
                    self.push(&mut r, Kind::Confiscate, s.clone(), None, c, value, None);
                }
            }
            let pot = r.bond + confiscated + lose_stake;
            let units = win_stake / UNIT;
            let mut paid_share = 0u128;
            let winners: Vec<Accuser> = r.accusers.iter().filter(|a| a.seat == cabal_seat).cloned().collect();
            for a in winners {
                let share = pot * (a.stake.0 / UNIT) / units;
                paid_share += share;
                pay(&a.who, a.stake.0 + share);
            }
            pay(&self.house, pot - paid_share); // rounding dust only
            r.payout = pot;
        }
        self.rounds.insert(round, r);
    }

    /// If the house misses the reveal deadline, anyone can void the round. Every accuser gets their stake back and
    /// the house bond is split among them by stake.
    pub fn void_unrevealed(&mut self, round: u32) {
        let mut r = self.round(round);
        require!(r.status == Status::Open, "round already settled");
        require!(env::block_timestamp_ms() > r.reveal_by_ms, "reveal window still open");
        r.status = Status::Voided;
        let total: u128 = r.accusers.iter().map(|a| a.stake.0).sum();
        let units = total / UNIT;
        if units == 0 {
            pay(&self.house, r.bond);
        } else {
            let mut paid = 0u128;
            for a in r.accusers.clone() {
                let share = r.bond * (a.stake.0 / UNIT) / units;
                paid += share;
                pay(&a.who, a.stake.0 + share);
            }
            pay(&self.house, r.bond - paid);
            r.payout = r.bond;
        }
        let caller = env::predecessor_account_id();
        let bond = r.bond;
        self.push(&mut r, Kind::Void, caller, None, 0, bond, None);
        self.rounds.insert(round, r);
    }

    /// Free storage held by old rounds. Anyone may call it for settled rounds that are at least three tables old.
    pub fn prune(&mut self, round: u32, max: u32) -> u32 {
        let mut r = self.round(round);
        require!(r.status != Status::Open && round + 3 <= self.latest, "round is too recent to prune");
        let end = r.feed_len.min(r.pruned + max.min(100));
        let mut removed = 0;
        while r.pruned < end {
            self.feed.remove(&(round, r.pruned));
            r.pruned += 1;
            removed += 1;
        }
        self.rounds.insert(round, r);
        removed
    }

    // ───────────────────────── player actions ─────────────────────────

    /// Buy as many chips as the attached NEAR pays for. Change is returned. Reverts if fewer than `min_chips`.
    #[payable]
    pub fn buy(&mut self, round: u32, min_chips: u64) -> u64 {
        let mut r = self.round(round);
        require!(r.status == Status::Open && env::block_timestamp_ms() < r.closes_ms, "market closed");
        let deposit = env::attached_deposit().as_yoctonear();
        require!(deposit >= MIN_TRADE, "minimum trade is 0.001 NEAR");
        let (chips, cost) = quote_buy_chips(r.supply, deposit);
        require!(chips >= 1 && chips >= min_chips, "price moved: fewer chips than your minimum");
        let who = env::predecessor_account_id();
        r.supply += chips;
        r.reserve += cost;
        let key = (round, who.clone());
        let bal = self.chips.get(&key).copied().unwrap_or(0);
        self.chips.insert(key, bal + chips);
        self.push(&mut r, Kind::Buy, who.clone(), None, chips, cost, None);
        self.rounds.insert(round, r);
        pay(&who, deposit - cost);
        chips
    }

    /// Sell chips back to the curve at any time, including after the market closes.
    pub fn sell(&mut self, round: u32, chips: u64, min_proceeds: U128) -> U128 {
        let mut r = self.round(round);
        require!(chips >= 1, "sell at least one chip");
        let who = env::predecessor_account_id();
        let key = (round, who.clone());
        let bal = self.chips.get(&key).copied().unwrap_or(0);
        require!(bal >= chips, "you do not hold that many chips");
        let value = curve_value(r.supply - chips, r.supply);
        require!(value >= min_proceeds.0, "price moved: less NEAR than your minimum");
        require!(r.reserve >= value, "reserve invariant violated");
        r.supply -= chips;
        r.reserve -= value;
        self.chips.insert(key, bal - chips);
        self.push(&mut r, Kind::Sell, who.clone(), None, chips, value, None);
        self.rounds.insert(round, r);
        pay(&who, value);
        U128(value)
    }

    /// Accuse a seat. The stake is returned with a share of the pot if the seat is the cabal, and lost otherwise.
    #[payable]
    pub fn expose(&mut self, round: u32, seat: u8) {
        let mut r = self.round(round);
        let who = env::predecessor_account_id();
        require!(r.status == Status::Open && env::block_timestamp_ms() < r.closes_ms, "accusations are closed");
        require!((seat as usize) < r.seats.len(), "no such seat");
        let stake = env::attached_deposit().as_yoctonear();
        require!((MIN_STAKE..=MAX_STAKE).contains(&stake), "stake must be between 0.01 and 0.1 NEAR");
        require!(r.accusers.len() < MAX_ACCUSERS, "too many accusations on this table");
        require!(!r.accusers.iter().any(|a| a.who == who), "you already accused someone this round");
        require!(
            who != self.house && !r.funders.contains(&who) && !r.seats.iter().any(|s| s.wallet == who),
            "table wallets cannot accuse"
        );
        r.accusers.push(Accuser { who: who.clone(), seat, stake: U128(stake) });
        self.push(&mut r, Kind::Expose, who, None, seat as u64, stake, None);
        self.rounds.insert(round, r);
    }

    // ───────────────────────── views ─────────────────────────

    pub fn house(&self) -> AccountId {
        self.house.clone()
    }

    pub fn latest_round(&self) -> u32 {
        self.latest
    }

    pub fn get_round(&self, round: u32) -> Option<RoundView> {
        self.rounds.get(&round).map(|r| RoundView {
            id: r.id,
            ticker: r.ticker.clone(),
            commitment: r.commitment.clone(),
            seats: r.seats.clone(),
            funders: r.funders.clone(),
            opened_ms: r.opened_ms,
            closes_ms: r.closes_ms,
            reveal_by_ms: r.reveal_by_ms,
            supply: r.supply,
            reserve: U128(r.reserve),
            bond: U128(r.bond),
            price: U128(K * r.supply as u128),
            status: r.status,
            cabal_seat: r.cabal_seat,
            sides: r.sides.clone(),
            salt: r.salt.clone(),
            accusers: r.accusers.clone(),
            feed_len: r.feed_len,
            pruned: r.pruned,
            payout: U128(r.payout),
        })
    }

    pub fn get_feed(&self, round: u32, from: u32, limit: u32) -> Vec<Entry> {
        let Some(r) = self.rounds.get(&round) else { return vec![] };
        let start = from.max(r.pruned);
        let end = r.feed_len.min(start.saturating_add(limit.min(200)));
        (start..end).filter_map(|i| self.feed.get(&(round, i)).cloned()).collect()
    }

    pub fn chips_of(&self, round: u32, account_id: AccountId) -> u64 {
        self.chips.get(&(round, account_id)).copied().unwrap_or(0)
    }

    pub fn funded_by_of(&self, round: u32, account_id: AccountId) -> Option<AccountId> {
        self.funded_by.get(&(round, account_id)).cloned()
    }

    pub fn quote_buy(&self, round: u32, deposit: U128) -> BuyQuote {
        let supply = self.rounds.get(&round).map(|r| r.supply).unwrap_or(0);
        let (chips, cost) = quote_buy_chips(supply, deposit.0);
        BuyQuote { chips, cost: U128(cost), refund: U128(deposit.0.saturating_sub(cost)) }
    }

    pub fn quote_sell(&self, round: u32, chips: u64) -> U128 {
        let supply = self.rounds.get(&round).map(|r| r.supply).unwrap_or(0);
        if chips > supply {
            return U128(0);
        }
        U128(curve_value(supply - chips, supply))
    }

    /// The exact string the commitment hashes, so clients can verify a reveal in the browser.
    pub fn commitment_preimage(&self, cabal_seat: u8, sides: Vec<AccountId>, salt: String) -> String {
        preimage(cabal_seat, &sides, &salt)
    }

    // ───────────────────────── internals ─────────────────────────

    fn round(&self, id: u32) -> RoundData {
        self.rounds.get(&id).cloned().expect("no such round")
    }

    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        r: &mut RoundData,
        kind: Kind,
        actor: AccountId,
        other: Option<AccountId>,
        chips: u64,
        near: u128,
        text: Option<String>,
    ) {
        require!(r.feed_len < MAX_FEED, "table is full");
        let e = Entry {
            idx: r.feed_len,
            ts_ms: env::block_timestamp_ms(),
            block: env::block_height(),
            kind,
            actor,
            other,
            chips,
            near: U128(near),
            text,
        };
        self.feed.insert((r.id, r.feed_len), e);
        r.feed_len += 1;
    }
}

#[cfg(test)]
mod tests;
