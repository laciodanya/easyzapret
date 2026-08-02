import { useId } from "react";

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
      <circle cx="256" cy="256" r="148" fill="none" stroke="#ffffff" strokeOpacity="0.14" strokeWidth="18" />
      <path
        d="M256 148c-4 0-8 2-10 6l-78 148c-3 6 1 14 8 14h52l-18 78c-2 8 8 14 14 8l98-152c4-6-1-14-8-14h-54l22-74c2-8-4-14-12-14z"
        fill="#ffffff"
      />
    </svg>
  );
}
