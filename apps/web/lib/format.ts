const YOCTO = 10n ** 24n;

/** yoctoNEAR (string) -> NEAR as a number. Precision loss is fine for display. */
export function near(y: string | number | bigint | undefined | null): number {
  if (y === undefined || y === null || y === "") return 0;
  let v: bigint;
  try {
    v = BigInt(y);
  } catch {
    return 0;
  }
  const whole = v / YOCTO;
  const frac = v % YOCTO;
  return Number(whole) + Number(frac) / 1e24;
}

export function fmtNear(y: string | number | bigint | undefined | null, digits = 3): string {
  const n = near(y);
  if (n !== 0 && Math.abs(n) < 0.001) return n.toExponential(1);
  return n.toFixed(digits);
}

/** Price per chip in NEAR from the contract's price (yocto). */
export function fmtPrice(y: string | undefined): string {
  const n = near(y);
  if (n === 0) return "0.0000";
  return n < 0.01 ? n.toFixed(5) : n.toFixed(4);
}

export function shortAcct(a: string | null | undefined): string {
  if (!a) return "";
  return a.length > 20 ? `${a.slice(0, 5)}…${a.slice(-4)}` : a;
}

/** mm:ss from milliseconds, clamped at 0. */
export function clock(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
}

/** "+36s" style offset from the start of the table. */
export function offset(ms: number): string {
  const s = Math.round(ms / 1000);
  return `${s < 0 ? "−" : "+"}${Math.abs(s)}s`;
}

export function pct(a: number, b: number): number {
  return b === 0 ? 0 : (a / b) * 100;
}
