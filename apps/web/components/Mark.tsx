/** The CABAL mark: one green candle, circled in red pen. Same shapes as the favicon. */
export function Mark({ size = 28 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
      <rect width="64" height="64" rx="6" fill="#14110d" />
      <line x1="32" y1="9" x2="32" y2="55" stroke="#4fd18b" strokeWidth="4" strokeLinecap="square" />
      <rect x="22" y="20" width="20" height="26" fill="#4fd18b" />
      <path d="M47.5 14.5C40 6.5 22 6 13.5 16.5 5 27 8 46 20.5 53c11 6 27.5 3.5 33-10.5 3-8 1-18-3-23" fill="none" stroke="#e5402a" strokeWidth="4.2" strokeLinecap="round" />
    </svg>
  );
}
