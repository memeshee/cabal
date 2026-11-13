"use client";
import { useEffect, useState } from "react";
import { fmtNear, near } from "@/lib/format";
import type { TableView } from "@/lib/types";

async function sha256Hex(s: string): Promise<string> {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(s));
  return Array.from(new Uint8Array(buf)).map((b) => b.toString(16).padStart(2, "0")).join("");
}

/** The proof moment. The browser recomputes the hash itself rather than trusting the house's `matches` flag. */
export function Reveal({ view, mineSeat, myAccount, onAgain, again }: { view: TableView; mineSeat: number | null; myAccount: string | null; onAgain: () => void; again: boolean }) {
  const r = view.reveal;
  const rv = view.round_view;
  const [local, setLocal] = useState<string | null>(null);
  useEffect(() => {
    if (r) sha256Hex(r.preimage).then(setLocal).catch(() => setLocal(null));
  }, [r]);

  if (view.phase === "voided") {
    return (
      <section className="reveal">
        <div className="l">
          <span className="stamp big slam">Voided</span>
          <p className="dim">The house did not reveal in time, so the contract voided this table. Its bond was paid to everyone who accused someone, split by stake.</p>
        </div>
        <div className="r">
          <div className="mono dim">This is the guarantee working: a house that stalls loses money, it doesn&apos;t get to pick a winner.</div>
          {rv && <a className="btn" href={`${view.explorer}/address/${view.contract}`} target="_blank" rel="noreferrer">contract on nearblocks ↗</a>}
        </div>
      </section>
    );
  }
  if (!r || !rv) return null;

  const caught = rv.accusers.some((a) => a.seat === r.cabal_seat);
  const youWon = mineSeat !== null && mineSeat === r.cabal_seat;
  const youLost = mineSeat !== null && mineSeat !== r.cabal_seat;
  const myStake = rv.accusers.find((a) => a.who === myAccount);
  const independent = local !== null && local === r.commitment;
  const color = view.seats[r.cabal_seat]?.color;

  return (
    <section className="reveal" aria-live="polite">
      <div className="l">
        <span className={`stamp big slam ${youWon ? "ok" : ""}`}>{youWon ? "Exposed" : youLost ? "Wrong" : caught ? "Exposed" : "Got away"}</span>
        <h2 style={{ color }}>{r.cabal_handle}</h2>
        <p className="dim" style={{ margin: 0 }}>
          {youWon && myStake && <>You called it. Your {fmtNear(myStake.stake, 2)} NEAR comes back with a share of the pot ({fmtNear(rv.payout, 3)} NEAR in all, including their burned bag).</>}
          {youLost && <>Not this time. {view.seats[mineSeat!].handle} was honest. Look at the wallet files: who paid for the fresh wallets that bought before a call?</>}
          {mineSeat === null && (caught ? <>Caught. Their {r.sides.length} side wallets were burned and the pot paid out.</> : <>Nobody accused them, so they keep the bag.</>)}
        </p>
        <button className="btn go big" onClick={onAgain} disabled={again}>{again ? "Dealing…" : "Deal another table"}</button>
      </div>
      <div className="r">
        <div className="mono" style={{ fontSize: 11, letterSpacing: "0.14em", textTransform: "uppercase" }} >
          <span className="dim">Proof: the house could not have chosen after the fact</span>
        </div>
        <div className="verify">
          <div className="line"><span className="ok">1</span><div>Before anyone traded, the contract recorded a commitment:<br /><code>{r.commitment}</code></div></div>
          <div className="line"><span className="ok">2</span><div>The house then revealed what was inside it:<br /><code>{r.preimage}</code></div></div>
          <div className="line">
            <span className={independent ? "ok" : local === null ? "dim" : "bad"}>{independent ? "✓" : local === null ? "…" : "✗"}</span>
            <div>Your browser hashed that text itself (SHA-256):<br /><code>{local ?? "computing…"}</code><br /><b className={independent ? "ok" : "bad"}>{independent ? "It matches the commitment." : local === null ? "" : "It does NOT match."}</b></div>
          </div>
          <div className="line">
            <span className="ok">✓</span>
            <div>The contract also checked, on-chain, that each of the {r.sides.length} side wallets was paid for by {r.cabal_handle}&apos;s wallet. A reveal that fails either check is rejected.</div>
          </div>
        </div>
        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
          {r.hash && <a className="btn sm" href={`${view.explorer}/txns/${r.hash}`} target="_blank" rel="noreferrer">reveal transaction ↗</a>}
          <a className="btn sm" href={`${view.explorer}/address/${view.contract}`} target="_blank" rel="noreferrer">contract ↗</a>
          {view.round && <a className="btn sm" href={`/t/r${view.round}`}>replay this table</a>}
        </div>
        <div className="faint mono" style={{ fontSize: 11 }}>{near(rv.reserve) > 0 ? `${fmtNear(rv.reserve, 3)} NEAR still in the curve.` : ""}</div>
      </div>
    </section>
  );
}
