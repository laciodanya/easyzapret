const PATHS = {
  home: "M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z",
  shield: "M12 3 4 6v6c0 4.5 3.2 7.8 8 9 4.8-1.2 8-4.5 8-9V6z M9 12l2 2 4-4",
  activity: "M3 12h4l3-8 4 16 3-8h4",
  send: "M21 3 3 10.5l7 2.5 2.5 7z M21 3 10 13",
  cloud: "M7 18a4 4 0 0 1-.5-7.97A6 6 0 0 1 18 9.5a4.5 4.5 0 0 1-.5 8.5z",
  file: "M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z M14 3v5h5 M9 13h6 M9 17h6",
  sliders: "M4 7h9 M17 7h3 M4 17h3 M11 17h9 M15 4v6 M7 14v6",
  check: "m5 12 5 5 9-10",
  x: "M6 6l12 12M18 6 6 18",
  copy: "M9 9h11v11H9z M5 15H4V4h11v1",
  eye: "M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z M12 9.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z",
  eyeOff:
    "M3 3l18 18 M10.6 6.1A10 10 0 0 1 12 6c6.4 0 10 6 10 6a17 17 0 0 1-3.2 3.8 M6.5 7.6C3.9 9.4 2 12 2 12s3.6 6 10 6c1.3 0 2.5-.3 3.6-.7 M9.9 9.9a2.5 2.5 0 0 0 3.2 3.2",
  refresh:
    "M20 11a8 8 0 0 0-14.5-4.2L4 9 M4 4v5h5 M4 13a8 8 0 0 0 14.5 4.2L20 15 M20 20v-5h-5",
  power: "M12 3v9 M6.3 6.3a8 8 0 1 0 11.4 0",
  chevronDown: "m6 9 6 6 6-6",
  chevronRight: "m9 6 6 6-6 6",
  external: "M14 4h6v6 M20 4 10 14 M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5",
  alert: "M12 4 2.5 20h19z M12 10v4 M12 17.5v.01",
  info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z M12 11v5 M12 8v.01",
  download: "M12 4v11 M7 11l5 5 5-5 M5 20h14",
  search: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14z M21 21l-4.3-4.3",
  trash: "M4 7h16 M9 7V4h6v3 M6 7l1 13h10l1-13",
  folder:
    "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  flask:
    "M9 3h6 M10 3v6L4.5 19a1.5 1.5 0 0 0 1.3 2h12.4a1.5 1.5 0 0 0 1.3-2L14 9V3",
  stop: "M6 6h12v12H6z",
  play: "M7 5v14l12-7z",
  minus: "M5 12h14",
  square: "M5 5h14v14H5z",
  restore: "M8 8h11v11H8z M5 16V5h11",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({
  name,
  size = 18,
  strokeWidth = 1.75,
  className,
}: {
  name: IconName;
  size?: number;
  strokeWidth?: number;
  className?: string;
}) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
