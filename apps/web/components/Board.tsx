"use client";
import { useMemo } from "react";
import { entryLabel, layoutBoard } from "@/lib/board";
import type { TableView } from "@/lib/types";

/** The evidence board. Time runs left to right; strings run from whoever paid for a wallet to that wallet. */
export function Board({ view, selected, onSelect, final }: { view: TableView; selected: string | null; onSelect: (w: string | null) => void; final: boolean }) {
  const L = useMemo(() => layoutBoard(view, 1000, view.elapsed_ms), [view]);
  const cabalSeat = view.reveal?.cabal_seat ?? null;
  const sideSet = useMemo(() => new Set(view.reveal?.sides ?? []), [view.reveal]);
  const nSeats = view.seats.length;

  return (
    <div className="panel board">
      <header>
        <b>The board</b>
        <span className="hint">strings run from whoever paid for a wallet · click a wallet to open its file</span>
      </header>
      <svg viewBox={`0 0 ${L.width} ${L.height}`} role="img" aria-label="Evidence board: calls, funded wallets and their trades over time">
        <defs>
          <pattern id="hatch" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
            <line x1="0" y1="0" x2="0" y2="6" stroke="#3a342a" strokeWidth="1" />
          </pattern>
        </defs>

        {/* time axis */}
        {L.ticks.map((t) => (
          <g key={t.label}>
            <line x1={t.x} x2={t.x} y1={8} y2={L.height - 40} stroke="#2b261e" strokeWidth="1" strokeDasharray="2 5" />
            <text className="axis" x={t.x} y={L.height - 22} textAnchor="middle">{t.label}</text>
          </g>
        ))}

        {/* KOL lanes + the exchange lane */}
        {[...view.seats.map((s) => ({ label: s.handle, sub: "caller", color: s.color })), { label: "exchange", sub: "funds big buyers", color: "#7d7566" }].map((lane, i) => {
          const y = L.laneY[i];
          const isCabal = final && cabalSeat === i;
          return (
            <g key={lane.label}>
              <rect x={0} y={y - L.laneH / 2 + 4} width={L.width} height={L.laneH - 8} fill={i % 2 ? "transparent" : "rgba(255,255,255,0.018)"} />
              <line x1={L.padL} x2={L.width - L.padR} y1={y} y2={y} stroke={lane.color} strokeOpacity={0.35} strokeWidth="1" />
              <rect x={14} y={y - 14} width={6} height={28} fill={lane.color} />
              <text className="lane-label" x={28} y={y + 2}>{lane.label}</text>
              <text className="lane-sub" x={28} y={y + 17}>{isCabal ? "THE CABAL" : lane.sub}</text>
            </g>
          );
        })}

        {/* the buying wave: anyone who is not a seat or a funded wallet */}
        <text className="lane-sub" x={14} y={L.crowdBase - L.crowdH - 6}>copy-traders</text>
        <line x1={L.padL} x2={L.width - L.padR} y1={L.crowdBase} y2={L.crowdBase} stroke="#3a342a" />
        {L.crowd.map((b, i) => {
          const scale = 11;
          const up = Math.min(L.crowdH, b.buys * scale);
          const dn = Math.min(L.crowdH * 0.6, b.sells * scale);
          return (
            <g key={i}>
              {up > 0 && <rect x={b.x} y={L.crowdBase - up} width={b.w} height={up} fill="#4fd18b" opacity={0.75} />}
              {dn > 0 && <rect x={b.x} y={L.crowdBase} width={b.w} height={dn} fill="#ef6a52" opacity={0.75} />}
            </g>
          );
        })}

        {/* wallet rows: string from the payer, then buy / sell marks */}
        {L.rows.map((r, i) => {
          const isSelected = selected === r.wallet;
          const kolColor = r.funderIsKol ? view.seats[r.funderLane].color : "#58b6c9";
          const revealedSide = final && sideSet.has(r.wallet);
          const lab = entryLabel(view, r.dossier.funder).text;
          return (
            <g key={r.wallet} className={`wallet-hit ${revealedSide ? "side" : ""} ${isSelected ? "sel" : ""}`} onClick={() => onSelect(isSelected ? null : r.wallet)} tabIndex={0} role="button" aria-label={`Wallet ${r.wallet.slice(0, 6)} funded by ${lab}`} onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && onSelect(isSelected ? null : r.wallet)}>
              <rect x={0} y={r.y - 14} width={L.width} height={28} fill="transparent" />
              {/* span line */}
              {r.spanX && <line x1={r.spanX[0]} x2={r.spanX[1]} y1={r.y} y2={r.y} stroke={kolColor} strokeOpacity={0.35} strokeWidth="1" />}
              {/* the string */}
              <path className={`string ${r.funderIsKol ? "" : "ex"}`} d={r.stringPath} style={{ animationDelay: `${Math.min(i, 6) * 60}ms` }} />
              <circle className="wallet-dot" cx={r.fundedX} cy={r.y} r={6} stroke={isSelected ? "#f2c230" : kolColor} />
              <text className="axis" x={14} y={r.y + 4} fill={revealedSide ? "#e5402a" : undefined} style={revealedSide ? { fill: "#e5402a" } : undefined}>
                {r.wallet.slice(0, 4)}…{r.wallet.slice(-3)}
              </text>
              <text className="axis" x={104} y={r.y + 4} style={{ fill: kolColor }}>←{lab}</text>
              {r.marks.map((m, k) =>
                m.kind === "buy" ? (
                  <rect key={k} x={m.x - 5} y={r.y - 5} width={10} height={10} fill="#4fd18b" />
                ) : (
                  <rect key={k} x={m.x - 5} y={r.y - 5} width={10} height={10} fill="#ef6a52" transform={`rotate(45 ${m.x} ${r.y})`} />
                ),
              )}
            </g>
          );
        })}

        {/* call pins on top so they are never hidden */}
        {L.pins.map((p) => {
          const color = view.seats[p.seat].color;
          const y = L.laneY[p.seat];
          return (
            <g key={p.seat}>
              <line x1={p.x} x2={p.x} y1={y} y2={L.height - 40} stroke={color} strokeOpacity={0.5} strokeWidth="1" strokeDasharray="1 4" />
              <path d={`M ${p.x} ${y + 3} l -9 -22 h 18 z`} fill={color} />
              <circle cx={p.x} cy={y - 24} r={6} fill={color} stroke="#14110d" strokeWidth="2" />
              <text className="pin-text" x={p.x} y={y - 21} textAnchor="middle" style={{ fontSize: 8 }}>CALL</text>
              <title>{`${view.seats[p.seat].handle}: ${p.text}`}</title>
            </g>
          );
        })}

        {/* "now" line */}
        {L.nowX !== null && !final && <line x1={L.nowX} x2={L.nowX} y1={4} y2={L.height - 40} stroke="#f2c230" strokeWidth="1.5" />}
      </svg>
      <div className="legend">
        <span><i />paid for by a caller</span>
        <span><i className="b" />paid for by the exchange</span>
        <span><s />bought</span>
        <span><s className="sl" />sold</span>
        <span>▲ the caller&apos;s call</span>
        <span>{nSeats} callers · one is lying</span>
      </div>
    </div>
  );
}
