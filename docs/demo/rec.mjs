// Records the demo footage on the live site: cursor, callouts, markers (ms since video start).
import { chromium } from "/home/kiter/.local/node/lib/node_modules/playwright/index.mjs";
import fs from "node:fs";
const base = process.env.BASE ?? "https://cabal-near.fly.dev";
const OUT = "/tmp/rec/raw"; fs.rmSync(OUT, { recursive: true, force: true }); fs.mkdirSync(OUT, { recursive: true });
const browser = await chromium.launch();
const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, recordVideo: { dir: OUT, size: { width: 1440, height: 900 } } });
const vStart = Date.now();
const page = await ctx.newPage();
const M = {}; const mark = (k) => { M[k] = (Date.now() - vStart) / 1000; console.log(M[k].toFixed(1).padStart(6), k); };

await ctx.addInitScript(() => {
  const mk = () => {
    if (document.getElementById("__cur")) return;
    const s = document.createElement("style");
    s.textContent = `#__cur{position:fixed;left:0;top:0;width:22px;height:22px;margin:-4px 0 0 -4px;z-index:99999;pointer-events:none;transition:transform .02s linear}
#__cur svg{filter:drop-shadow(0 2px 3px rgba(0,0,0,.6))}
.__rip{position:fixed;z-index:99998;pointer-events:none;width:14px;height:14px;margin:-7px 0 0 -7px;border-radius:50%;border:3px solid #f2c230;animation:__r .55s ease-out forwards}
@keyframes __r{to{transform:scale(4);opacity:0}}
.__co{position:fixed;z-index:99997;pointer-events:none;border:3px solid #f2c230;border-radius:6px;box-shadow:0 0 0 9999px rgba(8,6,3,.28);animation:__in .35s ease-out}
.__lb{position:fixed;z-index:99997;pointer-events:none;background:#f2c230;color:#14110d;font:700 19px/1.25 var(--f-body),system-ui,sans-serif;padding:9px 14px;border-radius:6px;max-width:340px;box-shadow:0 6px 18px rgba(0,0,0,.5);animation:__in .35s ease-out}
@keyframes __in{from{opacity:0;transform:translateY(6px)}to{opacity:1;transform:none}}
.__card{position:fixed;inset:0;z-index:100000;background:#14110d;color:#efe6d3;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:18px;text-align:center;padding:40px}
.__card h1{font:400 150px/0.95 var(--f-display),Impact,sans-serif;text-transform:uppercase;letter-spacing:.03em;margin:0;color:#efe6d3}
.__card h1 b{color:#e5402a;font-weight:inherit}
.__card p{font:500 30px/1.3 var(--f-body),system-ui,sans-serif;margin:0;color:#b9af9b;max-width:900px}
.__card .u{font:700 38px var(--f-mono),monospace;color:#f2c230;margin-top:10px}
.__card.fade{animation:__out .6s ease-in forwards}@keyframes __out{to{opacity:0}}`;
    document.head.appendChild(s);
    const c = document.createElement("div"); c.id = "__cur";
    c.innerHTML = '<svg width="22" height="22" viewBox="0 0 22 22"><path d="M3 2l14 8-6 1.5L8.5 18z" fill="#fff" stroke="#14110d" stroke-width="1.6" stroke-linejoin="round"/></svg>';
    c.style.opacity = "0"; document.body.appendChild(c);
  };
  const ready = () => { mk(); };
  document.addEventListener("DOMContentLoaded", ready); if (document.body) ready();
  addEventListener("mousemove", (e) => { mk(); const c = document.getElementById("__cur"); c.style.opacity = "1"; c.style.transform = `translate(${e.clientX}px,${e.clientY}px)`; }, true);
  addEventListener("mousedown", (e) => { const r = document.createElement("div"); r.className = "__rip"; r.style.left = e.clientX + "px"; r.style.top = e.clientY + "px"; document.body.appendChild(r); setTimeout(() => r.remove(), 700); }, true);
});

const sleep = (s) => page.waitForTimeout(s * 1000);
async function move(x, y, steps = 28) { await page.mouse.move(x, y, { steps }); }
async function center(loc) { await loc.scrollIntoViewIfNeeded(); const b = await loc.boundingBox(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, b }; }
async function hclick(loc, pause = 0.35) { const { x, y } = await center(loc); await move(x, y); await sleep(pause); await page.mouse.click(x, y); }
// highlight an element with a label next to it; removed after `sec`
async function co(sel, text, sec, side = "below", opts = {}) {
  let b;
  if (sel && sel.box) b = sel.box; else {
    const loc = typeof sel === "string" ? page.locator(sel).first() : sel;
    await loc.scrollIntoViewIfNeeded().catch(() => {});
    b = await loc.boundingBox(); if (!b) return;
  }
  await page.evaluate(({ b, text, side, dim }) => {
    document.querySelectorAll(".__co,.__lb").forEach((e) => e.remove());
    const r = document.createElement("div"); r.className = "__co"; if (!dim) r.style.boxShadow = "none";
    Object.assign(r.style, { left: b.x - 4 + "px", top: b.y - 4 + "px", width: b.width + 8 + "px", height: b.height + 8 + "px" }); document.body.appendChild(r);
    const l = document.createElement("div"); l.className = "__lb"; l.textContent = text; document.body.appendChild(l);
    const w = l.getBoundingClientRect(); let x = b.x, y = b.y + b.height + 14;
    if (side === "above") y = b.y - w.height - 14; if (side === "inside") { y = b.y + 14; x = b.x + 14; }
    if (side === "left") { x = b.x - w.width - 14; y = b.y + 10; } if (side === "right") { x = b.x + b.width + 14; y = b.y + 10; }
    x = Math.max(12, Math.min(innerWidth - w.width - 12, x)); y = Math.max(12, Math.min(innerHeight - w.height - 12, y));
    l.style.left = x + "px"; l.style.top = y + "px";
  }, { b, text, side, dim: opts.dim ?? false });
  if (sec) { await sleep(sec); await clear(); }
}
const clear = () => page.evaluate(() => document.querySelectorAll(".__co,.__lb").forEach((e) => e.remove()));
const card = (html) => page.evaluate((h) => { document.querySelectorAll(".__card").forEach((e) => e.remove()); const d = document.createElement("div"); d.className = "__card"; d.innerHTML = h; document.body.appendChild(d); }, html);
const fadeCard = async () => { await page.evaluate(() => document.querySelectorAll(".__card").forEach((e) => e.classList.add("fade"))); await sleep(0.7); await page.evaluate(() => document.querySelectorAll(".__card").forEach((e) => e.remove())); };

// ---------- landing
await page.goto(base, { waitUntil: "networkidle" });
mark("landing");
await card("<h1>Ca<b>b</b>al</h1><p>Four callers. One is secretly scamming you. Catch them.</p>");
await sleep(4.5); await fadeCard(); mark("landing_clear");
await move(640, 420, 20); await sleep(2.0);
await co(".hero h1", "A memecoin detective game", 3.2, "below");
await page.mouse.wheel(0, 360); await sleep(2.2);
await page.mouse.wheel(0, -400); await sleep(1.4);
const rookie = page.getByRole("radio", { name: /rookie/i });
await co(page.locator(".tiers").first(), "Three difficulty levels. We start easy.", 3.0, "below");
await hclick(rookie); await sleep(0.8);
mark("click_play");
await hclick(page.getByRole("button", { name: /play the demo table/i }));
await page.waitForURL(/\/t\//, { timeout: 120000 });
mark("dealing");
await page.waitForSelector(".tablebar", { timeout: 180000 });
const t0 = Date.now(); mark("table_open");
const at = async (s) => { const w = s * 1000 - (Date.now() - t0); if (w > 0) await page.waitForTimeout(w); };

// ---------- table
await move(700, 300, 20);
await at(1.5); await co(".board", "Each lane is a caller. Each row is a wallet.", 5.5, "inside"); mark("board_explained");
await co(".clock", "Two minutes on the clock", 3.5, "below", { dim: false }); mark("clock_explained");
// ask @mira (so a claim exists for later)
await at(13);
await hclick(page.getByRole("tab", { name: "@mira" })); await sleep(0.6);
mark("ask_start");
await hclick(page.getByRole("button", { name: "Other wallets?", exact: true }));
await page.waitForSelector(".msg .claim", { timeout: 20000 }).catch(async (e) => { await page.screenshot({ path: "/tmp/rec/fail.png" }); console.log(await page.locator(".msg").allInnerTexts()); throw e; }); mark("claim_in");
await sleep(0.6);
await co(".msg:has(.claim)", "She says she has no other wallets. Remember that.", 4.2, "above", { dim: false }); mark("claim_explained");
await move(640, 360, 20);
// action on the board
await page.evaluate(() => window.scrollTo({ top: 0, behavior: "smooth" })); await at(25.5); await co(".board", "Watch the red strings: someone is paying for new wallets", 0, "inside"); mark("funds_start");
await at(34); await clear(); mark("funds_end");
await at(38.5); await co(".board", "Those new wallets are all buying", 0, "inside"); mark("buys_start");
await at(46); await clear(); mark("buys_end");
await at(50.5); await co(".board", "Now @mira tells everyone to buy", 0, "inside"); mark("call_start");
await at(56); await clear(); mark("call_end");
await at(58.5); await co(".board", "And the wallets sell. Into the buyers.", 0, "inside"); mark("sells_start");
await at(67.5); await clear(); mark("sells_end");
// claim → lit wallets
await hclick(page.locator(".claim").first()); await sleep(0.7);
mark("claim_click");
await page.evaluate(() => window.scrollTo({ top: 0, behavior: "smooth" })); await sleep(1.2);
const dots = await page.locator(".wallet-hit.lit .wallet-dot").evaluateAll((els) => els.map((e) => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height }; }));
const x0 = Math.min(...dots.map((d) => d.x)) - 190, y0 = Math.min(...dots.map((d) => d.y)) - 8, y1 = Math.max(...dots.map((d) => d.y + d.h)) + 8;
await co({ box: { x: x0, y: y0, width: 560, height: y1 - y0 } }, "Three wallets she said did not exist", 3.8, "below", { dim: false }); mark("claim_lit_done");
const lit = page.locator(".wallet-hit.lit").first().locator(".wallet-dot");
await hclick(lit); await page.waitForSelector(".dossier"); mark("dossier_open");
await sleep(1.2);
await co(".dossier", "Paid for by @mira. Bought before her call. Sold after.", 5.5, "left", { dim: false }); mark("dossier_done");
// accuse
const exposeBtn = page.locator(".suspect", { hasText: "@mira" }).getByRole("button", { name: /expose/i });
await co(page.locator(".suspect", { hasText: "@mira" }), "Put your stake on a name", 2.8, "left", { dim: false });
await hclick(exposeBtn); await sleep(1.0); mark("expose_dialog");
await hclick(page.getByRole("button", { name: /yes, expose/i })); mark("accused");
await page.waitForSelector(".toast.good", { timeout: 30000 }); await sleep(2.5); mark("toast_seen");
await move(900, 500, 20);
// wait for the reveal
await page.waitForSelector(".reveal .stamp", { timeout: 240000 }); mark("reveal_stamp");
await sleep(5); mark("reveal_held");
await co(".verify", "The sealed envelope matches. The house could not cheat.", 6, "above", { dim: false }); mark("verify_done");
await page.locator(".autopsy").scrollIntoViewIfNeeded(); await sleep(0.6); mark("autopsy_view");
await co(".autopsy", "A plain-words autopsy of how she did it", 6, "above", { dim: false }); mark("autopsy_done");
await page.locator(".record").scrollIntoViewIfNeeded().catch(() => {}); await sleep(3); mark("record_view");
await page.evaluate(() => window.scrollTo({ top: 0, behavior: "smooth" })); await sleep(2.5);
await card("<h1>Ca<b>b</b>al</h1><p>Two minutes. No wallet. No real money. A real blockchain.</p><div class='u'>cabal-near.fly.dev</div>");
await sleep(7); mark("outro_end");
const vp = await page.video().path();
await ctx.close(); await browser.close();
fs.writeFileSync("/tmp/rec/markers.json", JSON.stringify({ M, video: vp }, null, 1));
console.log("saved", vp);
