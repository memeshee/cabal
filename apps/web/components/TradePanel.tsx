"use client";
import { useEffect, useState } from "react";
import { buy, getMe, sell } from "@/lib/api";
import { fmtNear, fmtPrice } from "@/lib/format";
import type { Me, TableView } from "@/lib/types";

const AMOUNTS = [10, 25, 50, 100];

export function TradePanel({ view, token, live, onToast, refreshKey }: { view: TableView; token: string | null; live: boolean; onToast: (m: string, kind?: "good" | "bad", link?: string | null) => void; refreshKey: number }) {
  const [me, setMe] = useState<Me | null>(null);
  const [amt, setAmt] = useState(25);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    if (!token) return;
    let alive = true;
    const load = () => getMe(view.key, token).then((m) => alive && m.ok && setMe(m)).catch(() => {});
    load();
    const t = setInterval(load, 4000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [view.key, token, refreshKey]);

  async function go(kind: "buy" | "sell", pct = 100) {
    if (!token || busy) return;
    setBusy(kind);
    const r = kind === "buy" ? await buy(view.key, token, amt) : await sell(view.key, token, pct);
    setBusy(null);
    if (r.ok) onToast(kind === "buy" ? `Bought for ${amt / 1000} NEAR` : "Sold", "good", r.explorer_url);
    else onToast(r.error ?? "That didn't go through.", "bad");
    getMe(view.key, token).then((m) => m.ok && setMe(m)).catch(() => {});
  }

  const price = view.round_view?.price;
  return (
    <div className="panel">
      <header>
        <b>Your desk</b>
        <span className="hint mono">${view.ticker} · {fmtPrice(price)} NEAR / chip</span>
      </header>
      <div className="me">
        <div>
          <div className="l">Play money</div>
          <div className="v">{me ? me.balance_near.toFixed(3) : "—"}<small className="faint"> NEAR</small></div>
        </div>
        <div>
          <div className="l">Your chips</div>
          <div className="v">{me?.chips ?? 0}<small className="faint"> ≈{fmtNear(me?.chips_value, 3)}</small></div>
        </div>
      </div>
      {live ? (
        <div className="trade">
          <div className="amounts" role="group" aria-label="Amount to buy in thousandths of a NEAR">
            {AMOUNTS.map((a) => (
              <button key={a} className={amt === a ? "on" : ""} onClick={() => setAmt(a)}>{(a / 1000).toFixed(3)}</button>
            ))}
          </div>
          <div className="two">
            <button className="btn buy" disabled={!!busy || !token} onClick={() => go("buy")}>{busy === "buy" ? "Buying…" : "Buy"}</button>
            <button className="btn sell" disabled={!!busy || !token || !me?.chips} onClick={() => go("sell", 100)}>{busy === "sell" ? "Selling…" : "Sell all"}</button>
          </div>
          <div className="quote">Trading is optional. It only moves the price the callers are working with.</div>
        </div>
      ) : (
        <div className="trade">
          <button className="btn sell" disabled={!!busy || !me?.chips} onClick={() => go("sell", 100)}>{busy === "sell" ? "Selling…" : me?.chips ? "Sell what you hold" : "Nothing to sell"}</button>
        </div>
      )}
    </div>
  );
}
