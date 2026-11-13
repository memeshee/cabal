"use client";
import Link from "next/link";
import { use, useCallback, useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { ensureGuest, getMe, getRound, newTable } from "@/lib/api";
import { useTable } from "@/lib/useTable";
import { clock, fmtPrice } from "@/lib/format";
import { saveResult, suggestedTier, loadProgress } from "@/lib/progress";
import type { ChatLine, Me, TableView } from "@/lib/types";
import { Board } from "@/components/Board";
import { Chat } from "@/components/Chat";
import { Footer, Topbar } from "@/components/Shell";
import { Reveal } from "@/components/Reveal";
import { Suspects } from "@/components/Suspects";
import { Tape } from "@/components/Tape";
import { Autopsy } from "@/components/Autopsy";
import { ShareCard } from "@/components/ShareCard";
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
            <span className={`tier-badge ${view.tier}`}>{view.tier}</span>
          </div>
          <div className="grid">
            <div className="col">
              <Board view={view} selected={sel} onSelect={setSel} final />
              {view.reveal || view.phase === "voided" ? <Reveal view={view} mineSeat={null} myAccount={null} onAgain={() => {}} again={false} /> : <div className="empty">This table has not been revealed yet.</div>}
              {view.reveal && <Autopsy view={view} />}
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
  const [focus, setFocus] = useState<string | null>(null);
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
      const k = await newTable(false, suggestedTier(loadProgress()));
      router.push(`/t/${k}`);
    } catch (e) {
      push((e as Error).message, "bad");
      setAgain(false);
    }
  }

  const phase = view?.phase;
  const settled = phase === "revealed" ? view?.reveal?.cabal_seat ?? null : null;
  const myAccusedSeat = me?.accusation?.seat ?? null;
  useEffect(() => {
    if (settled === null || !view) return;
    // Only count tables the player actually accused in; watching is not playing.
    if (myAccusedSeat !== null) saveResult(view.tier, myAccusedSeat === settled);
  }, [settled, myAccusedSeat, view?.tier]); // eslint-disable-line react-hooks/exhaustive-deps
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
          <div className="primer">
            <div><b>1</b><span>A red string runs from whoever paid for a wallet to that wallet.</span></div>
            <div><b>2</b><span>A green square is a buy, an orange diamond a sell, a pin is a caller's call.</span></div>
            <div><b>3</b><span>The cabal's wallets buy <em>before</em> its call and sell <em>right after</em>.</span></div>
          </div>
          <p className="faint mono" style={{ marginTop: 18, maxWidth: 560, marginInline: "auto" }}>Opening a table commits the hidden roles on-chain first.</p>
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
        <span className={`tier-badge ${view.tier}`}>{view.tier}</span>
        {view.players > 1 && <span className="presence"><i />{view.players} at this table</span>}
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
          <Board view={view} selected={sel} onSelect={setSel} final={final} focus={focus} />
          {(final || phase === "revealing") && (
            final ? (
              <Reveal view={view} mineSeat={mineSeat} myAccount={account} onAgain={dealAgain} again={again} />
            ) : (
              <div className="panel"><div className="body" style={{ textAlign: "center" }}><span className="stamp">Market closed</span><p className="dim mono" style={{ margin: "12px 0 0" }}>The house is revealing the committed roles. The contract rejects anything that doesn&apos;t match.</p></div></div>
            )
          )}
          {final && view.reveal && <Autopsy view={view} />}
          {final && view.reveal && (
            <div className="panel"><div className="body">
              <ShareCard view={view} won={mineSeat === null ? null : mineSeat === view.reveal.cabal_seat} seconds={me?.accusation && mineSeat === view.reveal.cabal_seat ? Math.round(((view.round_view?.accusers.find((a) => a.who === account) as { at_ms?: number } | undefined)?.at_ms ?? 0) / 1000) || null : null} />
            </div></div>
          )}
          <Tape view={view} onSelect={setSel} />
        </div>
        <div className="col">
          {sel && <WalletFile view={view} wallet={sel} onClose={() => setSel(null)} />}
          <Suspects view={view} token={token} me={me} live={live} onToast={onToast} />
          {me?.record && me.record.played > 0 && (
            <div className="record" aria-label="Your detective record">
              Detective record: <b>{me.record.caught}</b> caught in <b>{me.record.played}</b> tables{me.record.best_ms > 0 && <> · fastest <b>{Math.round(me.record.best_ms / 1000)}s</b></>}
            </div>
          )}
          <TradePanel view={view} token={token} live={live || final} onToast={onToast} refreshKey={tick} />
          <Chat view={view} token={token} live={live} onAnswer={onAnswer} onToast={onToast} focus={focus} onFocus={setFocus} />
        </div>
      </div>
      <Toasts items={items} />
      <Footer />
    </div>
  );
}
