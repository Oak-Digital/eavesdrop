import type { CSSProperties } from "react";
export function Icon({
  name,
  size = 20,
  style,
}: {
  name:
    | "arrow"
    | "download"
    | "wave"
    | "lock"
    | "file"
    | "mic"
    | "github"
    | "apple"
    | "windows"
    | "check"
    | "replay";
  size?: number;
  style?: CSSProperties;
}) {
  const paths = {
    arrow: <path d="M5 12h14m-6-6 6 6-6 6" />,
    download: (
      <>
        <path d="M12 3v12m-5-5 5 5 5-5M5 16v4h14v-4" />
      </>
    ),
    wave: <path d="M3 10v4m4-8v12m5-16v20m5-16v12m4-8v4" />,
    lock: (
      <>
        <rect x="5" y="10" width="14" height="11" rx="2" />
        <path d="M8 10V7a4 4 0 0 1 8 0v3m-4 5v2" />
      </>
    ),
    file: (
      <>
        <path d="M14 3H5v18h14V8l-5-5Zm0 0v5h5M8 12h8m-8 4h6" />
      </>
    ),
    mic: (
      <>
        <rect x="9" y="2" width="6" height="13" rx="3" />
        <path d="M5 10v2a7 7 0 0 0 14 0v-2m-7 9v3m-4 0h8" />
      </>
    ),
    github: (
      <path d="M9 19c-4 1-4-2-6-2m12 5v-4c0-1 .2-2-1-3 4 0 7-2 7-6 0-2-1-3-2-4 0-1 0-3-1-3-2 0-3 1-4 2a15 15 0 0 0-4 0C9 3 7 2 5 2 4 3 4 5 5 6 4 7 3 8 3 10c0 4 3 5 7 5-1 1-1 2-1 3v4" />
    ),
    apple: (
      <>
        <path d="M14 5c0-2 1-3 3-4 0 2-1 4-3 4Zm-2 2c3-2 6-1 8 1-4 2-3 6 0 7-1 4-4 7-6 6-2-1-2-1-4 0-2 1-6-4-6-9 0-5 4-7 8-5Z" />
      </>
    ),
    windows: (
      <>
        <path d="m3 5 8-1v7H3V5Zm10-1 8-1v8h-8V4ZM3 13h8v7l-8-1v-6Zm10 0h8v8l-8-1v-7Z" />
      </>
    ),
    check: <path d="m5 12 4 4L19 6" />,
    replay: (
      <>
        <path d="M3 10a9 9 0 1 1 1 7M3 4v6h6" />
      </>
    ),
  };
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      style={style}
    >
      {paths[name]}
    </svg>
  );
}
