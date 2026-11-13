"use client";
import { useMemo } from "react";
import { autopsy } from "@/lib/autopsy";
import type { TableView } from "@/lib/types";

/** After the reveal: exactly what the cabal did, in the board's own terms. Turns a lost round into a lesson. */
export function Autopsy({ view }: { view: TableView }) {
  const a = useMemo(() => autopsy(view), [view]);
  if (!a) return null;
  const s = (ms: number | null) => (ms === null ? "—" : `${Math.round(ms / 1000)}s`);
  return (
    <div className="panel autopsy">
      <header>
        <b>The tell</b>
        <span className="hint">what {a.cabal} actually did</span>
      </header>
      <div className="body">
        <p className="lead">{a.headline}</p>
        <p className="dim" style={{ margin: "0 0 12px" }}>{a.tactic}</p>
        <table className="tell">
          <thead>
            <tr><th>wallet</th><th>paid for by</th><th>bought</th><th>sold</th></tr>
          </thead>
          <tbody>
            {a.rows.map((r) => (
              <tr key={r.wallet}>
                <td className="mono">{r.wallet.slice(0, 5)}…{r.wallet.slice(-3)}</td>
                <td className="mono">{r.trail.join(" ← ")}</td>
                <td className="up mono">{r.boughtBeforeCallMs === null ? "—" : `${s(r.boughtBeforeCallMs)} before call`}</td>
                <td className="down mono">{r.soldAfterCallMs === null ? "burned" : `${s(r.soldAfterCallMs)} after`}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <ul className="clues">
          {a.clues.map((c) => <li key={c}>{c}</li>)}
        </ul>
      </div>
    </div>
  );
}
