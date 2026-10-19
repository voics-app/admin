const compact = new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 });
const grouped = new Intl.NumberFormat("en");

export const formatCount = (value: number | null | undefined): string =>
  value == null ? "n/a" : grouped.format(value);

export const formatCompact = (value: number): string => compact.format(value);

export const formatPercent = (ratio: number | null | undefined, digits = 2): string =>
  ratio == null ? "n/a" : `${(ratio * 100).toFixed(digits)}%`;

export const formatMilliseconds = (value: number | null | undefined): string => {
  if (value == null) return "n/a";
  return value >= 1000 ? `${(value / 1000).toFixed(2)} s` : `${Math.round(value)} ms`;
};

export const formatMinutes = (minutes: number | null | undefined): string => {
  if (minutes == null) return "n/a";
  if (minutes >= 60 * 24) return `${(minutes / 60 / 24).toFixed(1)} d`;
  if (minutes >= 60) return `${(minutes / 60).toFixed(1)} h`;
  return `${Math.round(minutes)} min`;
};

export const formatBitrate = (bitsPerSecond: number): string => {
  const units = ["bit/s", "kbit/s", "Mbit/s", "Gbit/s"];
  let value = bitsPerSecond;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return `${unit === 0 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`;
};

export const formatRate = (perSecond: number | null | undefined): string =>
  perSecond == null ? "n/a" : `${perSecond < 10 ? perSecond.toFixed(2) : Math.round(perSecond)} req/s`;

export function formatTimestamp(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString("en-GB", {
    day: "2-digit",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

/** Axis tick label: time of day for short ranges, calendar day for long ones. */
export function formatTick(iso: string, range: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return range === "1h" || range === "24h"
    ? date.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit", hour12: false })
    : date.toLocaleDateString("en-GB", { day: "2-digit", month: "short" });
}
