"use client";
import { useRef, useState } from "react";
import { shareText } from "@/lib/autopsy";
import type { TableView } from "@/lib/types";

/** Draws a 1200x630 result card in the browser: no server, no image service. */
function draw(canvas: HTMLCanvasElement, v: TableView, won: boolean | null, seconds: number | null) {
  const ctx = canvas.getContext("2d");
  if (!ctx || !v.reveal) return;
  const W = 1200, H = 630;
  canvas.width = W;
  canvas.height = H;
  const css = getComputedStyle(document.documentElement);
  const f = (name: string, fb: string) => css.getPropertyValue(name).trim() || fb;
  const display = `800 ${"%"}px ${f("--font-display", "Impact")}, Impact, sans-serif`;
  const mono = `600 %px ${f("--font-mono", "monospace")}, monospace`;
  const font = (t: string, px: number) => t.replace("%", String(px));
  ctx.fillStyle = "#14110d";
  ctx.fillRect(0, 0, W, H);
  ctx.strokeStyle = "#3a342a";
  ctx.lineWidth = 2;
  ctx.strokeRect(24, 24, W - 48, H - 48);
  const color = v.seats[v.reveal.cabal_seat]?.color ?? "#f2c230";
  ctx.fillStyle = "#7d7566";
  ctx.font = font(mono, 22);
  ctx.fillText(`CASE FILE · TABLE #${v.round} · ${v.tier.toUpperCase()} · $${v.ticker}`, 64, 84);
  ctx.fillStyle = "#efe6d3";
  ctx.font = font(display, 150);
  ctx.fillText(v.reveal.cabal_handle.toUpperCase(), 64, 260);
  ctx.fillStyle = color;
  ctx.fillRect(64, 282, 220, 10);
  ctx.save();
  ctx.translate(780, 190);
  ctx.rotate((-6 * Math.PI) / 180);
  const stamp = won === true ? "EXPOSED" : won === false ? "WRONG" : "THE CABAL";
  const stampColor = won === true ? "#4fd18b" : "#e5402a";
  ctx.strokeStyle = stampColor;
  ctx.fillStyle = stampColor;
  ctx.lineWidth = 8;
  ctx.font = font(`400 %px ${f("--font-stamp", "Courier New")}, monospace`, 96);
  const w = ctx.measureText(stamp).width;
  ctx.strokeRect(-16, -84, w + 32, 116);
  ctx.fillText(stamp, 0, 0);
  ctx.restore();
  ctx.fillStyle = "#b9af9b";
  ctx.font = font(`500 %px ${f("--font-body", "sans-serif")}, sans-serif`, 34);
  const line = won === true ? `Caught${seconds ? ` in ${seconds}s` : ""}. Their ${v.reveal.sides.length} fresh wallets were burned.` : won === false ? "The chain had the whole story. I read it wrong." : "Three fresh wallets, bought early, dumped after the call.";
  ctx.fillText(line, 64, 380);
  ctx.fillStyle = "#efe6d3";
  ctx.font = font(display, 64);
  ctx.fillText("PEOPLE LIE. THE CHAIN DOESN'T.", 64, 520);
  ctx.fillStyle = "#f2c230";
  ctx.font = font(mono, 24);
  ctx.fillText("cabal-near.fly.dev", 64, 570);
}

export function ShareCard({ view, won, seconds }: { view: TableView; won: boolean | null; seconds: number | null }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const [copied, setCopied] = useState(false);
  const text = shareText(view, won, seconds);

  async function download() {
    const c = ref.current;
    if (!c) return;
    await document.fonts?.ready;
    draw(c, view, won, seconds);
    const a = document.createElement("a");
    a.download = `cabal-table-${view.round}.png`;
    a.href = c.toDataURL("image/png");
    a.click();
  }
  async function copy() {
    const url = `${location.origin}/t/r${view.round}`;
    try {
      await navigator.clipboard.writeText(`${text}\n${url}`);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      /* clipboard unavailable: the text is also shown below */
    }
  }
  return (
    <div className="share">
      <canvas ref={ref} width={1200} height={630} style={{ display: "none" }} />
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
        <button className="btn sm" onClick={copy}>{copied ? "Copied" : "Copy result + replay link"}</button>
        <button className="btn sm" onClick={download}>Download result card</button>
      </div>
    </div>
  );
}
