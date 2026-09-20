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
});
