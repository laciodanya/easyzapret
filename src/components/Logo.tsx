/** Squircle mark with a bolt. Flat, in the theme accent. */
export function Logo({ size = 36 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 512 512" aria-hidden="true">
      <rect x="16" y="16" width="480" height="480" rx="116" fill="var(--accent)" />
      <path d="M282.8 99.8 L173 282.8 L248.7 282.8 L229.2 412.2 L339 229.2 L263.3 229.2 Z" fill="#ffffff" />
    </svg>
  );
}
