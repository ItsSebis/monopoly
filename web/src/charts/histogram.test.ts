import { describe, expect, it } from "vitest";
import { bucket } from "./histogram";

describe("bucket", () => {
  it("returns no buckets for an empty input", () => {
    expect(bucket([])).toEqual({ labels: [], counts: [] });
  });

  it("puts every value in one bucket when they're all identical", () => {
    expect(bucket([5, 5, 5])).toEqual({ labels: ["5"], counts: [3] });
  });

  it("distributes values across the requested bucket count", () => {
    const { labels, counts } = bucket([0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100], 10);
    expect(labels).toHaveLength(10);
    expect(counts.reduce((a, b) => a + b, 0)).toBe(11);
  });

  it("puts the maximum value in the last bucket, not a spillover 11th one", () => {
    const { counts } = bucket([0, 100], 10);
    expect(counts).toEqual([1, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
  });
});
