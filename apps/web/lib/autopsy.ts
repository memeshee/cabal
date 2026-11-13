// The autopsy: after the reveal, explain exactly what the cabal did, in the same terms the board uses.
// Everything is derived from the public feed plus the revealed list of side wallets.

import type { TableView } from "./types";

export interface TellRow {
  wallet: string;
  /** Who paid for it, as the funding trail reads from the wallet back to the cabal, e.g. ["relay", "@mira"]. */
  trail: string[];
  boughtBeforeCallMs: number | null;
  soldAfterCallMs: number | null;
  chips: number;
}

export interface Autopsy {
  cabal: string;
  callAtMs: number | null;
  rows: TellRow[];
  headline: string;
  /** Plain-language summary of the tactic for the tier played. */
  tactic: string;
  /** What the player could have used, in the order a careful reader would. */
  clues: string[];
}

function label(v: TableView, a: string): string {
  const s = v.seats.findIndex((x) => x.wallet === a);
  if (s >= 0) return v.seats[s].handle;
  return v.labels[a] ?? `${a.slice(0, 5)}…`;
}

/** Walk first-funders from a wallet back to a caller (or the exchange), naming each hop. */
export function trailOf(v: TableView, wallet: string): string[] {
  const out: string[] = [];
  let cur = wallet;
  const funderOf = new Map<string, string>();
  for (const e of v.feed) if (e.kind === "Fund" && e.other && !funderOf.has(e.other)) funderOf.set(e.other, e.actor);
  for (let i = 0; i < 4; i++) {
    const f = funderOf.get(cur);
    if (!f) break;
    out.push(label(v, f));
    if (v.seats.some((s) => s.wallet === f)) break;
    cur = f;
  }
  return out;
}

export function autopsy(v: TableView): Autopsy | null {
  const r = v.reveal;
  if (!r) return null;
  const cabalWallet = v.seats[r.cabal_seat]?.wallet;
  const call = v.feed.find((e) => e.kind === "Call" && e.actor === cabalWallet);
  const callAt = call?.ts_ms ?? null;
  const rows: TellRow[] = r.sides.map((w) => {
    const buy = v.feed.find((e) => e.kind === "Buy" && e.actor === w);
    const sell = v.feed.find((e) => e.kind === "Sell" && e.actor === w);
    const bought = v.feed.filter((e) => e.kind === "Buy" && e.actor === w).reduce((n, e) => n + e.chips, 0);
    return {
      wallet: w,
      trail: trailOf(v, w),
      boughtBeforeCallMs: callAt !== null && buy ? callAt - buy.ts_ms : null,
      soldAfterCallMs: callAt !== null && sell ? sell.ts_ms - callAt : null,
      chips: bought,
    };
  });
  const leads = rows.map((x) => x.boughtBeforeCallMs).filter((x): x is number => x !== null);
  const dumps = rows.map((x) => x.soldAfterCallMs).filter((x): x is number => x !== null);
  const avg = (a: number[]) => (a.length ? a.reduce((n, x) => n + x, 0) / a.length : 0);
  const secs = (ms: number) => Math.round(ms / 1000);
  const headline = leads.length
    ? `${rows.length} fresh wallets bought about ${secs(avg(leads))}s before ${r.cabal_handle}'s call${dumps.length ? ` and sold about ${secs(avg(dumps))}s after it` : ""}.`
    : `${rows.length} fresh wallets belonged to ${r.cabal_handle}.`;

  const tier = v.tier;
  const tactic =
    tier === "rookie"
      ? `${r.cabal_handle} paid for the wallets directly. The string runs straight from their lane.`
      : tier === "hop"
        ? `${r.cabal_handle} paid a relay wallet first, and the relay paid the three. The trail has two legs, so reading only "who paid for this wallet" shows an anonymous relay.`
        : `${r.cabal_handle} was the first of four callers to top up the exchange, which then paid the three wallets. Every caller left money at the exchange, so the funding trail proves nothing without the order. The timing is the tell.`;

  const clues: string[] = [];
  if (tier !== "ghost") clues.push(`Follow the strings back: the trail ends at ${r.cabal_handle}.`);
  else clues.push("Funding was noise. Look for fresh wallets that bought just before one call and dumped right after it.");
  clues.push("A wallet that bought before a call and sold into the crowd after it is front-running that call.");
  if (v.chat.some((c) => c.claim_true === false)) {
    clues.push(
      tier === "ghost"
        ? `${r.cabal_handle}'s "no other wallets" was false, but on Ghost the funding cannot show it. Only the timing could.`
        : "A claim that the chain contradicted. Check what they said against who they paid.",
    );
  }
  return { cabal: r.cabal_handle, callAtMs: callAt, rows, headline, tactic, clues };
}

/** Text for the share card and the post. */
export function shareText(v: TableView, won: boolean | null, seconds: number | null): string {
  const r = v.reveal;
  if (!r) return "";
  const tier = v.tier[0].toUpperCase() + v.tier.slice(1);
  if (won === true) return `I exposed ${r.cabal_handle} as the cabal${seconds ? ` in ${seconds}s` : ""} on CABAL (${tier}). People lie. The chain doesn't.`;
  if (won === false) return `${r.cabal_handle} fooled me on CABAL (${tier}). The chain had the whole story. Can you read it?`;
  return `${r.cabal_handle} was the cabal on CABAL (${tier}).`;
}


export interface Standing { who: string; seat: number; handle: string; seconds: number; correct: boolean; stake: string; isYou: boolean }

/** Everyone who accused someone at this table, fastest correct answer first. */
export function standings(v: TableView, me: string | null): Standing[] {
  const rv = v.round_view;
  const cabal = v.reveal?.cabal_seat;
  if (!rv || cabal === undefined) return [];
  return rv.accusers
    .map((a) => ({
      who: a.who,
      seat: a.seat,
      handle: v.seats[a.seat]?.handle ?? "?",
      seconds: Math.round((a.at_ms ?? 0) / 1000),
      correct: a.seat === cabal,
      stake: a.stake,
      isYou: a.who === me,
    }))
    .sort((x, y) => Number(y.correct) - Number(x.correct) || x.seconds - y.seconds);
}
