#!/usr/bin/env python3
"""Generate the README diagrams as standalone SVGs in CABAL's evidence-desk style.
Usage: python3 scripts/gen-diagrams.py   (writes docs/diagrams/*.svg)
Two diagrams, each answering one question:
  trust-model.svg  How do I know the house can't cheat?
  the-pattern.svg  What exactly is the cabal doing, and how do I see it?"""
import os
from xml.sax.saxutils import escape

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "docs", "diagrams")
os.makedirs(OUT, exist_ok=True)

INK, INK2, INK3, RULE = "#14110d", "#1b1812", "#25211a", "#3a342a"
PAPER, DIM, FAINT = "#efe6d3", "#b9af9b", "#7d7566"
HI, RED, GREEN, SELL, BLUE = "#f2c230", "#e5402a", "#4fd18b", "#ef6a52", "#58b6c9"
DISPLAY = "'Big Shoulders Display', 'Arial Narrow', Impact, sans-serif"
MONO = "'IBM Plex Mono', ui-monospace, 'SF Mono', Menlo, monospace"
BODY = "'Schibsted Grotesk', system-ui, -apple-system, 'Segoe UI', sans-serif"
STAMP = "'Special Elite', 'Courier New', monospace"


def head(w, h, title, desc):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" role="img" aria-labelledby="t d">'
        f"<title id='t'>{escape(title)}</title><desc id='d'>{escape(desc)}</desc>"
        f'<rect width="{w}" height="{h}" fill="{INK}"/>'
        f'<defs>'
        f'<marker id="a" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto"><path d="M0 1 L9 5 L0 9 z" fill="{PAPER}"/></marker>'
        f'<marker id="ar" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto"><path d="M0 1 L9 5 L0 9 z" fill="{RED}"/></marker>'
        f'<marker id="ag" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto"><path d="M0 1 L9 5 L0 9 z" fill="{GREEN}"/></marker>'
        f'</defs>'
    )


def t(x, y, s, size=13, fill=PAPER, font=BODY, anchor="start", weight="400", spacing="0", rot=None):
    tr = f' transform="rotate({rot} {x} {y})"' if rot else ""
    return (f'<text x="{x}" y="{y}" font-family="{font}" font-size="{size}" fill="{fill}" text-anchor="{anchor}" '
            f'font-weight="{weight}" letter-spacing="{spacing}"{tr}>{escape(s)}</text>')


def box(x, y, w, h, fill=INK2, stroke=RULE, sw=1):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>'


def line(x1, y1, x2, y2, stroke=PAPER, sw=1.5, dash=None, marker=None):
    d = f' stroke-dasharray="{dash}"' if dash else ""
    m = f' marker-end="url(#{marker})"' if marker else ""
    return f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{stroke}" stroke-width="{sw}"{d}{m}/>'


def caption(w, h, s):
    return t(24, h - 18, s, 11, FAINT, MONO)


# ───────────────────────── 1. trust model ─────────────────────────
def trust_model():
    w, h = 1080, 580
    o = [head(w, h, "How CABAL keeps the house honest",
              "Before play the contract stores a hash of the secret roles. After play the house must reveal them; the contract rejects a reveal that does not match the hash, or whose side wallets the cabal never funded. A silent house loses its bond.")]
    o.append(t(24, 44, "THE HOUSE CAN'T CHEAT", 34, PAPER, DISPLAY, weight="800", spacing="1.5"))
    o.append(t(24, 68, "Who the cabal is gets locked in before anyone trades. These are the only moves the house has.", 13, DIM))

    # three phases as columns
    cols = [
        (24, "1 · BEFORE PLAY", "the house commits"),
        (384, "2 · DURING PLAY", "the house can't change the answer"),
        (744, "3 · AFTER PLAY", "the house must reveal"),
    ]
    for x, title, sub in cols:
        o.append(box(x, 96, 312, 372))
        o.append(f'<rect x="{x}" y="96" width="312" height="30" fill="{INK3}"/>')
        o.append(t(x + 14, 116, title, 12, HI, MONO, weight="600", spacing="1.4"))
        o.append(t(x + 14, 146, sub, 15, PAPER, DISPLAY, weight="700", spacing="0.6"))
    # arrows between columns
    o.append(line(340, 282, 380, 282, DIM, 1.5, marker="a"))
    o.append(line(700, 282, 740, 282, DIM, 1.5, marker="a"))

    # col 1: commitment
    x = 24
    o.append(t(x + 14, 178, "The house picks the cabal and its side", 13, DIM))
    o.append(t(x + 14, 196, "wallets, plus a random salt:", 13, DIM))
    o.append(box(x + 14, 208, 284, 64, INK, RULE))
    o.append(t(x + 24, 232, "cabal = @mira", 12.5, PAPER, MONO))
    o.append(t(x + 24, 250, "sides = 3 fresh wallets", 12.5, PAPER, MONO))
    o.append(t(x + 24, 266, "salt  = 9f2c…", 12.5, FAINT, MONO))
    o.append(line(x + 156, 274, x + 156, 304, HI, 1.5, marker="a"))
    o.append(t(x + 166, 294, "sha256", 11, HI, MONO))
    o.append(box(x + 14, 306, 284, 44, INK, HI, 1.5))
    o.append(t(x + 24, 324, "commitment", 11, FAINT, MONO))
    o.append(t(x + 24, 342, "40388cb184049d82…", 13, HI, MONO, weight="600"))
    o.append(line(x + 156, 352, x + 156, 380, HI, 1.5, marker="a"))
    o.append(box(x + 14, 382, 284, 70, "#1f1a10", HI, 1.5))
    o.append(t(x + 24, 402, "CONTRACT STORES THE HASH", 11.5, HI, MONO, weight="600", spacing="0.6"))
    o.append(t(x + 24, 420, "on-chain, in open_round().", 12, PAPER))
    o.append(t(x + 24, 437, "The secret itself stays with the house.", 12, DIM))

    # col 2
    x = 384
    o.append(t(x + 14, 178, "Everything players see is a real", 13, DIM))
    o.append(t(x + 14, 196, "transaction on the contract:", 13, DIM))
    for i, (k, c, label) in enumerate([("CALL", HI, "a caller posts"), ("FUND", BLUE, "someone pays for a wallet"), ("BUY", GREEN, "a wallet buys chips"), ("SELL", SELL, "a wallet sells chips"), ("EXPOSE", RED, "you stake on a name")]):
        yy = 212 + i * 30
        o.append(box(x + 14, yy, 284, 24, INK, RULE))
        o.append(t(x + 24, yy + 16, k, 11.5, c, MONO, weight="700", spacing="0.6"))
        o.append(t(x + 96, yy + 16, label, 12.5, PAPER))
    o.append(box(x + 14, 372, 284, 80, INK, RED, 1.5))
    o.append(t(x + 24, 392, "WHAT THE HOUSE CANNOT DO", 11.5, RED, MONO, weight="600", spacing="0.6"))
    o.append(t(x + 24, 412, "× change who the cabal is", 12.5, PAPER))
    o.append(t(x + 24, 430, "× edit the tape or the funding links", 12.5, PAPER))
    o.append(t(x + 24, 446, "× pick a new cabal after seeing who you accuse", 12.5, PAPER))

    # col 3
    x = 744
    o.append(t(x + 14, 178, "The house calls reveal(), sending", 13, DIM))
    o.append(t(x + 14, 196, "the secret. The contract checks:", 13, DIM))
    checks = [
        ("sha256(secret) == commitment", "else rejected"),
        ("each side wallet was funded by the cabal", "else rejected"),
        ("sent before the reveal deadline", "else anyone can void"),
    ]
    for i, (c, e) in enumerate(checks):
        yy = 212 + i * 52
        o.append(box(x + 14, yy, 284, 44, INK, GREEN, 1.2))
        o.append(t(x + 24, yy + 19, "✓ " + c, 12.5, PAPER))
        o.append(t(x + 38, yy + 36, e, 11, FAINT, MONO))
    o.append(box(x + 14, 372, 284, 80, "#12201a", GREEN, 1.5))
    o.append(t(x + 24, 392, "THEN THE CONTRACT PAYS OUT", 11.5, GREEN, MONO, weight="600", spacing="0.6"))
    o.append(t(x + 24, 412, "right accusers: stake + share of pot", 12.5, PAPER))
    o.append(t(x + 24, 430, "the cabal's remaining bag is burned", 12.5, PAPER))
    o.append(t(x + 24, 446, "silent house: bond goes to accusers", 12.5, PAPER))

    # footer strip: browser re-check
    o.append(box(24, 484, 1032, 68, INK2, RULE))
    o.append(t(40, 504, "AND YOUR BROWSER CHECKS IT AGAIN", 11.5, HI, MONO, weight="600", spacing="0.6"))
    o.append(t(40, 522, "On the reveal screen your browser hashes the revealed secret itself (Web Crypto) and compares it to the stored commitment,", 12, DIM))
    o.append(t(40, 538, "instead of trusting a flag from the house.", 12, DIM))
    o.append("</svg>")
    return "".join(o)


# ───────────────────────── 2. the pattern ─────────────────────────
def the_pattern():
    w, h = 1080, 520
    o = [head(w, h, "What the cabal does, and how the board shows it",
              "A caller pays for fresh wallets. Those wallets buy before the caller's call and sell into the copy-traders after it. A friend wallet paid for by an honest caller buys only after that caller's call, and a wallet paid for by the exchange is ruled out by its funder.")]
    o.append(t(24, 44, "THE PATTERN", 34, PAPER, DISPLAY, weight="800", spacing="1.5"))
    o.append(t(24, 68, "Time runs left to right. A red string runs from whoever paid for a wallet to that wallet.", 13, DIM))

    x0, x1 = 190, 1040  # plot area
    def X(sec):  # 0..180 s
        return x0 + (x1 - x0) * sec / 180.0
    for s in (0, 30, 60, 90, 120, 150, 180):
        o.append(line(X(s), 96, X(s), 452, RULE, 1, dash="2 5"))
        o.append(t(X(s), 470, f"{s}s", 11, FAINT, MONO, anchor="middle"))

    lanes = [("@mira", BLUE, 120), ("@oren", "#b6a1ff", 176), ("exchange", FAINT, 232)]
    for name, col, y in lanes:
        o.append(f'<rect x="24" y="{y - 14}" width="6" height="28" fill="{col}"/>')
        o.append(t(40, y + 6, name, 20, PAPER, DISPLAY, weight="700", spacing="0.8"))
        o.append(line(x0, y, x1, y, col, 1, 0))
    # calls
    def pin(sec, y, col):
        s = [f'<path d="M {X(sec)} {y + 3} l -8 -20 h 16 z" fill="{col}"/>',
             f'<circle cx="{X(sec)}" cy="{y - 24}" r="6" fill="{col}" stroke="{INK}" stroke-width="2"/>']
        return "".join(s)
    o.append(pin(65, 120, BLUE))
    o.append(t(X(65), 86, "@mira calls", 11.5, BLUE, MONO, anchor="middle", weight="600"))
    o.append(pin(118, 176, "#b6a1ff"))
    o.append(t(X(118), 142, "@oren calls", 11.5, "#b6a1ff", MONO, anchor="middle", weight="600"))
    o.append(line(X(65), 124, X(65), 452, BLUE, 1, dash="1 4"))
    o.append(line(X(118), 180, X(118), 452, "#b6a1ff", 1, dash="1 4"))

    def wallet(y, fund_sec, from_y, string_col, buys, sells, label, dashed=False):
        parts = []
        fx = X(fund_sec)
        bend = 26
        d = f"M {fx} {from_y} C {fx + bend} {from_y + (y - from_y) * 0.3}, {fx - bend} {from_y + (y - from_y) * 0.7}, {fx} {y}"
        dash = ' stroke-dasharray="4 4"' if dashed else ""
        parts.append(f'<path d="{d}" fill="none" stroke="{string_col}" stroke-width="1.6"{dash}/>')
        parts.append(f'<circle cx="{fx}" cy="{y}" r="6" fill="{INK}" stroke="{string_col}" stroke-width="1.6"/>')
        xs = [fx] + [X(s) for s in buys + sells]
        parts.append(line(min(xs), y, max(xs), y, string_col, 1, 0))
        for s in buys:
            parts.append(f'<rect x="{X(s) - 5}" y="{y - 5}" width="10" height="10" fill="{GREEN}"/>')
        for s in sells:
            parts.append(f'<rect x="{X(s) - 5}" y="{y - 5}" width="10" height="10" fill="{SELL}" transform="rotate(45 {X(s)} {y})"/>')
        parts.append(t(40, y + 4, label, 11.5, DIM, MONO))
        return "".join(parts)

    # cabal's three wallets
    o.append(wallet(290, 34, 120, RED, [49], [72], "fresh wallet #1"))
    o.append(wallet(318, 36, 120, RED, [52], [75], "fresh wallet #2"))
    o.append(wallet(346, 39, 120, RED, [53], [78], "fresh wallet #3"))
    # friend wallet (honest)
    o.append(wallet(386, 98, 176, "#b6a1ff", [130], [], "friend of @oren"))
    # exchange whale
    o.append(wallet(420, 5, 232, BLUE, [13], [146], "exchange whale", dashed=True))

    # annotations
    o.append(f'<rect x="{X(34) - 6}" y="278" width="{X(58) - X(34) + 16}" height="82" fill="none" stroke="{RED}" stroke-width="1.2" stroke-dasharray="3 3"/>')
    o.append(t(X(34), 272, "① @mira pays for 3 wallets", 12.5, RED, STAMP))
    o.append(t(X(33), 376, "② they buy first…", 12.5, RED, STAMP))
    o.append(t(X(68), 376, "③ …then sell into the crowd after her call", 12.5, RED, STAMP))
    o.append(t(X(104), 408, "buys only after @oren's own call: ruled out", 11.5, "#b6a1ff", MONO))
    o.append(t(X(20), 444, "funded by the exchange, not a caller: ruled out", 11.5, BLUE, MONO))

    # legend
    o.append(f'<rect x="24" y="486" width="9" height="9" fill="{GREEN}"/>' + t(40, 495, "bought", 11, DIM, MONO))
    o.append(f'<rect x="104" y="486" width="9" height="9" fill="{SELL}" transform="rotate(45 108 490)"/>' + t(122, 495, "sold", 11, DIM, MONO))
    o.append(line(180, 490, 204, 490, RED, 1.6) + t(212, 495, "paid for by a caller", 11, DIM, MONO))
    o.append(line(360, 490, 384, 490, BLUE, 1.6, dash="4 4") + t(392, 495, "paid for by the exchange", 11, DIM, MONO))
    o.append("</svg>")
    return "".join(o)


for name, fn in [("trust-model", trust_model), ("the-pattern", the_pattern)]:
    with open(os.path.join(OUT, f"{name}.svg"), "w") as f:
        f.write(fn())
    print("wrote", name)
