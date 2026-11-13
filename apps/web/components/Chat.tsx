"use client";
import { useEffect, useRef, useState } from "react";
import { ask } from "@/lib/api";
import type { ChatLine, TableView } from "@/lib/types";

export function Chat({ view, token, live, onAnswer, onToast }: { view: TableView; token: string | null; live: boolean; onAnswer: (l: ChatLine[]) => void; onToast: (m: string, kind?: "good" | "bad") => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [seat, setSeat] = useState(0);
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const n = view.chat.length;
  useEffect(() => {
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [n]);

  const color = (who: string) => view.seats.find((s) => s.handle === who)?.color;
  async function send(e: React.FormEvent) {
    e.preventDefault();
    if (!token || !text.trim() || busy) return;
    setBusy(true);
    const r = await ask(view.key, token, seat, text.trim());
    setBusy(false);
    if (r.ok && r.lines) {
      onAnswer(r.lines);
      setText("");
    } else onToast(r.error ?? "That didn't go through.", "bad");
  }

  return (
    <div className="panel">
      <header>
        <b>The room</b>
        <span className="hint">they say what they want you to hear</span>
      </header>
      <div className="chat" ref={ref}>
        {view.chat.length === 0 && <div className="empty">Quiet so far. Callers speak when they post.</div>}
        {view.chat.map((c) => (
          <div className={`msg ${c.kind}`} key={c.id}>
            <div className="h" style={{ color: c.kind === "kol" ? color(c.who) : undefined }}>
              {c.kind === "house" ? "THE HOUSE" : c.who}
            </div>
            <p>{c.text}</p>
          </div>
        ))}
      </div>
      {live && (
        <form className="askrow" onSubmit={send}>
          <select value={seat} onChange={(e) => setSeat(Number(e.target.value))} aria-label="Who to ask">
            {view.seats.map((s, i) => (
              <option key={s.handle} value={i}>{s.handle}</option>
            ))}
          </select>
          <input value={text} onChange={(e) => setText(e.target.value)} maxLength={140} placeholder="Ask them straight…" aria-label="Your question" />
          <button className="btn sm go" disabled={busy || !text.trim() || !token}>{busy ? "…" : "Ask"}</button>
        </form>
      )}
    </div>
  );
}
