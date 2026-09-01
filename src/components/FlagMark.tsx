const STRIPES: Record<string, string[]> = {
  de: ["#000000", "#dd0000", "#ffce00"],
  nl: ["#ae1c28", "#ffffff", "#21468b"],
  ru: ["#ffffff", "#0039a6", "#d52b1e"],
  at: ["#ed2939", "#ffffff", "#ed2939"],
  am: ["#d90012", "#0033a0", "#f2a800"],
  ee: ["#0072ce", "#000000", "#ffffff"],
  lt: ["#fdb913", "#006a44", "#c1272d"],
  hu: ["#ce2939", "#ffffff", "#477050"],
  lu: ["#ed2939", "#ffffff", "#00a1de"],
  bg: ["#ffffff", "#00966e", "#d62612"],
  ye: ["#ce1126", "#ffffff", "#000000"],
};

const CROSSED: Record<string, { bg: string; cross: string }> = {
  se: { bg: "#006aa7", cross: "#fecc00" },
  no: { bg: "#ba0c2f", cross: "#ffffff" },
  dk: { bg: "#c60c30", cross: "#ffffff" },
  fi: { bg: "#ffffff", cross: "#003580" },
  ch: { bg: "#ff0000", cross: "#ffffff" },
};

const BICOLOR: Record<string, [string, string]> = {
  ua: ["#0057b7", "#ffd700"],
  pl: ["#ffffff", "#dc143c"],
  id: ["#ff0000", "#ffffff"],
  mc: ["#ce1126", "#ffffff"],
};

const SOLID: Record<string, string> = {
  tr: "#e30a17",
  jp: "#ffffff",
  kr: "#ffffff",
  cn: "#de2910",
};

export function FlagMark({ code, className }: { code: string; className?: string }) {
  const cc = code.toLowerCase();
  return (
    <span
      className={className}
      title={cc.toUpperCase()}
      style={{
        display: "inline-flex",
        width: 28,
        height: 20,
        borderRadius: 4,
        overflow: "hidden",
        boxShadow: "inset 0 0 0 1px rgb(0 0 0 / 0.18)",
        flexShrink: 0,
      }}
    >
      <svg viewBox="0 0 28 20" width="28" height="20" aria-hidden>
        {renderFlag(cc)}
      </svg>
    </span>
  );
}

function renderFlag(cc: string) {
  if (STRIPES[cc]) {
    const h = 20 / STRIPES[cc].length;
    return STRIPES[cc].map((c, i) => <rect key={c + i} x={0} y={i * h} width={28} height={h + 0.3} fill={c} />);
  }
  if (BICOLOR[cc]) {
    const [a, b] = BICOLOR[cc];
    return (
      <>
        <rect width={28} height={10} fill={a} />
        <rect y={10} width={28} height={10} fill={b} />
      </>
    );
  }
  if (CROSSED[cc]) {
    const { bg, cross } = CROSSED[cc];
    if (cc === "ch") {
      return (
        <>
          <rect width={28} height={20} fill={bg} />
          <rect x={11} y={4} width={6} height={12} fill={cross} />
          <rect x={8} y={7} width={12} height={6} fill={cross} />
        </>
      );
    }
    return (
      <>
        <rect width={28} height={20} fill={bg} />
        <rect x={8} y={0} width={4} height={20} fill={cross} />
        <rect x={0} y={8} width={28} height={4} fill={cross} />
      </>
    );
  }
  if (cc === "fr" || cc === "it" || cc === "be" || cc === "ie" || cc === "ro") {
    const colors =
      cc === "fr"
        ? ["#0055a4", "#ffffff", "#ef4135"]
        : cc === "it"
          ? ["#009246", "#ffffff", "#ce2b37"]
          : cc === "be"
            ? ["#000000", "#fada5e", "#ef3340"]
            : cc === "ie"
              ? ["#169b62", "#ffffff", "#ff883e"]
              : ["#002b7f", "#fcd116", "#ce1126"];
    return colors.map((c, i) => <rect key={c} x={i * (28 / 3)} width={28 / 3 + 0.2} height={20} fill={c} />);
  }
  if (cc === "us") {
    return (
      <>
        <rect width={28} height={20} fill="#bf0a30" />
        {[1, 3, 5, 7, 9].map((i) => (
          <rect key={i} y={i * 2} width={28} height={2} fill="#ffffff" />
        ))}
        <rect width={12} height={11} fill="#002868" />
      </>
    );
  }
  if (cc === "gb") {
    return (
      <>
        <rect width={28} height={20} fill="#012169" />
        <path d="M0 0 L28 20 M28 0 L0 20" stroke="#fff" strokeWidth={4} />
        <path d="M0 0 L28 20 M28 0 L0 20" stroke="#c8102e" strokeWidth={1.5} />
        <rect x={12} width={4} height={20} fill="#fff" />
        <rect y={8} width={28} height={4} fill="#fff" />
        <rect x={13} width={2} height={20} fill="#c8102e" />
        <rect y={9} width={28} height={2} fill="#c8102e" />
      </>
    );
  }
  if (cc === "jp") {
    return (
      <>
        <rect width={28} height={20} fill="#fff" />
        <circle cx={14} cy={10} r={5.5} fill="#bc002d" />
      </>
    );
  }
  if (cc === "tr") {
    return (
      <>
        <rect width={28} height={20} fill="#e30a17" />
        <circle cx={11} cy={10} r={5} fill="#fff" />
        <circle cx={12.4} cy={10} r={4} fill="#e30a17" />
      </>
    );
  }
  if (cc === "kz") {
    return <rect width={28} height={20} fill="#00afca" />;
  }
  if (cc === "sg") {
    return (
      <>
        <rect width={28} height={10} fill="#ef3340" />
        <rect y={10} width={28} height={10} fill="#fff" />
      </>
    );
  }
  if (SOLID[cc]) {
    return <rect width={28} height={20} fill={SOLID[cc]} />;
  }
  return (
    <>
      <rect width={28} height={20} fill="#6366f1" />
      <text x={14} y={14} textAnchor="middle" fill="#fff" fontSize={8} fontWeight={700} fontFamily="system-ui">
        {cc.toUpperCase()}
      </text>
    </>
  );
}
