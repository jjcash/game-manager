import type { Section } from "../App";

const paths: Record<Section, string> = {
  Library: "M4 4h6v16H4z M14 4h6v6h-6z M14 14h6v6h-6z",
  Mods: "m12 3 9 5-9 5-9-5 9-5z M3 12l9 5 9-5 M3 16l9 5 9-5",
  Settings:
    "M9 4h6l1 3 3 1 1 5-2 2-1 4-5 1-3-2-4-1-1-5 2-2 1-4 2-2z M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0z",
};

export function Icon({ name }: { name: Section }) {
  return (
    <svg
      width="22"
      height="22"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={paths[name]} />
    </svg>
  );
}
