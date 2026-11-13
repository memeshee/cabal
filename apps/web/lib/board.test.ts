import { describe, expect, it } from "vitest";
import { layoutBoard, walletFacts, entryLabel } from "./board";
import { clock, fmtNear, near, offset, shortAcct } from "./format";
import type { Dossier, Entry, TableView } from "./types";

const OPEN = 1_000_000;
const seats = ["kaito", "mira", "dex", "oren"].map((h, i) => ({ handle: `@${h}`, wallet: `${h}w`, tagline: "", color: ["#f2c230", "#58b6c9", "#ff6a3d", "#b6a1ff"][i] }));

let idx = 0;
const e = (sec: number, kind: Entry["kind"], actor: string, extra: Partial<Entry> = {}): Entry => ({
  idx: idx++, ts_ms: OPEN + sec * 1000, block: idx, kind, actor, other: null, chips: 10, near: "0", text: null, ...extra,
});

function scenario(): TableView {
  idx = 0;
  const feed: Entry[] = [
    e(0, "Open", "house"),
    e(5, "Fund", "exchange", { other: "whale" }),
    e(11, "Buy", "whale"),
    // cabal = mira: funds s1, s1 buys 20s before mira's call, dumps 8s after it
    e(20, "Fund", "miraw", { other: "s1" }),
    e(34, "Buy", "s1"),
    e(54, "Call", "miraw", { text: "$FROG loading" }),
    e(62, "Sell", "s1"),
    // honest friend: funded by oren, buys only after oren's call
    e(70, "Fund", "orenw", { other: "f1" }),
    e(90, "Call", "orenw", { text: "$FROG thesis" }),
    e(102, "Buy", "f1"),
    e(95, "Buy", "crowd1"), e(97, "Sell", "crowd2"),
  ];
  const d = (wallet: string, funder: string, funderSeat: number | null, funded: number): Dossier => ({
    wallet, funder, funder_seat: funderSeat, root_seat: null, funded_ms: OPEN + funded * 1000, funded_idx: 0,
    first_buy_ms: null, first_buy_idx: null, chips_bought: 0, first_sell_ms: null, chips_sold: 0, funder_call_ms: null, lead_ms: null, dumped_after_ms: null,
  });
  return {
    key: "t", phase: "playing", round: 1, ticker: "FROG", tier: "rookie", players: 1, demo: false, note: "", error: null, seats, labels: { exchange: "exchange" },
    round_view: { id: 1, ticker: "FROG", commitment: "c", seats: seats.map((s) => ({ handle: s.handle, wallet: s.wallet })), funders: ["exchange"], opened_ms: OPEN, closes_ms: OPEN + 180_000, reveal_by_ms: OPEN + 270_000, supply: 0, reserve: "0", bond: "0", price: "0", status: "Open", cabal_seat: null, sides: [], salt: null, accusers: [], feed_len: feed.length, pruned: 0, payout: "0" },
    feed, feed_total: feed.length, chat: [], chat_total: 0,
    dossiers: [d("whale", "exchange", null, 5), d("s1", "miraw", 1, 20), d("f1", "orenw", 3, 70)],
    verdict: null, reveal: null, elapsed_ms: 100_000, play_ms: 130_000, reveal_ms: 90_000, contract: "c.testnet", explorer: "https://x",
  };
}

describe("board geometry", () => {
  const v = scenario();
  const L = layoutBoard(v, 1000);

  it("places time left to right inside the plot area", () => {
    for (const r of L.rows) {
      expect(r.fundedX).toBeGreaterThanOrEqual(L.padL);
      expect(r.fundedX).toBeLessThanOrEqual(L.padL + L.innerW);
    }
    expect(L.xOf(OPEN)).toBe(L.padL);
    expect(L.xOf(OPEN + 180_000)).toBe(L.padL + L.innerW);
  });

  it("draws the front-run: the funded wallet's buy lands left of its funder's pin, the sell right of it", () => {
    const pin = L.pins.find((p) => p.seat === 1)!;
    const row = L.rows.find((r) => r.wallet === "s1")!;
    const buy = row.marks.find((m) => m.kind === "buy")!;
    const sell = row.marks.find((m) => m.kind === "sell")!;
    expect(buy.x).toBeLessThan(pin.x);
    expect(sell.x).toBeGreaterThan(pin.x);
    expect(row.funderIsKol).toBe(true);
    expect(row.funderLane).toBe(1);
  });

  it("draws the honest friend differently: the buy lands right of the pin", () => {
    const pin = L.pins.find((p) => p.seat === 3)!;
    const row = L.rows.find((r) => r.wallet === "f1")!;
    expect(row.marks.find((m) => m.kind === "buy")!.x).toBeGreaterThan(pin.x);
    expect(row.marks.some((m) => m.kind === "sell")).toBe(false);
  });

  it("puts the exchange-funded whale on the exchange lane, not a KOL's", () => {
    const row = L.rows.find((r) => r.wallet === "whale")!;
    expect(row.funderIsKol).toBe(false);
    expect(row.funderLane).toBe(seats.length);
  });

  it("buckets only the crowd (not seats, not funded wallets) into the wave", () => {
    expect(L.crowd.reduce((n, b) => n + b.buys, 0)).toBe(1);
    expect(L.crowd.reduce((n, b) => n + b.sells, 0)).toBe(1);
  });

  it("is tall enough for every wallet row and never has NaN coordinates", () => {
    expect(L.height).toBeGreaterThan(L.rows[L.rows.length - 1].y);
    for (const r of L.rows) expect(r.stringPath).not.toMatch(/NaN/);
    expect(L.pins.length).toBe(2);
  });

  it("gives the same board for the same feed", () => {
    expect(JSON.stringify(layoutBoard(v, 1000))).toBe(JSON.stringify(layoutBoard(v, 1000)));
  });

  it("clamps late events to the right edge instead of drawing off the board", () => {
    const late = { ...v, feed: [...v.feed, e(400, "Sell", "s1")] };
    const row = layoutBoard(late, 1000).rows.find((r) => r.wallet === "s1")!;
    expect(Math.max(...row.marks.map((m) => m.x))).toBeLessThanOrEqual(L.padL + L.innerW);
  });
});

describe("wallet facts and labels", () => {
  const v = scenario();
  it("reports plain timing relations", () => {
    const f = walletFacts(v, "s1")!;
    expect(f.funderLabel).toBe("@mira");
    expect(f.fundedAt).toBe(20_000);
    expect(f.trades.map((t) => [t.kind, t.at])).toEqual([["Buy", 34_000], ["Sell", 62_000]]);
    expect(f.calls.find((c) => c.seat === 1)!.at).toBe(54_000);
  });
  it("labels KOLs, the exchange and anonymous wallets", () => {
    expect(entryLabel(v, "miraw")).toEqual({ text: "@mira", kol: true });
    expect(entryLabel(v, "exchange")).toEqual({ text: "exchange", kol: false });
    expect(entryLabel(v, "a".repeat(64)).text).toBe("aaaaa…aaaa");
  });
});

describe("formatters", () => {
  it("converts yoctoNEAR without losing the whole part", () => {
    expect(near("1000000000000000000000000")).toBe(1);
    expect(near("2500000000000000000000000")).toBe(2.5);
    expect(near("")).toBe(0);
    expect(near("not a number")).toBe(0);
    expect(near("123456789012345678901234567890")).toBeGreaterThan(1e5);
  });
  it("formats amounts, clocks and offsets", () => {
    expect(fmtNear("100000000000000000000000")).toBe("0.100");
    expect(clock(65_000)).toBe("01:05");
    expect(clock(-5)).toBe("00:00");
    expect(offset(-18_000)).toBe("−18s");
    expect(offset(8_000)).toBe("+8s");
    expect(shortAcct("a".repeat(64))).toBe("aaaaa…aaaa");
    expect(shortAcct("kaito.testnet")).toBe("kaito.testnet");
  });
});

describe("compact layout for phones", () => {
  it("uses the room it has: a narrower label gutter and fewer ticks, never wider than the canvas", () => {
    const v = scenario();
    const wide = layoutBoard(v, 1000);
    const small = layoutBoard(v, 560, null, true);
    expect(small.padL).toBeLessThan(wide.padL);
    expect(small.ticks.length).toBeLessThan(wide.ticks.length);
    expect(small.xOf(OPEN + 130_000)).toBe(small.padL + small.innerW);
    expect(small.padL + small.innerW + small.padR).toBe(560);
    for (const r of small.rows) expect(r.fundedX).toBeLessThanOrEqual(560);
  });
});
