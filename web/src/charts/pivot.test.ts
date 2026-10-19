import { describe, expect, it } from "vitest";
import { assignSlots, foldOverflow } from "./palette";
import { pivotSeries } from "./pivot";

describe("pivotSeries", () => {
  it("merges series on timestamp in chronological order", () => {
    const rows = pivotSeries([
      { label: "api", points: [["2026-10-17T10:01:00Z", 2], ["2026-10-17T10:00:00Z", 1]] },
      { label: "gateway", points: [["2026-10-17T10:00:00Z", 9]] },
    ]);
    expect(rows).toEqual([
      { t: "2026-10-17T10:00:00Z", api: 1, gateway: 9 },
      { t: "2026-10-17T10:01:00Z", api: 2 },
    ]);
  });
});

describe("palette", () => {
  it("assigns slots in label order, independent of the data order", () => {
    expect(assignSlots(["messages", "api", "gateway"])).toEqual([
      { label: "api", slot: 0 },
      { label: "gateway", slot: 1 },
      { label: "messages", slot: 2 },
    ]);
  });

  it("folds series beyond the cap instead of inventing hues", () => {
    const items = Array.from({ length: 10 }, (_, i) => ({ label: `s${i}` }));
    const { kept, folded } = foldOverflow(items);
    expect(kept).toHaveLength(8);
    expect(folded).toBe(2);
  });
});
