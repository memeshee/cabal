# Submission notes (NEAR HACKATHON)

Nothing in this file is sent anywhere automatically. The human submits.

## Facts the form needs

| Field | Value |
|---|---|
| Project name | CABAL |
| Tracks | `games`, `memecoins` |
| Repository | https://github.com/memeshee/cabal (must be public before the deadline) |
| Live demo | https://cabal-near.fly.dev |
| Contract | `cabal-game-bfbf.testnet` |
| Demo post | the X post the human publishes (draft below) |
| Lead name, email, team size, payout NEAR account | **the human decides; never guessed** |

Deadline: **2026-11-14 23:59 UTC**. One submission per repository, and at most two projects per person (the README names the author's other two submissions).

## What to say in the "what they built" field (20 to 2000 characters)

CABAL is a three-minute memecoin party game on NEAR testnet. Four AI callers hype the same coin and one of them is the cabal: it pays for fresh wallets, buys through them before its own call, then sells into the copy-traders who followed. Every wallet, trade and call is a real NEAR transaction, so the evidence is on-chain, and the board draws it as strings from whoever paid for a wallet. Players question the callers (who deny everything), then stake play money on a name. The house cannot cheat: before anyone trades, the contract stores a SHA-256 commitment to the cabal's identity and side wallets. At the end the house must reveal it, the contract rejects a mismatch or side wallets the cabal never funded, and a silent house forfeits its bond to the accusers. The browser re-hashes the reveal itself. Built with a Rust near-sdk contract (27 tests), a Rust Axum house service (27 tests) and a Next.js app. NEAR provides cheap fast accounts for every NPC, a contract that enforces the commitment and payouts, and NEAR AI Cloud for the callers' dialogue.

## X post draft

Keep the first line understandable to someone who has never heard of the project. Attach `docs/demo/cabal-demo.mp4` (1:54).

```
One of these four callers is dumping on you.

CABAL is a 3-minute memecoin game. The cabal pays for fresh wallets, buys through them before its own call, then sells into everyone who followed. The chain shows who paid for them. The callers will lie.

The house can't cheat: the contract commits to who the cabal is before anyone trades.

Built at NEAR HACKATHON @NEARProtocol #NEARHackathon

Play (no wallet, testnet play money): https://cabal-near.fly.dev
Code: https://github.com/memeshee/cabal

Which caller would you accuse?
```

Optional tags: @NearLegion @NearBuilders.

## Before submitting

1. Make the repository public (the human does this; it is private now).
2. Open the live demo once to wake the machine (it scales to zero; a cold start takes about 6 seconds).
3. Post the X thread, then send the form with the repo link and the post link.
4. Keep the demo up until at least 2026-11-18.
