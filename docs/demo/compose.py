import json, subprocess, os, sys, hashlib, re
F = os.path.expanduser("~/.local/bin/ffmpeg")
FP = os.path.expanduser("~/.local/bin/ffprobe") if os.path.exists(os.path.expanduser("~/.local/bin/ffprobe")) else "ffprobe"
ET = os.path.expanduser("~/.local/bin/edge-tts")
VOICE = os.environ.get("VOICE", "en-US-AndrewNeural")
RATE = os.environ.get("RATE", "+4%")
W = "/tmp/rec/work"; os.makedirs(W, exist_ok=True)
SRC = json.load(open("/tmp/rec/markers.json"))["video"]

# (src_a, src_b, [(spoken, subtitle)], silent_note)
SC = [
 (2.5, 13.2, [("Every day, someone online tells you which coin is about to explode.", None),
              ("Sometimes they're right. Sometimes they bought it an hour ago, and you're who they sell it to.", None)]),
 (13.2, 21.6, [("CABAL is a game where you catch that person.", None),
               ("Four callers share tips at one table. Three are honest. One is secretly running a scam.", None)]),
 (21.6, 31.4, [("The scammer pays a few fresh wallets to buy early, tells you to buy, then sells into your money.", None),
               ("Everything happens on a real blockchain, with play money. Before the round starts, the house locks the answer in a sealed envelope.", None)]),
 (31.4, 42.0, [("Here's the table. Each lane is a caller, and each row is a wallet. You get two minutes.", None),
               ("First, I'll ask mira if she has any other wallets.", "First, I'll ask @mira if she has any other wallets.")]),
 (42.0, 54.0, [("No other wallets, she says. Sure, mira.", "No other wallets, she says. Sure, @mira.")]),
 (54.0, 66.0, [("Now watch the red strings. Somebody is paying for three brand new wallets.", None),
               ("A fresh wallet has no history, which is why scammers love them. Think burner phones, but for money.", None)]),
 (66.0, 82.0, [("Those wallets all buy. Dex makes a call too, but his is honest. Meanwhile, the new wallets are out shopping.", "Those wallets all buy. @dex makes a call too, but his is honest. Meanwhile, the new wallets are out shopping.")]),
 (82.0, 98.9, [("Then mira tells everyone to buy. Her wallets already did. And now they sell, right into the crowd.", "Then @mira tells everyone to buy. Her wallets already did. And now they sell, right into the crowd."),
               ("So the people who trusted her are the ones left holding the bag.", None)]),
 (98.9, 106.0, [("Remember her answer? I click her claim, and the board lights up the three wallets she said didn't exist.", None)]),
 (106.0, 113.6, [("I open one. Paid for by mira. Bought before her call, sold after it. That's a receipt.", "I open one. Paid for by @mira. Bought before her call, sold after it. That's a receipt.")]),
 (113.6, 119.4, [("Good enough for me. I put my stake on mira.", "Good enough for me. I put my stake on @mira."),
                ("If I'm right, I get my stake back plus a share of the pot.", None)]),
 (124.5, 167.4, [("Then the clock runs out. A good time to stretch.", None)]),
 (167.4, 178.4, [("Exposed. And the blockchain backs it up.", None),
                 ("The house sealed its answer before the round began, and your browser just checked that it still matches. It had no way to cheat.", None)]),
 (178.4, 190.6, [("Her claim now reads false. And an autopsy explains, in plain words, exactly how she pulled it off.", None)]),
 (190.6, 198.1, [("Two minutes, no wallet, no real money. There are two harder levels where the cabal hides its tracks.", None),
                 ("Go catch a scammer. And please don't buy anything mira recommends.", "Go catch a scammer. And please don't buy anything @mira recommends.")]),
]
LEAD, GAP, TAIL = 0.35, 0.3, 0.45

def run(*a):
    r = subprocess.run(a, capture_output=True, text=True)
    if r.returncode: print(r.stderr[-1500:]); sys.exit(1)
    return r.stdout

def dur(p): return float(run(FP, "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", p).strip())

def tts(text):
    h = hashlib.md5((VOICE + RATE + text).encode()).hexdigest()[:12]
    p = f"{W}/tts_{h}.mp3"
    if os.path.exists(p) and subprocess.run([FP, "-v", "error", "-i", p, "-f", "null", "-"], capture_output=True).returncode: os.remove(p)
    if not os.path.exists(p):
        import time
        for k in range(5):
            r = subprocess.run([ET, "-v", VOICE, f"--rate={RATE}", "-t", text, "--write-media", p], capture_output=True, text=True)
            if r.returncode == 0 and os.path.exists(p) and subprocess.run([FP, "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", p], capture_output=True).returncode == 0: break
            print("tts retry", k, text[:40]); time.sleep(2)
        else: sys.exit("tts failed: " + text)
    return p

cues = []; t = 0.0; parts = []
for i, (a, b, lines) in enumerate(SC):
    mp = [(tts(sp), sub or sp) for sp, sub in lines]
    ds = [dur(p) for p, _ in mp]
    D = LEAD + sum(ds) + GAP * (len(ds) - 1) + TAIL
    s = b - a
    speed = s / D
    vf = f"trim={a}:{b},setpts=PTS-STARTPTS"
    if speed >= 0.8:
        vf += f",setpts=PTS*{D/s:.5f}"; pad = 0
    else:
        vf += f",setpts=PTS*{1/0.8:.5f}"; pad = D - s / 0.8
    vf += f",fps=25,scale=1440:900,tpad=stop_mode=clone:stop_duration={max(pad,0)+0.05:.3f},trim=duration={D:.3f},setpts=PTS-STARTPTS,format=yuv420p"
    # audio: delay each line
    inputs = []; fl = []; at = LEAD
    for k, ((p, sub), d) in enumerate(zip(mp, ds)):
        inputs += ["-i", p]
        fl.append(f"[{k+1}:a]adelay={int(at*1000)}|{int(at*1000)},apad[a{k}]")
        cues.append((t + at, t + at + d, sub))
        at += d + GAP
    mix = "".join(f"[a{k}]" for k in range(len(mp)))
    fl.append(f"{mix}amix=inputs={len(mp)}:normalize=0:duration=longest,atrim=duration={D:.3f},aresample=44100,aformat=channel_layouts=mono[aout]")
    out = f"{W}/sc{i:02d}.mp4"
    run(F, "-y", "-v", "error", "-i", SRC, *inputs, "-filter_complex", f"[0:v]{vf}[v];" + ";".join(fl),
        "-map", "[v]", "-map", "[aout]", "-c:v", "libx264", "-preset", "fast", "-crf", "20", "-r", "25", "-c:a", "aac", "-b:a", "128k", "-t", f"{D:.3f}", out)
    parts.append(out)
    print(f"scene {i:02d} src {s:5.1f}s -> {D:5.1f}s  speed x{speed:4.2f}" + (f" (+{pad:.1f}s hold)" if pad else ""))
    t += D

open(f"{W}/list.txt", "w").write("".join(f"file '{p}'\n" for p in parts))
run(F, "-y", "-v", "error", "-f", "concat", "-safe", "0", "-i", f"{W}/list.txt", "-c", "copy", f"{W}/joined.mp4")

def ts(x):
    h = int(x // 3600); m = int(x % 3600 // 60); s = x % 60
    return f"{h}:{m:02d}:{s:05.2f}"
def split(text, n=38):
    # break into subtitle pages of <= 2 lines worth (~2*n chars) at sentence/word boundaries
    words = text.split(); pages = []; cur = ""
    for w in words:
        if len(cur) + len(w) + 1 > n * 2 and cur: pages.append(cur); cur = w
        else: cur = (cur + " " + w).strip()
    if cur: pages.append(cur)
    return pages
ass = ["[Script Info]", "ScriptType: v4.00+", "PlayResX: 1440", "PlayResY: 900", "WrapStyle: 0", "",
       "[V4+ Styles]", "Format: Name,Fontname,Fontsize,PrimaryColour,SecondaryColour,OutlineColour,BackColour,Bold,Italic,Underline,StrikeOut,ScaleX,ScaleY,Spacing,Angle,BorderStyle,Outline,Shadow,Alignment,MarginL,MarginR,MarginV,Encoding",
       "Style: Sub,DejaVu Sans,38,&H00FFFFFF,&H00FFFFFF,&H8C0A0805,&H8C0A0805,1,0,0,0,100,100,0,0,3,9,0,2,140,140,30,1", "",
       "[Events]", "Format: Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect,Text"]
for a, b, text in cues:
    pg = split(text); tot = sum(len(p) for p in pg); x = a
    for p in pg:
        d = (b - a) * len(p) / tot
        ass.append(f"Dialogue: 0,{ts(x)},{ts(x+d)},Sub,,0,0,0,,{p}")
        x += d
open(f"{W}/subs.ass", "w").write("\n".join(ass) + "\n")
run(F, "-y", "-v", "error", "-i", f"{W}/joined.mp4", "-vf", f"subtitles={W}/subs.ass", "-c:v", "libx264", "-preset", "slow", "-crf", "23", "-c:a", "copy", "-movflags", "+faststart", "/tmp/rec/cabal-demo.mp4")
print("total", round(t, 1), "s ->", "/tmp/rec/cabal-demo.mp4", os.path.getsize("/tmp/rec/cabal-demo.mp4") // 1024, "KB")
