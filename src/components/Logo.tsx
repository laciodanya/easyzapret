import { useId } from "react";

/** Squircle mark with a centered bolt — no inner ring. */
export function Logo({ size = 36 }: { size?: number }) {
  const gid = useId().replace(/:/g, "");
  return (
    <svg width={size} height={size} viewBox="0 0 512 512" aria-hidden>
      <defs>
        <linearGradient id={gid} x1="0.15" y1="0" x2="0.9" y2="1">
          <stop offset="0" stopColor="#A78BFA" />
          <stop offset="0.55" stopColor="#7C5CFA" />
          <stop offset="1" stopColor="#5B3FD4" />
        </linearGradient>
      </defs>
      <rect x="36" y="36" width="440" height="440" rx="128" fill={`url(#${gid})`} />
      {/* Bolt optically centered in the squircle */}
      <path
        d="M278 128 L188 278 H250 L234 384 L324 234 H262 Z"
        fill="#ffffff"
      />
    </svg>
  );
}
