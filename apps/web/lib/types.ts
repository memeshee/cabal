// Mirrors the Rust house's JSON. Amounts that can exceed 2^53 (yoctoNEAR) stay strings.

export type Tier = "rookie" | "hop" | "ghost";

export type Phase = "dealing" | "playing" | "revealing" | "revealed" | "voided" | "failed";

export interface Entry {
  idx: number;
  ts_ms: number;
  block: number;
  kind: "Open" | "Buy" | "Sell" | "Call" | "Fund" | "Expose" | "Reveal" | "Confiscate" | "Void";
  actor: string;
  other: string | null;
  chips: number;
  near: string;
  text: string | null;
}

export interface Seat {
  handle: string;
  wallet: string;
  tagline: string;
  color: string;
}

export interface Accuser { who: string; seat: number; stake: string; at_ms: number }

export interface RoundView {
  id: number;
  ticker: string;
  commitment: string;
  seats: { handle: string; wallet: string }[];
  funders: string[];
  opened_ms: number;
  closes_ms: number;
  reveal_by_ms: number;
  supply: number;
  reserve: string;
  bond: string;
  price: string;
  status: "Open" | "Revealed" | "Voided";
  cabal_seat: number | null;
  sides: string[];
  salt: string | null;
  accusers: Accuser[];
  feed_len: number;
  pruned: number;
  payout: string;
}

export interface ChatLine {
  id: number;
  ts_ms: number;
  who: string;
  text: string;
  kind: "kol" | "you" | "house";
  via: "NearAi" | "Llm" | "Scripted" | null;
  /** Set when this is a caller's answer to one of the standard questions. */
  claim_q: number | null;
  claim_seat: number | null;
  claim_tag: string | null;
  /** Whether the claim was true. Null until the table is settled. */
  claim_true: boolean | null;
}

export interface Dossier {
  wallet: string;
  funder_seat: number | null;
  /** The caller at the end of the funding trail. Null while the table is live, and for laundered trails. */
  root_seat: number | null;
  funder: string;
  funded_ms: number;
  funded_idx: number;
  first_buy_ms: number | null;
  first_buy_idx: number | null;
  chips_bought: number;
  first_sell_ms: number | null;
  chips_sold: number;
  funder_call_ms: number | null;
  lead_ms: number | null;
  dumped_after_ms: number | null;
}

export interface RevealInfo {
  cabal_seat: number;
  cabal_handle: string;
  sides: string[];
  salt: string;
  preimage: string;
  commitment: string;
  recomputed: string;
  matches: boolean;
  hash: string | null;
}

export interface TableView {
  key: string;
  phase: Phase;
  round: number | null;
  ticker: string;
  tier: Tier;
  players: number;
  demo: boolean;
  note: string;
  error: string | null;
  seats: Seat[];
  labels: Record<string, string>;
  round_view: RoundView | null;
  feed: Entry[];
  feed_total: number;
  chat: ChatLine[];
  chat_total: number;
  dossiers: Dossier[];
  verdict: { seat: number; score: number; runner_up: number } | null;
  reveal: RevealInfo | null;
  elapsed_ms: number | null;
  play_ms: number;
  reveal_ms: number;
  contract: string;
  explorer: string;
}

export interface ActionResult {
  ok: boolean;
  hash: string | null;
  explorer_url: string | null;
  error: string | null;
  detail: unknown;
}

export interface DetectiveRecord { played: number; caught: number; best_ms: number }

export interface LobbyRow { key: string; ticker: string; tier: Tier; elapsed_ms: number; players: number; demo: boolean }

export interface Me {
  ok: boolean;
  record: DetectiveRecord;
  account: string;
  balance_near: number;
  chips: number;
  chips_value: string;
  accusation: Accuser | null;
}
