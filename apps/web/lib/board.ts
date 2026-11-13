// Board geometry: turns the on-chain feed into lanes, pins and strings. Pure so it can be tested.
// Time runs left to right. Each KOL has a lane with a pin where they posted their call. Every wallet that someone
// funded gets its own row below, with a red string from the funder's lane at the moment of funding.

import type { Dossier, Entry, TableView } from "./types";

export interface Pin { seat: number; x: number; ts: number; text: string }
export interface Mark { kind: "buy" | "sell"; x: number; ts: number; chips: number }
export interface WalletRow {
  wallet: string;
  y: number;
  funderLane: number;           // lane index of the funder (seats 0..n-1, n = exchange); the relay row's lane when paid by a wallet
  funderIsKol: boolean;
  /** Set when another wallet row paid for this one (the relay in the Hop tier): the string starts on that row. */
  funderRow: number | null;
  fundedX: number;
  stringPath: string;
  marks: Mark[];
  spanX: [number, number] | null;
  dossier: Dossier;
}
export interface LaneLink { x: number; fromY: number; toY: number; seat: number; ts: number }
export interface CrowdBar { x: number; w: number; buys: number; sells: number }
export interface Layout {
  width: number; height: number; padL: number; padR: number; innerW: number;
  laneY: number[];              // y of each lane centre (kols then exchange)
  laneH: number;
  pins: Pin[];
  rows: WalletRow[];
  /** A caller paying the exchange lane directly (every caller does, in the Ghost tier). */
  laneLinks: LaneLink[];
  crowd: CrowdBar[];
  crowdBase: number;
  crowdH: number;
  ticks: { x: number; label: string }[];
  nowX: number | null;
  xOf: (tsMs: number) => number;
}

export const LANE_H = 54;
export const ROW_H = 30;

export function layoutBoard(v: TableView, width = 1000, nowMs: number | null = null, compact = false): Layout {
  const padL = compact ? 96 : 150, padR = compact ? 14 : 24;
  const innerW = width - padL - padR;
  const open = v.round_view?.opened_ms ?? v.feed[0]?.ts_ms ?? 0;
  const span = v.play_ms || 130_000;
  const xOf = (ts: number) => padL + Math.min(1, Math.max(0, (ts - open) / span)) * innerW;

  const nSeats = v.seats.length;
  const laneY = Array.from({ length: nSeats + 1 }, (_, i) => 20 + i * LANE_H + LANE_H / 2);
  const walletTop = 20 + (nSeats + 1) * LANE_H + 26;

  const pins: Pin[] = v.feed
    .filter((e) => e.kind === "Call")
    .flatMap((e) => {
      const seat = v.seats.findIndex((s) => s.wallet === e.actor);
      return seat < 0 ? [] : [{ seat, x: xOf(e.ts_ms), ts: e.ts_ms, text: e.text ?? "" }];
    });

  // Rows first, so a wallet paid by another wallet (the relay) can draw its string from that row.
  const rowY = new Map<string, number>(v.dossiers.map((d, i) => [d.wallet, walletTop + i * ROW_H + ROW_H / 2]));
  const rowIdx = new Map<string, number>(v.dossiers.map((d, i) => [d.wallet, i]));
  const rows: WalletRow[] = v.dossiers.map((d, i) => {
    const y = walletTop + i * ROW_H + ROW_H / 2;
    const funderIsKol = d.funder_seat !== null;
    const funderRowIdx = rowIdx.get(d.funder) ?? null;
    const funderLane = funderIsKol ? (d.funder_seat as number) : nSeats;
    const fundedX = xOf(d.funded_ms);
    const fy = funderRowIdx !== null ? (rowY.get(d.funder) as number) : laneY[funderLane];
    const bend = Math.min(60, Math.abs(y - fy) * 0.35);
    const stringPath = `M ${fundedX} ${fy} C ${fundedX + bend} ${fy + (y - fy) * 0.3}, ${fundedX - bend} ${fy + (y - fy) * 0.7}, ${fundedX} ${y}`;
    const marks: Mark[] = v.feed
      .filter((e) => e.actor === d.wallet && (e.kind === "Buy" || e.kind === "Sell"))
      .map((e) => ({ kind: e.kind === "Buy" ? "buy" : "sell", x: xOf(e.ts_ms), ts: e.ts_ms, chips: e.chips }));
    const xs = [fundedX, ...marks.map((m) => m.x)];
    return { wallet: d.wallet, y, funderLane, funderIsKol, funderRow: funderRowIdx, fundedX, stringPath, marks, spanX: [Math.min(...xs), Math.max(...xs)], dossier: d };
  });

  // Callers paying the exchange lane directly: drawn as a link between the two lanes.
  const exchange = Object.keys(v.labels).find((a) => v.labels[a] === "exchange");
  const laneLinks: LaneLink[] = exchange
    ? v.feed
        .filter((e) => e.kind === "Fund" && e.other === exchange)
        .flatMap((e) => {
          const seat = v.seats.findIndex((s) => s.wallet === e.actor);
          return seat < 0 ? [] : [{ x: xOf(e.ts_ms), fromY: laneY[seat], toY: laneY[nSeats], seat, ts: e.ts_ms }];
        })
    : [];

  const height = walletTop + Math.max(1, rows.length) * ROW_H + 96 + 40;

  // Everyone who is neither a seat nor a funded wallet: the crowd. Bucketed buys/sells show the wave after a call.
  const known = new Set<string>([...v.seats.map((s) => s.wallet), ...v.dossiers.map((d) => d.wallet)]);
  const BUCKETS = 36;
  const buckets = Array.from({ length: BUCKETS }, () => ({ buys: 0, sells: 0 }));
  for (const e of v.feed) {
    if (known.has(e.actor) || (e.kind !== "Buy" && e.kind !== "Sell")) continue;
    const b = Math.min(BUCKETS - 1, Math.max(0, Math.floor(((e.ts_ms - open) / span) * BUCKETS)));
    if (e.kind === "Buy") buckets[b].buys++;
    else buckets[b].sells++;
  }
  const bw = innerW / BUCKETS;
  const crowd: CrowdBar[] = buckets.map((b, i) => ({ x: padL + i * bw, w: bw - 2, buys: b.buys, sells: b.sells }));

  const ticks = (compact ? [0, 40, 80, 120] : [0, 20, 40, 60, 80, 100, 120, 140, 160, 180])
    .filter((s) => s * 1000 <= span)
    .map((s) => ({ x: xOf(open + s * 1000), label: `${s}s` }));

  return {
    width, height, padL, padR, innerW, laneY, laneH: LANE_H, pins, rows, laneLinks, crowd,
    crowdBase: height - 40, crowdH: 48, ticks,
    nowX: nowMs === null ? null : xOf(open + nowMs),
    xOf,
  };
}

/** Facts for the side panel. Plain relations between timestamps; no conclusions drawn. */
export function walletFacts(v: TableView, wallet: string) {
  const open = v.round_view?.opened_ms ?? 0;
  const d = v.dossiers.find((x) => x.wallet === wallet);
  if (!d) return null;
  const calls = v.feed
    .filter((e) => e.kind === "Call")
    .map((e) => ({ seat: v.seats.findIndex((s) => s.wallet === e.actor), at: e.ts_ms - open }));
  const trades = v.feed.filter((e) => e.actor === wallet && (e.kind === "Buy" || e.kind === "Sell"));
  return {
    d,
    funderLabel: d.funder_seat !== null ? v.seats[d.funder_seat].handle : v.labels[d.funder] ?? "exchange",
    fundedAt: d.funded_ms - open,
    trades: trades.map((t) => ({ kind: t.kind, chips: t.chips, at: t.ts_ms - open, idx: t.idx })),
    calls,
  };
}

export function isKolWallet(v: TableView, a: string): number {
  return v.seats.findIndex((s) => s.wallet === a);
}

export function entryLabel(v: TableView, a: string): { text: string; kol: boolean } {
  const s = isKolWallet(v, a);
  if (s >= 0) return { text: v.seats[s].handle, kol: true };
  const l = v.labels[a];
  if (l) return { text: l, kol: false };
  return { text: a.length > 16 ? `${a.slice(0, 5)}…${a.slice(-4)}` : a, kol: false };
}

export type { Entry };


/** Wallets a caller paid for, directly or through other wallets (a relay). A trail through the exchange does not count. */
export function descendants(v: TableView, seat: number): Set<string> {
  const out = new Set<string>();
  const me = v.seats[seat]?.wallet;
  if (!me) return out;
  let changed = true;
  const owned = new Set<string>([me]);
  while (changed) {
    changed = false;
    for (const d of v.dossiers) {
      if (!out.has(d.wallet) && owned.has(d.funder)) {
        out.add(d.wallet);
        owned.add(d.wallet);
        changed = true;
      }
    }
  }
  return out;
}

/** Every non-caller wallet that bought in the half minute before a caller's call, ending six seconds before it. */
export function earlyBuyers(v: TableView, seat: number): Set<string> {
  const out = new Set<string>();
  const call = v.feed.find((e) => e.kind === "Call" && e.actor === v.seats[seat]?.wallet);
  if (!call) return out;
  const seatWallets = new Set(v.seats.map((s) => s.wallet));
  for (const e of v.feed) {
    if (e.kind !== "Buy" || seatWallets.has(e.actor)) continue;
    const lead = call.ts_ms - e.ts_ms;
    if (lead >= 6_000 && lead <= 32_000) out.add(e.actor);
  }
  return out;
}

/** Which wallets a claim chip should light up on the board. Neutral: it shows the relation, not the verdict. */
export function claimHighlight(v: TableView, seat: number, q: number): Set<string> {
  if (q === 0) return descendants(v, seat);
  if (q === 1) return earlyBuyers(v, seat);
  return new Set();
}
