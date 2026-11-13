"use client";
import { walletFacts } from "@/lib/board";
import { offset } from "@/lib/format";
import type { TableView } from "@/lib/types";

/** What anyone can read about one wallet. Facts only: no verdicts. */
export function WalletFile({ view, wallet, onClose }: { view: TableView; wallet: string; onClose: () => void }) {
  const f = walletFacts(view, wallet);
  if (!f) return null;
  const sealed = view.reveal?.sides.includes(wallet);
  return (
    <div className="panel">
      <header>
        <b>Wallet file · {wallet.slice(0, 6)}…{wallet.slice(-4)}</b>
        <button className="btn sm" onClick={onClose} aria-label="Close wallet file">Close</button>
      </header>
      <div className="dossier">
        <div className="kv"><span>paid for by</span><b>{f.funderLabel}</b></div>
        <div className="kv"><span>funded at</span><span>{offset(f.fundedAt)}</span></div>
        {f.trades.length === 0 && <div className="kv"><span>trades</span><span className="dim">none yet</span></div>}
        {f.trades.map((t) => (
          <div className="kv" key={t.idx}>
            <span>{t.kind === "Buy" ? "bought" : "sold"}</span>
            <span className={t.kind === "Buy" ? "up" : "down"}>{t.chips} chips at {offset(t.at)}</span>
          </div>
        ))}
        {f.calls.map((c, i) => (
          <div className="kv" key={i}>
            <span>{view.seats[c.seat]?.handle} called</span>
            <span className="dim">{offset(c.at)}</span>
          </div>
        ))}
        {sealed && <div className="kv"><span>verdict</span><b style={{ color: "var(--red)" }}>cabal side wallet</b></div>}
        <a href={`${view.explorer}/address/${wallet}`} target="_blank" rel="noreferrer">open on nearblocks ↗</a>
      </div>
    </div>
  );
}
