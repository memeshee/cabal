"use client";
import Link from "next/link";
import { use, useCallback, useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { ensureGuest, getMe, getRound, newTable } from "@/lib/api";
import { useTable } from "@/lib/useTable";
import { clock, fmtPrice } from "@/lib/format";
import type { ChatLine, Me, TableView } from "@/lib/types";
import { Board } from "@/components/Board";
import { Chat } from "@/components/Chat";
import { Footer, Topbar } from "@/components/Shell";
import { Reveal } from "@/components/Reveal";
import { Suspects } from "@/components/Suspects";
import { Tape } from "@/components/Tape";
import { Toasts, useToasts } from "@/components/Toasts";
import { TradePanel } from "@/components/TradePanel";
import { WalletFile } from "@/components/WalletFile";

export default function TablePage({ params }: { params: Promise<{ key: string }> }) {
  const { key } = use(params);
  return key.startsWith("r") && /^r\d+$/.test(key) ? <Replay id={Number(key.slice(1))} /> : <Live tableKey={key} />;
}

/* ───────── a past round, rebuilt purely from the contract ───────── */
function Replay({ id }: { id: number }) {
  const [view, setView] = useState<TableView | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [sel, setSel] = useState<string | null>(null);
  useEffect(() => {
    getRound(id).then(setView).catch((e: Error) => setErr(e.message));
  }, [id]);
  return (
    <div className="shell">
      <Topbar />
      {err && <div className="empty">{err} <Link href="/past">Back to past tables</Link></div>}
      {!view && !err && <div className="dealing"><div className="big">Reading the chain…</div></div>}
      {view && (
        <>
          <div className="tablebar">
            <div className="tick">${view.ticker}<small>table #{view.round} · replay from the contract</small></div>
          </div>
          <div className="grid">
            <div className="col">
              <Board view={view} selected={sel} onSelect={setSel} final />
              {view.reveal || view.phase === "voided" ? <Reveal view={view} mineSeat={null} myAccount={null} onAgain={() => {}} again={false} /> : <div className="empty">This table has not been revealed yet.</div>}
            </div>
            <div className="col">
              {sel && <WalletFile view={view} wallet={sel} onClose={() => setSel(null)} />}
              <Tape view={view} onSelect={setSel} />
            </div>
          </div>
        </>
      )}
      <Footer />
    </div>
  );
}

/* ───────── a live table ───────── */
function Live({ tableKey }: { tableKey: string }) {
  const router = useRouter();
  const { view, gone, lost, addChat } = useTable(tableKey);
  const { items, push } = useToasts();
  const [token, setToken] = useState<string | null>(null);
  const [account, setAccount] = useState<string | null>(null);
  const [sel, setSel] = useState<string | null>(null);
  const [me, setMe] = useState<Me | null>(null);
  const [tick, setTick] = useState(0);
  const [again, setAgain] = useState(false);
  const [localElapsed, setLocalElapsed] = useState<{ at: number; ms: number } | null>(null);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    ensureGuest().then((g) => { setToken(g.token); setAccount(g.account); }).catch((e: Error) => push(e.message, "bad"));
  }, [push]);

  // The server reports elapsed time on every poll; interpolate between polls so the clock ticks smoothly.
  useEffect(() => {
    if (view?.elapsed_ms != null) setLocalElapsed({ at: Date.now(), ms: view.elapsed_ms });
  }, [view?.elapsed_ms]);
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(t);
  }, []);

  // Keep "my accusation" fresh for the reveal screen and the suspects panel.
  useEffect(() => {
    if (!token || !view?.round) return;
    getMe(tableKey, token).then((m) => m.ok && setMe(m)).catch(() => {});
  }, [token, tableKey, view?.round, view?.round_view?.accusers.length, view?.phase, tick]);

  const onToast = useCallback((m: string, kind: "good" | "bad" | "info" = "info", link?: string | null) => { push(m, kind, link); setTick((t) => t + 1); }, [push]);
  const onAnswer = useCallback((l: ChatLine[]) => addChat(l), [addChat]);

  async function dealAgain() {
    setAgain(true);
    try {
      const k = await newTable(false);
      router.push(`/t/${k}`);
    } catch (e) {
      push((e as Error).message, "bad");
      setAgain(false);
    }
  }

  const phase = view?.phase;
  const elapsed = useMemo(() => (localElapsed ? localElapsed.ms + (now - localElapsed.at) : null), [localElapsed, now]);
  const live = phase === "playing";
  const final = phase === "revealed" || phase === "voided";

  if (gone) {
    return (
      <div className="shell">
        <Topbar />
        <div className="dealing">
          <div className="big">That table is gone</div>
          <p className="dim">Live tables are kept for a few minutes. Finished ones stay readable from the chain under Past tables.</p>
          <div className="cta-row" style={{ justifyContent: "center" }}>
            <Link className="btn go" href="/">Deal a new table</Link>
            <Link className="btn" href="/past">Past tables</Link>
          </div>
        </div>
      </div>
    );
  }
  if (!view) {
    return (
      <div className="shell">
        <Topbar />
        <div className="dealing"><div className="big">{lost > 3 ? "The house isn't answering…" : "Pulling up a chair…"}</div><div className="shuffle"><i /><i /><i /><i /></div></div>
      </div>
    );
  }
  if (phase === "failed") {
    return (
      <div className="shell">
        <Topbar />
        <div className="dealing">
          <div className="big">The house fumbled the deal</div>
          <p className="dim">{view.error ?? "Something went wrong opening this table."} Nothing was charged to you.</p>
          <button className="btn go big" onClick={dealAgain} disabled={again}>{again ? "Dealing…" : "Try another table"}</button>
        </div>
      </div>
    );
  }
  if (phase === "dealing") {
    return (
      <div className="shell">
        <Topbar />
        <div className="dealing">
          <div className="big">Dealing the table</div>
          <p className="dim mono">{view.note || "Shuffling the wallets…"}</p>
          <div className="shuffle"><i /><i /><i /><i /></div>
          <p className="faint mono" style={{ marginTop: 22, maxWidth: 560, marginInline: "auto" }}>Every wallet is a real testnet account. Opening a table commits the hidden roles on-chain first, so this takes about 15 seconds the first time.</p>
        </div>
      </div>
    );
  }

  const left = elapsed == null ? null : view.play_ms - elapsed;
  const revealLeft = elapsed == null ? null : view.play_ms + view.reveal_ms - elapsed;
  const hot = live && left !== null && left < 30_000;
  const pct = elapsed == null ? 0 : Math.min(100, (elapsed / view.play_ms) * 100);
  const mineSeat = me?.accusation?.seat ?? null;

  return (
    <div className="shell">
      <Topbar right={account ? <span className="chip-net mono" title="Your throwaway testnet account">you · {account.slice(0, 6)}…</span> : undefined} />
      <div className="tablebar">
        <div className="tick">${view.ticker}<small>table #{view.round}{view.demo ? " · demo table" : ""}</small></div>
        <div className="px">{fmtPrice(view.round_view?.price)}<small>NEAR / chip</small></div>
        <div className="px"><span className="dim">{view.round_view?.supply ?? 0}</span><small>chips out</small></div>
        <div className={`clock ${hot ? "hot" : ""}`}>
          {live && <><div className="t">{left === null ? "--:--" : clock(left)}</div><div className="l">until the market closes</div></>}
          {phase === "revealing" && <><div className="t">{revealLeft === null ? "--:--" : clock(revealLeft)}</div><div className="l">house must reveal</div></>}
          {final && <><div className="t">DONE</div><div className="l">table settled</div></>}
        </div>
      </div>
      {live && <div className="bar" aria-hidden="true"><i style={{ width: `${pct}%` }} /></div>}
      {lost > 3 && <div className="empty" role="alert" style={{ color: "var(--red)" }}>Lost touch with the house. Retrying…</div>}

      <div className="grid">
        <div className="col">
          <Board view={view} selected={sel} onSelect={setSel} final={final} />
          {(final || phase === "revealing") && (
            final ? (
              <Reveal view={view} mineSeat={mineSeat} myAccount={account} onAgain={dealAgain} again={again} />
            ) : (
              <div className="panel"><div className="body" style={{ textAlign: "center" }}><span className="stamp">Market closed</span><p className="dim mono" style={{ margin: "12px 0 0" }}>The house is revealing the committed roles. The contract rejects anything that doesn&apos;t match.</p></div></div>
            )
          )}
          <Tape view={view} onSelect={setSel} />
        </div>
        <div className="col">
          {sel && <WalletFile view={view} wallet={sel} onClose={() => setSel(null)} />}
          <Suspects view={view} token={token} me={me} live={live} onToast={onToast} />
          <TradePanel view={view} token={token} live={live || final} onToast={onToast} refreshKey={tick} />
          <Chat view={view} token={token} live={live} onAnswer={onAnswer} onToast={onToast} />
        </div>
      </div>
      <Toasts items={items} />
      <Footer />
    </div>
  );
}
