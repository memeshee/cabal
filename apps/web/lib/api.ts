import type { ActionResult, ChatLine, Me, TableView } from "./types";

const TOKEN_KEY = "cabal.guest.v1";

export function getToken(): string | null {
  try {
    return localStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}
function setToken(t: string) {
  try {
    localStorage.setItem(TOKEN_KEY, t);
  } catch {
    /* private mode: the token lives for this page only */
  }
}

async function json<T>(r: Response): Promise<T> {
  const text = await r.text();
  try {
    return JSON.parse(text) as T;
  } catch {
    throw new Error(r.ok ? "The house sent something unreadable." : `The house is unreachable (${r.status}).`);
  }
}

export interface Guest { token: string; account: string; balance_near: number }

/** Get (or restore) this visitor's throwaway testnet account. */
export async function ensureGuest(): Promise<Guest> {
  const r = await fetch("/api/guest", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ token: getToken() }),
  });
  const j = await json<{ ok: boolean; token?: string; account?: string; balance_near?: number; error?: string }>(r);
  if (!j.ok || !j.token) throw new Error(j.error ?? "Could not set you up with play money.");
  setToken(j.token);
  return { token: j.token, account: j.account!, balance_near: j.balance_near ?? 0 };
}

export async function newTable(demo: boolean): Promise<string> {
  const r = await fetch("/api/table", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ demo }) });
  const j = await json<{ ok: boolean; key?: string; error?: string }>(r);
  if (!j.ok || !j.key) throw new Error(j.error ?? "Could not open a table.");
  return j.key;
}

export async function getTable(key: string, feed: number, chat: number): Promise<TableView> {
  const r = await fetch(`/api/table/${key}?feed=${feed}&chat=${chat}`, { cache: "no-store" });
  if (r.status === 404) throw new Gone();
  return json<TableView>(r);
}

export class Gone extends Error {
  constructor() {
    super("That table is gone.");
  }
}

export async function getRound(id: number): Promise<TableView> {
  const r = await fetch(`/api/round/${id}`, { cache: "no-store" });
  if (!r.ok) throw new Error("No such table on the contract.");
  return json<TableView>(r);
}

export async function getMe(key: string, token: string): Promise<Me> {
  const r = await fetch(`/api/table/${key}/me?token=${encodeURIComponent(token)}`, { cache: "no-store" });
  return json<Me>(r);
}

async function act(key: string, what: string, body: Record<string, unknown>): Promise<ActionResult> {
  try {
    const r = await fetch(`/api/table/${key}/${what}`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
    return await json<ActionResult>(r);
  } catch (e) {
    return { ok: false, hash: null, explorer_url: null, error: (e as Error).message, detail: null };
  }
}

export const buy = (key: string, token: string, mnear: number) => act(key, "buy", { token, mnear });
export const sell = (key: string, token: string, pct: number) => act(key, "sell", { token, pct });
export const expose = (key: string, token: string, seat: number, mnear: number) => act(key, "expose", { token, seat, mnear });

export async function ask(key: string, token: string, seat: number, text: string): Promise<{ ok: boolean; lines?: ChatLine[]; error?: string }> {
  try {
    const r = await fetch(`/api/table/${key}/ask`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ token, seat, text }) });
    return await json(r);
  } catch (e) {
    return { ok: false, error: (e as Error).message };
  }
}

export async function listRounds(): Promise<{ id: number; ticker: string; status: string; accusers: number; cabal: string | null }[]> {
  const r = await fetch("/api/rounds", { cache: "no-store" });
  const j = await json<{ rounds?: { id: number; ticker: string; status: string; accusers: number; cabal: string | null }[] }>(r);
  return j.rounds ?? [];
}
