import type { Tier } from "./types";

const KEY = "cabal.progress.v1";
export const TIERS: Tier[] = ["rookie", "hop", "ghost"];

export interface Progress {
  /** Tiers the player has played to a verdict, in order. */
  done: Tier[];
  /** Tiers they exposed correctly. */
  won: Tier[];
}

export function loadProgress(): Progress {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) {
      const p = JSON.parse(raw) as Progress;
      return { done: p.done ?? [], won: p.won ?? [] };
    }
  } catch {
    /* private mode */
  }
  return { done: [], won: [] };
}

export function saveResult(tier: Tier, won: boolean): Progress {
  const p = loadProgress();
  if (!p.done.includes(tier)) p.done.push(tier);
  if (won && !p.won.includes(tier)) p.won.push(tier);
  try {
    localStorage.setItem(KEY, JSON.stringify(p));
  } catch {
    /* private mode */
  }
  return p;
}

/** The tier to suggest: the first one not yet won, so a player who solved Rookie is offered Hop. */
export function suggestedTier(p: Progress): Tier {
  return TIERS.find((t) => !p.won.includes(t)) ?? "ghost";
}

export const TIER_INFO: Record<Tier, { name: string; blurb: string }> = {
  rookie: { name: "Rookie", blurb: "The cabal pays for its wallets directly. Follow the strings." },
  hop: { name: "Hop", blurb: "The money goes through a relay first, and an honest caller buys early out of conviction." },
  ghost: { name: "Ghost", blurb: "Everyone tops up the exchange. The money trail is noise; only timing gives the cabal away." },
};
