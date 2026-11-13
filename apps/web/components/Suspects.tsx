"use client";
import { useState } from "react";
import { expose } from "@/lib/api";
import { fmtNear } from "@/lib/format";
import type { Me, TableView } from "@/lib/types";

const STAKES = [10, 25, 50, 100];

export function Suspects({ view, token, me, live, onToast }: { view: TableView; token: string | null; me: Me | null; live: boolean; onToast: (m: string, kind?: "good" | "bad", link?: string | null) => void }) {
  const [confirm, setConfirm] = useState<number | null>(null);
  const [stake, setStake] = useState(50);
  const [busy, setBusy] = useState(false);
  const mine = me?.accusation ?? view.round_view?.accusers.find((a) => token && a.who === me?.account) ?? null;
  const cabal = view.reveal?.cabal_seat ?? null;
  const counts = view.seats.map((_, i) => view.round_view?.accusers.filter((a) => a.seat === i).length ?? 0);

  async function accuse(seat: number) {
    if (!token || busy) return;
    setBusy(true);
    const r = await expose(view.key, token, seat, stake);
    setBusy(false);
    setConfirm(null);
    if (r.ok) onToast(`You accused ${view.seats[seat].handle}. Now we wait for the reveal.`, "good", r.explorer_url);
    else onToast(r.error ?? "That didn't go through.", "bad");
  }

  return (
    <div className="panel">
      <header>
        <b>Who&apos;s the cabal?</b>
        <span className="hint">one accusation per table</span>
      </header>
      {view.seats.map((s, i) => {
        const picked = mine?.seat === i;
        return (
          <div key={s.handle} className={`suspect ${picked ? "picked" : ""} ${cabal === i ? "is-cabal" : ""}`}>
            <div className="avatar" style={{ background: s.color }}>{s.handle.slice(1, 2).toUpperCase()}</div>
            <div>
              <div className="nm" style={{ color: s.color }}>{s.handle}{cabal === i && <span className="stamp" style={{ fontSize: 11, marginLeft: 8, borderWidth: 2, padding: "0 6px" }}>cabal</span>}</div>
              <div className="tl">{s.tagline}{counts[i] > 0 && <> · <span className="mono">{counts[i]} accusing</span></>}</div>
            </div>
            <div>
              {live && !mine && confirm !== i && (
                <button className="btn sm exp" disabled={!token} onClick={() => setConfirm(i)}>Expose</button>
              )}
              {picked && <span className="mono" style={{ fontSize: 11 }}>YOUR PICK<br /><span className="dim">{fmtNear(mine?.stake, 2)} NEAR</span></span>}
            </div>
            {confirm === i && (
              <div style={{ gridColumn: "1 / -1", display: "grid", gap: 10 }}>
                <div className="mono" style={{ fontSize: 12 }}>
                  Stake on <b style={{ color: s.color }}>{s.handle}</b>. Right: you get it back plus a share of the pot. Wrong: it goes to the people who were right.
                </div>
                <div className="amounts" role="group" aria-label="Stake">
                  {STAKES.map((a) => (
                    <button key={a} className={stake === a ? "on" : ""} onClick={() => setStake(a)}>{(a / 1000).toFixed(3)}</button>
                  ))}
                </div>
                <div className="two" style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
                  <button className="btn sm" onClick={() => setConfirm(null)}>Not yet</button>
                  <button className="btn sm exp" disabled={busy} onClick={() => accuse(i)}>{busy ? "Filing…" : "Yes, expose"}</button>
                </div>
              </div>
            )}
          </div>
        );
      })}
      {live && !mine && (
        <div className="stake-note">Accusations lock when the market closes. Wait for more evidence, or don&apos;t.</div>
      )}
    </div>
  );
}
