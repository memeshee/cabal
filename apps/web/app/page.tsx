"use client";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { ensureGuest, newTable } from "@/lib/api";
import { Footer, Topbar } from "@/components/Shell";
import { SampleBoard } from "@/components/SampleBoard";

export default function Home() {
  const router = useRouter();
  const [busy, setBusy] = useState<"play" | "demo" | null>(null);
  const [err, setErr] = useState<string | null>(null);

  async function start(demo: boolean) {
    setBusy(demo ? "demo" : "play");
    setErr(null);
    try {
      await ensureGuest();
      const key = await newTable(demo);
      router.push(`/t/${key}`);
    } catch (e) {
      setErr((e as Error).message);
      setBusy(null);
    }
  }

  return (
    <div className="shell">
      <Topbar />
      <section className="hero">
        <div>
          <h1>
            One of them is <em>dumping</em> on you.
          </h1>
          <p className="lede">
            A three-minute memecoin game. Four callers hype the same coin. <b>One of them is the cabal</b>: they buy first through fresh wallets, then sell into everyone who followed their call. The chain shows who paid for those wallets. The callers will lie.
          </p>
          <div className="cta-row">
            <button className="btn go big" onClick={() => start(false)} disabled={!!busy}>{busy === "play" ? "Dealing your table…" : "Take a seat"}</button>
            <button className="btn big" onClick={() => start(true)} disabled={!!busy} title="Always deals the same table, so you can compare notes or follow the README">{busy === "demo" ? "Dealing…" : "Play the demo table"}</button>
          </div>
          <small>No wallet. No sign-up. You get play money on NEAR testnet.</small>
          {err && <p className="mono" role="alert" style={{ color: "var(--red)", marginTop: 14 }}>{err}</p>}
        </div>
        <SampleBoard />
      </section>

      <section className="claim" aria-label="Why this game">
        <div>
          <h4>The real pattern</h4>
          <div className="q">A top trader was caught pulling <span className="red">$1.7M</span> out of followers through side wallets.</div>
        </div>
        <div>
          <h4>The game</h4>
          <div className="q">Learn to read it <span className="red">before</span> it&apos;s your money.</div>
        </div>
      </section>

      <section id="how" className="steps" aria-label="How a table works">
        <div className="step">
          <div className="n">1</div>
          <h3>Read the board</h3>
          <p>Every call, every trade and every &ldquo;who paid for this wallet&rdquo; is on the chain, drawn as strings between wallets and callers.</p>
        </div>
        <div className="step">
          <div className="n">2</div>
          <h3>Question them</h3>
          <p>Ask any caller anything. They stay in character, they deny everything, and honest ones sound exactly like the guilty one.</p>
        </div>
        <div className="step">
          <div className="n">3</div>
          <h3>Expose the cabal</h3>
          <p>Stake a little play money on a name. Right, and you split the pot with their burned bag. Wrong, and you pay the people who were right.</p>
        </div>
      </section>

      <section className="claim" aria-label="Why you can trust the table">
        <div>
          <h4>The house can&apos;t cheat</h4>
          <p style={{ margin: 0, color: "var(--paper-dim)" }}>Before anyone trades, the contract stores a commitment to who the cabal is. At the end the house must reveal it, and the contract rejects any answer that doesn&apos;t hash to that commitment. Your browser checks it again itself.</p>
        </div>
        <div>
          <h4>Nothing is staged</h4>
          <p style={{ margin: 0, color: "var(--paper-dim)" }}>Every wallet is a real NEAR account and every trade a real transaction. Finished tables stay replayable from the chain, so you can audit any round.</p>
        </div>
      </section>
      <Footer />
    </div>
  );
}
