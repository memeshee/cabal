import { describe, expect, it } from "vitest";
import { autopsy, shareText, standings, trailOf } from "./autopsy";
import { claimHighlight, descendants, earlyBuyers, layoutBoard } from "./board";
import type { Dossier, Entry, TableView } from "./types";

const OPEN = 5_000_000;
const seats = ["kaito", "mira", "dex", "oren"].map((h, i) => ({ handle: `@${h}`, wallet: `${h}w`, tagline: "", color: ["#f2c230", "#58b6c9", "#ff6a3d", "#b6a1ff"][i] }));
let idx = 0;
const e = (sec: number, kind: Entry["kind"], actor: string, extra: Partial<Entry> = {}): Entry => ({
  idx: idx++, ts_ms: OPEN + sec * 1000, block: idx, kind, actor, other: null, chips: 10, near: "0", text: null, ...extra,
});
const dos = (wallet: string, funder: string, funderSeat: number | null, root: number | null, funded: number): Dossier => ({
  wallet, funder, funder_seat: funderSeat, root_seat: root, funded_ms: OPEN + funded * 1000, funded_idx: 0,
  first_buy_ms: null, first_buy_idx: null, chips_bought: 0, first_sell_ms: null, chips_sold: 0, funder_call_ms: null, lead_ms: null, dumped_after_ms: null,
});

function view(tier: "rookie" | "hop" | "ghost"): TableView {
  idx = 0;
  const cabalPays = tier === "hop" ? "relay" : tier === "ghost" ? "exchange" : "miraw";
  const feed: Entry[] = [
    e(0, "Open", "house"),
    ...(tier === "ghost" ? [e(2, "Fund", "miraw", { other: "exchange" }), e(3, "Fund", "kaitow", { other: "exchange" })] : []),
    ...(tier === "hop" ? [e(3, "Fund", "miraw", { other: "relay" })] : []),
    e(10, "Fund", cabalPays, { other: "s1" }),
    e(12, "Fund", cabalPays, { other: "s2" }),
    e(34, "Buy", "s1", { chips: 60 }),
    e(36, "Buy", "s2", { chips: 55 }),
    e(52, "Call", "miraw", { text: "$FROG" }),
    e(60, "Sell", "s1", { chips: 60 }),
    e(63, "Sell", "s2", { chips: 55 }),
    e(70, "Call", "orenw", { text: "$FROG" }),
  ];
  const dossiers: Dossier[] = [
    ...(tier === "hop" ? [dos("relay", "miraw", 1, 1, 3)] : []),
    dos("s1", cabalPays, tier === "rookie" ? 1 : null, tier === "ghost" ? null : 1, 10),
    dos("s2", cabalPays, tier === "rookie" ? 1 : null, tier === "ghost" ? null : 1, 12),
  ];
  return {
    key: "t", phase: "revealed", round: 1, ticker: "FROG", tier, players: 1, demo: false, note: "", error: null, seats, labels: { exchange: "exchange", relay: "relay" },
    round_view: { id: 1, ticker: "FROG", commitment: "c", seats: seats.map((s) => ({ handle: s.handle, wallet: s.wallet })), funders: ["kaitow", "miraw", "dexw", "orenw", "exchange", "relay"], opened_ms: OPEN, closes_ms: OPEN + 130_000, reveal_by_ms: OPEN + 190_000, supply: 0, reserve: "0", bond: "0", price: "0", status: "Revealed", cabal_seat: 1, sides: ["s1", "s2"], salt: "x", accusers: [], feed_len: feed.length, pruned: 0, payout: "0" },
    feed, feed_total: feed.length, chat: [], chat_total: 0, dossiers, verdict: null,
    reveal: { cabal_seat: 1, cabal_handle: "@mira", sides: ["s1", "s2"], salt: "x", preimage: "p", commitment: "c", recomputed: "c", matches: true, hash: null },
    elapsed_ms: null, play_ms: 130_000, reveal_ms: 60_000, contract: "c.testnet", explorer: "https://x",
  };
}

describe("the board draws a relay as two legs", () => {
  it("starts a relay-paid wallet's string on the relay's row, not on a caller's lane", () => {
    const v = view("hop");
    const L = layoutBoard(v, 1000);
    const relay = L.rows.find((r) => r.wallet === "relay")!;
    const side = L.rows.find((r) => r.wallet === "s1")!;
    expect(relay.funderIsKol).toBe(true);
    expect(side.funderRow).toBe(L.rows.indexOf(relay));
    expect(side.stringPath.startsWith(`M ${side.fundedX} ${relay.y}`)).toBe(true);
    expect(relay.stringPath.startsWith(`M ${relay.fundedX} ${L.laneY[1]}`)).toBe(true);
  });

  it("draws every caller's top-up of the exchange as a link between lanes (the Ghost noise)", () => {
    const L = layoutBoard(view("ghost"), 1000);
    expect(L.laneLinks.map((l) => l.seat)).toEqual([1, 0]);
    expect(L.laneLinks.every((l) => l.toY === L.laneY[4])).toBe(true);
    expect(layoutBoard(view("rookie"), 1000).laneLinks).toEqual([]);
  });
});

describe("claim highlighting", () => {
  it("lights the wallets a caller paid for, through a relay, and not those paid by the exchange", () => {
    expect([...descendants(view("rookie"), 1)].sort()).toEqual(["s1", "s2"]);
    expect([...descendants(view("hop"), 1)].sort()).toEqual(["relay", "s1", "s2"]);
    expect([...descendants(view("ghost"), 1)]).toEqual([]);
    expect([...descendants(view("rookie"), 0)]).toEqual([]);
  });

  it("lights the wallets that bought in the half minute before a call", () => {
    const v = view("rookie");
    expect([...earlyBuyers(v, 1)].sort()).toEqual(["s1", "s2"]);
    expect([...earlyBuyers(v, 3)]).toEqual([]);
    expect([...claimHighlight(v, 1, 2)]).toEqual([]);
  });
});

describe("the autopsy", () => {
  it("names the trail, the lead and the dump for each side wallet", () => {
    const a = autopsy(view("hop"))!;
    expect(a.cabal).toBe("@mira");
    expect(a.callAtMs).toBe(OPEN + 52_000);
    expect(a.rows[0].trail).toEqual(["relay", "@mira"]);
    expect(a.rows[0].boughtBeforeCallMs).toBe(18_000);
    expect(a.rows[0].soldAfterCallMs).toBe(8_000);
    expect(a.headline).toContain("before @mira's call");
    expect(a.tactic).toContain("relay");
  });

  it("explains the Ghost tier as a timing puzzle", () => {
    const a = autopsy(view("ghost"))!;
    // After the reveal the whole trail is shown: the exchange, and the caller who paid it first.
    expect(a.rows[0].trail).toEqual(["exchange", "@mira"]);
    expect(a.tactic).toContain("first of four callers to top up the exchange");
    expect(a.clues[0]).toContain("noise");
  });

  it("does not tell a Ghost player to check funding against a claim", () => {
    const v = view("ghost");
    v.chat = [{ id: 0, ts_ms: 0, who: "@mira", text: "No.", kind: "kol", via: null, claim_q: 0, claim_seat: 1, claim_tag: "no other wallets", claim_true: false }];
    const clues = autopsy(v)!.clues.join(" ");
    expect(clues).toContain("Only the timing could");
    expect(clues).not.toContain("who they paid");
    const r = view("rookie");
    r.chat = v.chat;
    expect(autopsy(r)!.clues.join(" ")).toContain("who they paid");
  });

  it("walks a trail back to the caller and stops there", () => {
    expect(trailOf(view("rookie"), "s1")).toEqual(["@mira"]);
    expect(trailOf(view("hop"), "s2")).toEqual(["relay", "@mira"]);
  });

  it("returns nothing before the reveal", () => {
    const v = { ...view("rookie"), reveal: null };
    expect(autopsy(v)).toBeNull();
  });

  it("writes a share line for a win, a loss and a bystander", () => {
    const v = view("hop");
    expect(shareText(v, true, 47)).toContain("exposed @mira");
    expect(shareText(v, true, 47)).toContain("in 47s");
    expect(shareText(v, false, null)).toContain("fooled me");
    expect(shareText(v, null, null)).toContain("was the cabal");
  });
});

describe("standings", () => {
  it("ranks correct accusers by speed ahead of wrong ones", () => {
    const v = view("rookie");
    v.round_view!.accusers = [
      { who: "slow", seat: 1, stake: "1", at_ms: 80_000 },
      { who: "wrong", seat: 2, stake: "1", at_ms: 10_000 },
      { who: "fast", seat: 1, stake: "1", at_ms: 41_000 },
    ];
    const s = standings(v, "slow");
    expect(s.map((x) => x.who)).toEqual(["fast", "slow", "wrong"]);
    expect(s.map((x) => x.correct)).toEqual([true, true, false]);
    expect(s[1].isYou).toBe(true);
    expect(s[0].seconds).toBe(41);
  });
  it("is empty before the reveal", () => {
    const v = { ...view("rookie"), reveal: null };
    expect(standings(v, null)).toEqual([]);
  });
});
