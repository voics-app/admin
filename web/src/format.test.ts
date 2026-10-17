import { describe, expect, it } from "vitest";
import { formatBitrate, formatCount, formatMilliseconds, formatMinutes, formatPercent, formatRate } from "./format";

describe("format", () => {
  it("shows n/a for values the BFF could not provide", () => {
    expect(formatCount(null)).toBe("n/a");
    expect(formatPercent(undefined)).toBe("n/a");
    expect(formatMilliseconds(null)).toBe("n/a");
    expect(formatMinutes(null)).toBe("n/a");
    expect(formatRate(null)).toBe("n/a");
  });

  it("groups thousands", () => {
    expect(formatCount(1234567)).toBe("1,234,567");
  });

  it("renders ratios as percentages", () => {
    expect(formatPercent(0.0123)).toBe("1.23%");
  });

  it("switches latency units at one second", () => {
    expect(formatMilliseconds(84.4)).toBe("84 ms");
    expect(formatMilliseconds(1500)).toBe("1.50 s");
  });

  it("scales durations", () => {
    expect(formatMinutes(42)).toBe("42 min");
    expect(formatMinutes(90)).toBe("1.5 h");
    expect(formatMinutes(60 * 48)).toBe("2.0 d");
  });

  it("scales bitrates", () => {
    expect(formatBitrate(512)).toBe("512 bit/s");
    expect(formatBitrate(2_500_000)).toBe("2.5 Mbit/s");
  });
});
