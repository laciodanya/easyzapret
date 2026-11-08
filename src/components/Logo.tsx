/** Squircle mark with a bolt. Flat, in the theme accent. */
export function Logo({ size = 36 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 512 512" aria-hidden="true">
      <rect x="36" y="36" width="440" height="440" rx="128" fill="var(--accent)" />
      <path d="M278 128 L188 278 H250 L234 384 L324 234 H262 Z" fill="#ffffff" />
    </svg>
  );
}
