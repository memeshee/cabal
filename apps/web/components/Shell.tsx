import Link from "next/link";
import { Mark } from "./Mark";

export function Topbar({ right }: { right?: React.ReactNode }) {
  return (
    <div className="topbar">
      <Link href="/" className="wordmark" aria-label="CABAL home">
        <Mark />
        <b>CABAL</b>
      </Link>
      <nav>
        <Link href="/#how" className="hide-sm">How it works</Link>
        <Link href="/past" className="hide-sm">Past tables</Link>
        <span className="chip-net">NEAR testnet</span>
        {right}
      </nav>
    </div>
  );
}

export function Footer() {
  return (
    <div className="footer">
      <span>Play money on NEAR testnet. Nothing here is financial advice or real money.</span>
      <span>
        <a href="https://github.com/memeshee/cabal" target="_blank" rel="noreferrer">source</a> · built for NEAR HACKATHON
      </span>
    </div>
  );
}
