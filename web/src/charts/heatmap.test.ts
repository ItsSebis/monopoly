import { describe, expect, it } from "vitest";
import { colorFor } from "./heatmap";

describe("colorFor", () => {
  it("returns the lightest shade for a value of 0", () => {
    expect(colorFor(0, 100)).toBe("hsl(210, 80%, 92%)");
  });

  it("returns the darkest shade at the max value", () => {
    expect(colorFor(100, 100)).toBe("hsl(210, 80%, 35%)");
  });

  it("clamps values above max instead of exceeding the darkest shade", () => {
    expect(colorFor(200, 100)).toBe(colorFor(100, 100));
  });

  it("returns the lightest shade when max is 0 (an all-zero series)", () => {
    expect(colorFor(0, 0)).toBe("hsl(210, 80%, 92%)");
  });

  it("stretches a narrow range across the full scale when min is given", () => {
    // A series clustered between 40 and 60 should still span light-to-dark,
    // not stay clustered near the middle of the scale.
    expect(colorFor(40, 60, 40)).toBe(colorFor(0, 100));
    expect(colorFor(60, 60, 40)).toBe(colorFor(100, 100));
    expect(colorFor(50, 60, 40)).toBe(colorFor(50, 100));
  });

  it("returns the lightest shade when min equals max (a constant series)", () => {
    expect(colorFor(50, 50, 50)).toBe("hsl(210, 80%, 92%)");
  });

  it("defaults min to 0, matching the previous absolute-scale behavior", () => {
    expect(colorFor(30, 100)).toBe(colorFor(30, 100, 0));
  });
});
