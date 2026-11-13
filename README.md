# CABAL

**One of them is dumping on you.**

CABAL is a three-minute memecoin party game on NEAR. Four AI influencers hype the same coin. One of them is the *cabal*: it funds fresh wallets, buys through them *before* its own call, then sells into the copy-traders who followed. Every wallet, trade and call is a real NEAR transaction, so the evidence is on-chain. The influencers will lie to you. The chain won't.

> **People lie. The chain doesn't.**

- **Live demo:** https://cabal-near.fly.dev (NEAR testnet, play money, no wallet or sign-up)
- **Contract:** [`cabal-game-bfbf.testnet`](https://testnet.nearblocks.io/address/cabal-game-bfbf.testnet)
- **Tracks:** Games With Real Economies (`games`) and Memecoins With Mechanics (`memecoins`)

![A live table: the board, the tape, the suspects and the room](docs/screenshots/table-mid.png)

---

## Why this exists

Crypto traders are losing money to a pattern they can't see in time. In the week this was built, Bubblemaps published an investigation into a top "social trader" who pulled about **$1.7M** from followers through roughly 30 linked wallets: a public wallet attracts copy-traders, side wallets dump on them. Bitquery's analysis of 8.2M pump.fun launches found developers and bundled wallets made about **$579M**, against **$31.5M** for snipers.

The pattern is readable on-chain. Almost nobody reads it, because it takes a lot of practice and the practice is expensive. CABAL turns the practice into a game with the same structure as the real thing, with play money.

## How a table works

1. **Take a seat.** You get a throwaway testnet account with play money. No wallet.
2. **The house commits.** Before anyone trades, the contract stores `sha256(cabal seat, side wallets, salt)`. The house cannot change its mind later.
3. **Read the board.** Four callers (`@kaito`, `@mira`, `@dex`, `@oren`) each post one call. The board draws every wallet someone paid for as a string from the payer, with a green square where it bought and an orange diamond where it sold. The cabal's pattern is geometry: **a wallet its caller paid for, buying before the call and selling right after it.**
4. **Question them.** Ask any caller anything. They stay in character and deny everything. Honest ones sound like the guilty one.
5. **Expose.** Stake 0.01 to 0.1 play-NEAR on a name before the market closes.
6. **The reveal.** The house reveals the roles. The contract checks the hash and that each side wallet really was paid for by the cabal's wallet. If you were right, you get your stake back plus a share of the pot (the house bond, the cabal's burned bag, and wrong stakes). If the house stays silent, anyone can void the table and the house bond goes to the accusers.

There are red herrings on purpose: a whale funded by the exchange that buys before everyone (ruled out by the funding link), and a friend wallet paid for by an honest caller that buys *after* that caller's call (ruled out by timing).

## Try it (for judges and reviewers)

These are exact steps with the expected result.

### A. Play the pinned demo table (3 minutes)

1. Open https://cabal-near.fly.dev and click **Play the demo table**.
   *Expected:* after about 15 to 40 seconds ("Dealing the table") a table opens with a countdown from 03:00.
2. Watch the **Board**. Three things happen to `@mira` in order (times are seconds after the table opens, give or take a second of chain latency):
   - at about **34s to 41s** she pays for three fresh wallets (red strings starting on her lane);
   - at about **50s to 57s** those three wallets each buy (green squares);
   - at about **65s to 69s** she posts her call (a pin on her lane); at about **73s to 82s** the three wallets sell (orange diamonds).

   *Expected:* three wallet rows whose red string starts on `@mira`'s lane, with green squares to the left of her pin and orange diamonds to the right of it.
3. Click one of those wallet rows (or a wallet name in **The tape**).
   *Expected:* a **Wallet file** showing "paid for by @mira", a buy about 15s before her call and a sell about 7s after it.
4. Compare with the two red herrings. The row with a dashed blue string was paid for by the exchange and buys early (ruled out by who paid for it). The row paid for by `@oren` appears later, at about 98s, and only buys at about 130s, after `@oren`'s own call at about 118s (ruled out by timing).
5. Once the evidence is in (any time after about 80s), press **Expose** next to `@mira` and confirm the stake.
   *Expected:* a toast "You accused @mira" with a link to the transaction.
6. Wait for the clock to run out.
   *Expected:* a **Reveal** screen with the stamp **EXPOSED**, `@mira` named, and a *Proof* panel: the commitment recorded before play, the revealed preimage, and a hash your browser computed itself, marked "It matches the commitment".
7. Click **replay this table** (or open **Past tables**).
   *Expected:* the same round rebuilt purely from the contract, with no house memory involved.

The demo table is always the same deal (seed pinned in `services/house/src/plan.rs` and enforced by a test), so these timings hold. The normal **Take a seat** button deals a random table with a random cabal.

### B. Prove the table is winnable from public data alone (headless, about 5 minutes)

```bash
cd services/house
cargo run -- selftest
```

This plays a full table against the live testnet, runs a *solver* that sees only the public feed, accuses with one funded guest while a second guest accuses the wrong seat, and then checks: the solver named the true cabal before the market closed, the revealed preimage hashes to the earlier commitment, the correct accuser was paid, the wrong accuser lost their stake, and a replay from the chain matches. It ends with `SELFTEST PASSED`.

## What is real and what is not

| Thing | Status |
|---|---|
| Wallets, trades, calls, funding, accusations, reveal, payouts | **Real NEAR testnet transactions.** Every row on the tape is a contract event. |
| The cabal commitment and its check | **Real, enforced by the contract** (`reveal` rejects a mismatched preimage, a late reveal, or side wallets the cabal never funded). |
| The coin and its price | **Real bonding curve in the contract** (`price = K × supply`); the reserve always equals the curve integral, so every holder can sell. |
| Money | **Testnet NEAR only.** The contract and server refuse any other network. |
| The four callers, the copy-traders, the whale and the friend wallet | **Simulated by the house.** They are real accounts making real transactions, but their behavior comes from a seeded script, not from independent players. |
| The callers' chat lines | **Generated text** (see "Dialogue" below). Chat is off-chain and never affects outcomes. |
| Guest accounts | **Custodial throwaway testnet accounts held by the server**, derived from a token in your browser. They are not your own keys. Do not reuse them. |
| NEAR Intents | **Not used.** There is no Intents testnet and this game does not need cross-chain settlement. |
| Multiplayer | **Not implemented.** Several people can sit at different tables at once; a table has one human seat. |

## Innovation

- **A deduction game whose clues are on-chain facts.** The usual hidden-role game relies on a dealer you trust. Here the dealer is bound by a commitment in a contract and punished by the same contract if it cheats.
- **Evidence you can compute, not guess.** The board is built from the contract's own feed. A solver that sees only that feed names the cabal before the market closes (the self-test proves it), so a table is winnable by reasoning alone.
- **The model never knows the secret.** The LLM writes dialogue from public facts only. The prompt builder takes no role input, so there is nothing for an attacker to extract. Tests assert prompts never mention roles and that prompts are identical across seats apart from the persona line. The API also withholds timing derivations and the solver's verdict until the table is final, so the answer cannot be read out of the JSON.

## Architecture

```
 browser (Next.js)  ──/api──▶  Rust house (Axum)  ──RPC──▶  NEAR testnet
   board, tape, chat            deals tables, plays           cabal contract
   Web Crypto re-checks         KOLs + crowd, indexes         (near-sdk 5)
   the commitment               the feed, reveals
                                  │
                                  └──▶ NEAR AI Cloud / OpenAI-compatible endpoint (dialogue only)
```

- **`contracts/cabal`** (Rust, near-sdk 5.30): the bonding-curve market, the evidence feed, accusations, the commitment/reveal/void logic. 27 unit tests, including swapped-cabal, changed-side-wallet, late-reveal and wrong-funder attacks.
- **`services/house`** (Rust, Axum, Tokio, `near-api`): the deal is a pure seeded function (`plan.rs`), executed against the chain by `game.rs`. All house wallets and all guest accounts are derived from one master secret, so the service stores no keys and can finish a table after a restart. 26 unit tests.
- **`apps/web`** (Next.js 16, React 19, TypeScript): the evidence board is hand-written SVG, laid out by a pure, tested module (`lib/board.ts`). 12 tests.

Key decisions and tradeoffs are written next to the code; the two that matter most:

- **Commit/reveal over a trusted dealer.** The house does hold the secret during play (the demo needs hidden roles). The contract makes cheating costly and detectable instead of impossible: a lying or silent house loses its bond.
- **Custodial throwaway guests.** It removes every wallet prompt, which is the point of the game, at the cost of players not holding their own keys. That is fine for play money and would not be for real funds.

## Dialogue

The callers' lines come from a chat model, with scripted lines as a guaranteed fallback. The provider is any OpenAI-compatible endpoint, chosen by environment variables: [NEAR AI Cloud](https://docs.near.ai/cloud/quickstart) is tried first when `NEAR_AI_API_KEY` is set, a second endpoint is used if that fails, and scripted lines are used if both fail or a daily cap is hit. Replies are filtered (no leaks, no role talk, length capped). The chat panel does not show which provider answered, and nothing in the game depends on it.

## Run it yourself

Prerequisites: Rust (stable, with the `wasm32-unknown-unknown` target), Node 22, pnpm, and a funded NEAR testnet account for the house.

```bash
# 1. tests
cargo test -p cabal            # contract
cargo test -p house            # house
pnpm install && pnpm web:test  # web

# 2. build and deploy your own contract (see .env.example)
cargo build --release --target wasm32-unknown-unknown -p cabal
near create-account <game>.testnet --useFaucet
near create-account <house>.testnet --useFaucet
near deploy <game>.testnet target/wasm32-unknown-unknown/release/cabal.wasm \
  --initFunction new --initArgs '{"house":"<house>.testnet"}'

# 3. run
cp .env.example .env            # set CABAL_CONTRACT and HOUSE_ACCOUNT
cd services/house && PORT=8080 cargo run --release
API_URL=http://127.0.0.1:8080 pnpm --dir apps/web build && node apps/web/.next/standalone/apps/web/server.js
```

The house needs a few tens of testnet NEAR: it funds about 20 wallets per table slot, grants guests play money, and posts a 0.1 NEAR bond per table. `fly.toml` and `Dockerfile` deploy the whole stack as one image.

## Security and reliability

- **Contract:** checked integer math on `u128`; the reserve is an exact curve integral; slippage guards on `buy` and `sell`; accusations, calls and funding are role-gated (a seat can only post its own call; only registered funders can `fund`; table wallets cannot accuse); feeds and accusers are bounded; storage is freed by `prune`.
- **Fairness:** the commitment is checked on-chain; side wallets must be provably funded by the cabal wallet; a missed reveal window lets anyone void the table and pay accusers from the bond.
- **House:** testnet only (refuses mainnet); per-IP and daily limits on new players and tables; guest tokens are HMAC-tagged so arbitrary tokens cannot make the server derive keys; player text is stripped and length-capped before it reaches a prompt; model output is filtered; a model spend cap protects the account; pre-inclusion `InvalidTransaction` rejections are retried (and nothing else is).
- **Failure handling:** if the RPC stalls a step is logged and the table carries on; if dialogue fails the callers use scripted lines; if the house restarts mid-table it re-derives the table and reveals; if the reveal fails, the contract's void rule takes over.

## Limitations and roadmap

- Testnet only, one human seat per table, callers are scripted rather than independent agents.
- The cabal's pattern is fixed in shape (fund, buy early, call, dump). Harder tables with decoys that mimic the pattern are the next difficulty tier.
- Next: shared tables where several humans race to expose, a "play the cabal" mode where AI detectives try to catch you, and a daily seeded table with a shareable result.
- Public-feed labeling from accusations could eventually train real detection tools, which is a path beyond a game.

## Hackathon notes

Built during the NEAR HACKATHON window (Nov 2026). The author also submitted two other independent projects to this hackathon: **Bonded** (performance bonds for AI agents) and **That Invoice** (invoices as NEAR accounts). They share no code with CABAL beyond generic patterns for talking to the NEAR RPC. Libraries and AI-assisted code are used throughout; the dependency list is in `Cargo.toml` and `apps/web/package.json`.

## License

MIT
