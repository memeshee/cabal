"use client";
import Link from "next/link";
import { useEffect, useState } from "react";
import { listRounds } from "@/lib/api";
import { Footer, Topbar } from "@/components/Shell";

type R = { id: number; ticker: string; status: string; accusers: number; cabal: string | null };

export default function Past() {
  const [rows, setRows] = useState<R[] | null>(null);
  const [err, setErr] = useState(false);
  useEffect(() => {
    listRounds().then(setRows).catch(() => setErr(true));
  }, []);
  return (
    <div className="shell">
      <Topbar />
      <h1 className="disp" style={{ fontSize: 56, margin: "34px 0 6px" }}>Past tables</h1>
      <p className="dim" style={{ maxWidth: 640 }}>Every table is stored on the contract, so any finished round can be replayed and its reveal re-checked. Nothing here depends on the house&apos;s memory.</p>
      {err && <div className="empty">Couldn&apos;t reach the house.</div>}
      {!rows && !err && <div className="empty">Reading the chain…</div>}
      {rows && rows.length === 0 && <div className="empty">No tables yet. <Link href="/">Deal the first one.</Link></div>}
      {rows && rows.length > 0 && (
        <div className="pastlist">
          {rows.map((r) => (
            <Link key={r.id} href={`/t/r${r.id}`}>
              <span>#{r.id}</span>
              <span>${r.ticker}</span>
              <span className="dim">{r.status === "Revealed" ? `cabal: ${r.cabal}` : r.status === "Voided" ? "voided: house failed to reveal" : "in play"}</span>
              <span className="faint">{r.accusers} accusing</span>
            </Link>
          ))}
        </div>
      )}
      <Footer />
    </div>
  );
}
