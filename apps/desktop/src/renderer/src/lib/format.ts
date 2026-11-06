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

const L1_NAMES: Record<number, string> = {
  1: "Ethereum",
  11155111: "Sepolia",
  31337: "Anvil (local)",
};

/** "Sepolia (11155111)": the L1 an Aztec node settles to, by name where we know it. */
export function l1Name(chainId: number | undefined): string {
  if (chainId === undefined) return "unknown L1";
  const name = L1_NAMES[chainId];
  return name ? `${name}, ${chainId}` : `chain ${chainId}`;
}

/** Base units to a decimal string with the token's decimals ("1.5" for 1500000000000000000n at 18). */
export function formatUnits(value: string | bigint, decimals: number): string {
  const v = BigInt(value);
  if (decimals === 0) return v.toString();
  const base = 10n ** BigInt(decimals);
  const fraction = (v % base).toString().padStart(decimals, "0").replace(/0+$/u, "");
  return `${number.format(v / base)}${fraction ? `.${fraction}` : ""}`;
}

/** A decimal amount typed by the member to base units; undefined if it is not a valid positive amount. */
export function parseUnits(value: string, decimals: number): bigint | undefined {
  const match = /^(\d+)(?:\.(\d+))?$/u.exec(value.trim().replaceAll(",", ""));
  if (!match) return undefined;
  const [, whole = "0", fraction = ""] = match;
  if (fraction.length > decimals) return undefined;
  const base =
    BigInt(whole) * 10n ** BigInt(decimals) + BigInt(fraction.padEnd(decimals, "0") || "0");
  return base > 0n ? base : undefined;
}

/** Seconds until a unix timestamp, as "in 6d 4h" or "3h ago". */
export function until(timestamp: number, now = Date.now() / 1000): string {
  const delta = Math.round(timestamp - now);
  const abs = Math.abs(delta);
  const d = Math.floor(abs / 86_400);
  const h = Math.floor((abs % 86_400) / 3600);
  const m = Math.floor((abs % 3600) / 60);
  const text = d > 0 ? `${d}d ${h}h` : h > 0 ? `${h}h ${m}m` : `${m}m`;
  return delta >= 0 ? `in ${text}` : `${text} ago`;
}
