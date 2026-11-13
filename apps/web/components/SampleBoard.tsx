/** A small, fixed illustration of the pattern the game teaches. Drawn with the same visual language as the live board. */
export function SampleBoard() {
  const lanes = [
    { y: 40, label: "@kaito", color: "#f2c230" },
    { y: 100, label: "@mira", color: "#58b6c9" },
    { y: 160, label: "@dex", color: "#ff6a3d" },
  ];
  return (
    <div className="sample" aria-label="An example of the pattern: fresh wallets that buy before a call and sell right after it">
      <div className="cap"><span>Case file · table #7 · $FROG</span><span>fig. 1</span></div>
      <svg viewBox="0 0 520 300" role="img" aria-label="Example board">
        {lanes.map((l) => (
          <g key={l.label}>
            <line x1="84" x2="510" y1={l.y} y2={l.y} stroke={l.color} strokeOpacity="0.4" />
            <rect x="8" y={l.y - 12} width="5" height="24" fill={l.color} />
            <text x="20" y={l.y + 5} fill="#efe6d3" style={{ fontFamily: "var(--f-display)", fontSize: 18, letterSpacing: "0.04em", textTransform: "uppercase" }}>{l.label}</text>
          </g>
        ))}
        {/* mira's call pin */}
        <line x1="300" x2="300" y1="100" y2="270" stroke="#58b6c9" strokeOpacity="0.5" strokeDasharray="1 4" />
        <path d="M300 103 l-8 -20 h16 z" fill="#58b6c9" />
        <circle cx="300" cy="78" r="6" fill="#58b6c9" stroke="#14110d" strokeWidth="2" />
        {/* two other calls */}
        <path d="M180 43 l-8 -20 h16 z" fill="#f2c230" /><circle cx="180" cy="18" r="6" fill="#f2c230" stroke="#14110d" strokeWidth="2" />
        <path d="M420 163 l-8 -20 h16 z" fill="#ff6a3d" /><circle cx="420" cy="138" r="6" fill="#ff6a3d" stroke="#14110d" strokeWidth="2" />
        {/* three fresh wallets funded by @mira, buying BEFORE her call, selling AFTER */}
        {[0, 1, 2].map((i) => {
          const y = 196 + i * 28;
          const fx = 150 + i * 20;
          const bx = 215 + i * 16;
          const sx = 335 + i * 26;
          return (
            <g key={i}>
              <path d={`M ${fx} 100 C ${fx + 30} 130, ${fx - 30} 170, ${fx} ${y}`} fill="none" stroke="#e5402a" strokeWidth="1.6" />
              <line x1={fx} x2={sx} y1={y} y2={y} stroke="#58b6c9" strokeOpacity="0.4" />
              <circle cx={fx} cy={y} r="5" fill="#14110d" stroke="#58b6c9" strokeWidth="1.5" />
              <rect x={bx - 5} y={y - 5} width="10" height="10" fill="#4fd18b" />
              <rect x={sx - 5} y={y - 5} width="10" height="10" fill="#ef6a52" transform={`rotate(45 ${sx} ${y})`} />
            </g>
          );
        })}
        <text x="20" y="215" fill="#7d7566" style={{ fontFamily: "var(--f-mono)", fontSize: 10 }}>fresh</text>
        <text x="20" y="228" fill="#7d7566" style={{ fontFamily: "var(--f-mono)", fontSize: 10 }}>wallets</text>
        <text x="196" y="186" fill="#e5402a" style={{ fontFamily: "var(--f-stamp)", fontSize: 13 }} transform="rotate(-4 196 186)">bought first…</text>
        <text x="336" y="290" fill="#e5402a" style={{ fontFamily: "var(--f-stamp)", fontSize: 13 }}>…sold into the crowd</text>
      </svg>
    </div>
  );
}
