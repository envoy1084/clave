/** `0x1234…cdef`: enough of an identifier to recognise it, never enough to mistake it. */
export function short(value: string, head = 6, tail = 4): string {
  if (value.length <= head + tail + 1) return value;
  return `${value.slice(0, head)}…${value.slice(-tail)}`;
}

const units: [Intl.RelativeTimeFormatUnit, number][] = [
  ["year", 31_536_000],
  ["month", 2_592_000],
  ["day", 86_400],
  ["hour", 3600],
  ["minute", 60],
];
const relative = new Intl.RelativeTimeFormat("en", { numeric: "auto" });

export function ago(timestamp: number, now = Date.now()): string {
  const seconds = Math.round((timestamp - now) / 1000);
  if (Math.abs(seconds) < 45) return "just now";
  for (const [unit, size] of units)
    if (Math.abs(seconds) >= size) return relative.format(Math.round(seconds / size), unit);
  return relative.format(Math.round(seconds / 60), "minute");
}

export const number = new Intl.NumberFormat("en");

export function date(timestamp: number): string {
  return new Intl.DateTimeFormat("en", { dateStyle: "medium", timeStyle: "short" }).format(
    timestamp,
  );
}

/** "2 of 3": how thresholds read everywhere in the app. */
export const ofN = (k: number, n: number) => `${k} of ${n}`;

/** A stable, pleasant colour per member: by slot inside a vault (always distinct), else from their identity. */
const PALETTE = [
  "#4F6300",
  "#5B5FD6",
  "#B5532F",
  "#2F7D73",
  "#8A4FA3",
  "#94741A",
  "#3E6EA8",
  "#A33F62",
];
export function memberColor(identity: string, slot?: number): string {
  if (slot !== undefined) return PALETTE[(slot - 1 + PALETTE.length) % PALETTE.length] ?? "#4F6300";
  let hash = 0;
  for (const char of identity) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  return PALETTE[hash % PALETTE.length] ?? "#4F6300";
}

export const initials = (name: string) =>
  name
    .split(/\s+/u)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part.charAt(0).toUpperCase())
    .join("") || "?";
