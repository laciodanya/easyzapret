/** Squircle mark with a bolt. Flat, in the theme accent. */
export function Logo({ size = 36 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 512 512" aria-hidden="true">
      <rect width="512" height="512" rx="120" fill="var(--accent)" />
      <path d="M232 76 H354 L275 213 H373 L175 443 L230 272 H151 Z" fill="#ffffff" />
    </svg>
  );
}
