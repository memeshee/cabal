"use client";
import { useEffect, useRef } from "react";
import { entryLabel } from "@/lib/board";
import { fmtNear, offset } from "@/lib/format";
import type { TableView } from "@/lib/types";

/** The raw on-chain feed. Every row links to the wallet's file on the board. */
export function Tape({ view, onSelect }: { view: TableView; onSelect: (w: string) => void }) {
  const open = view.round_view?.opened_ms ?? 0;
  const ref = useRef<HTMLDivElement>(null);
  const n = view.feed.length;
  useEffect(() => {
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [n]);
  const rows = view.feed;
  return (
    <div className="panel">
      <header>
        <b>The tape</b>
        <span className="hint">straight from the contract · {n} entries</span>
      </header>
      <div className="tape" ref={ref}>
        {rows.length === 0 && <div className="empty">Waiting for the first trade…</div>}
        {rows.map((e) => {
          const who = entryLabel(view, e.actor);
          const other = e.other ? entryLabel(view, e.other) : null;
          const isWallet = !who.kol && e.actor.length === 64;
          let body: React.ReactNode;
          switch (e.kind) {
            case "Call":
              body = (
                <>
                  <span className="who kol">{who.text}</span> <span className="call-text">“{e.text}”</span>
                </>
              );
              break;
            case "Fund":
              body = (
                <>
                  <span className={`who ${who.kol ? "kol" : ""}`} onClick={() => !who.kol && isWallet && onSelect(e.actor)}>{who.text}</span> paid for{" "}
                  <span className="who" onClick={() => e.other && onSelect(e.other)}>{other?.text}</span>
                </>
              );
              break;
            case "Buy":
            case "Sell":
              body = (
                <>
                  <span className={`who ${who.kol ? "kol" : ""}`} onClick={() => isWallet && onSelect(e.actor)}>{who.text}</span> {e.kind === "Buy" ? "bought" : "sold"} {e.chips} chips
                </>
              );
              break;
            case "Expose":
              body = (
                <>
                  <span className="who">{who.text}</span> accused <b>{view.seats[e.chips]?.handle}</b>
                </>
              );
              break;
            case "Open":
              body = <>table opened · roles committed</>;
              break;
            case "Reveal":
              body = <>house revealed the roles</>;
              break;
            case "Confiscate":
              body = <>{who.text} bag burned · {e.chips} chips</>;
              break;
            default:
              body = <>{e.kind}</>;
          }
          const amt = e.kind === "Buy" || e.kind === "Sell" ? fmtNear(e.near, 3) : e.kind === "Expose" || e.kind === "Fund" || e.kind === "Open" ? fmtNear(e.near, 3) : "";
          return (
            <div className="row" key={e.idx}>
              <span className="t">{offset(e.ts_ms - open)}</span>
              <span className={`k ${e.kind}`}>{e.kind.toUpperCase()}</span>
              <span>{body}</span>
              <span className="amt">{amt}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
