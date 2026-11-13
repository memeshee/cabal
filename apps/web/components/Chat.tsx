"use client";
import { useEffect, useRef, useState } from "react";
import { ask } from "@/lib/api";
import type { ChatLine, TableView } from "@/lib/types";

export const QUESTION_LABELS = ["Other wallets?", "Bought early?", "Holding?"];

/** The room. Callers answer three standard questions with claims you can check against the board. */
export function Chat({
  view, token, live, onAnswer, onToast, focus, onFocus,
}: {
  view: TableView; token: string | null; live: boolean;
  onAnswer: (l: ChatLine[]) => void; onToast: (m: string, kind?: "good" | "bad") => void;
  /** The claim currently lit up on the board, as `${seat}:${q}`. */
  focus: string | null; onFocus: (f: string | null) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [seat, setSeat] = useState(0);
  const [text, setText] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const n = view.chat.length;
  useEffect(() => {
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [n]);

  const color = (who: string) => view.seats.find((s) => s.handle === who)?.color;
  const asked = new Set(view.chat.filter((c) => c.claim_q !== null && c.claim_seat !== null).map((c) => `${c.claim_seat}:${c.claim_q}`));

  async function askQ(q: number) {
    if (!token || busy) return;
    setBusy(`${seat}:${q}`);
    const r = await ask(view.key, token, seat, "", q);
    setBusy(null);
    if (r.ok && r.lines) {
      onAnswer(r.lines);
      onFocus(`${seat}:${q}`);
    } else onToast(r.error ?? "That didn't go through.", "bad");
  }
  async function chat(e: React.FormEvent) {
    e.preventDefault();
    if (!token || !text.trim() || busy) return;
    setBusy("chat");
    const r = await ask(view.key, token, seat, text.trim());
    setBusy(null);
    if (r.ok && r.lines) {
      onAnswer(r.lines);
      setText("");
    } else onToast(r.error ?? "That didn't go through.", "bad");
  }

  return (
    <div className="panel">
      <header>
        <b>The room</b>
        <span className="hint">their answers can be checked on the board</span>
      </header>
      <div className="chat" ref={ref}>
        {view.chat.length === 0 && <div className="empty">Quiet so far. Callers speak when they post.</div>}
        {view.chat.map((c) => {
          const key = c.claim_seat !== null && c.claim_q !== null ? `${c.claim_seat}:${c.claim_q}` : null;
          const on = key !== null && focus === key;
          return (
            <div className={`msg ${c.kind}`} key={c.id}>
              <div className="h" style={{ color: c.kind === "kol" ? color(c.who) : undefined }}>
                {c.kind === "house" ? "THE HOUSE" : c.who}
              </div>
              <p>{c.text}</p>
              {key && c.claim_tag && on && (
                <button className="claim-clear" onClick={() => onFocus(null)} aria-label="Stop highlighting this claim on the board">clear</button>
              )}
              {key && c.claim_tag && (
                <button className={`claim ${on ? "on" : ""} ${c.claim_true === true ? "true" : c.claim_true === false ? "false" : ""}`} onClick={() => onFocus(key)} aria-pressed={on} title={on ? "Showing on the board" : "Show what this is about on the board"}>
                  <span className="mark">{c.claim_true === true ? "✓ TRUE" : c.claim_true === false ? "✗ FALSE" : on ? "CHECKING" : "CHECK"}</span>
                  <span>claims: {c.claim_tag}</span>
                </button>
              )}
            </div>
          );
        })}
      </div>
      {live && (
        <div className="asks">
          <div className="who" role="tablist" aria-label="Who to question">
            {view.seats.map((s, i) => (
              <button key={s.handle} role="tab" aria-selected={seat === i} className={seat === i ? "on" : ""} style={seat === i ? { borderColor: s.color, color: s.color } : undefined} onClick={() => setSeat(i)}>
                {s.handle}
              </button>
            ))}
          </div>
          <div className="qs">
            {QUESTION_LABELS.map((label, q) => {
              const done = asked.has(`${seat}:${q}`);
              return (
                <button key={q} className="btn sm" disabled={!token || !!busy || done} onClick={() => askQ(q)}>
                  {busy === `${seat}:${q}` ? "…" : done ? "✓ " + label : label}
                </button>
              );
            })}
          </div>
          <form className="askrow" onSubmit={chat}>
            <input value={text} onChange={(e) => setText(e.target.value)} maxLength={140} placeholder={`Or say something to ${view.seats[seat]?.handle ?? "them"}…`} aria-label="Free chat" />
            <button className="btn sm" disabled={!!busy || !text.trim() || !token}>{busy === "chat" ? "…" : "Send"}</button>
          </form>
        </div>
      )}
    </div>
  );
}
