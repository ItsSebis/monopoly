import { describe, expect, it } from "vitest";
import { BOARD_LAYOUT, spaceName } from "./layout";

describe("BOARD_LAYOUT", () => {
  it("has exactly 40 entries, indexed 0-39 with no gaps or duplicates", () => {
    expect(BOARD_LAYOUT).toHaveLength(40);
    expect(BOARD_LAYOUT.map((s) => s.index)).toEqual([...Array(40).keys()]);
  });

  it("every grid position is within the 11x11 perimeter and unique", () => {
    const positions = new Set<string>();
    for (const space of BOARD_LAYOUT) {
      expect(space.row).toBeGreaterThanOrEqual(0);
      expect(space.row).toBeLessThanOrEqual(10);
      expect(space.col).toBeGreaterThanOrEqual(0);
      expect(space.col).toBeLessThanOrEqual(10);
      const key = `${space.row},${space.col}`;
      expect(positions.has(key)).toBe(false);
      positions.add(key);
    }
  });

  it("matches the engine's known space indices (docs/game-rules.md)", () => {
    expect(BOARD_LAYOUT[0].name).toBe("GO");
    expect(BOARD_LAYOUT[0]).toMatchObject({ row: 10, col: 10 });
    expect(BOARD_LAYOUT[10]).toMatchObject({ row: 10, col: 0 }); // Jail
    expect(BOARD_LAYOUT[20]).toMatchObject({ row: 0, col: 0 }); // Free Parking
    expect(BOARD_LAYOUT[30]).toMatchObject({ row: 0, col: 10 }); // Go To Jail
    // Railroads: RAILROAD_SPACES = [5, 15, 25, 35]
    for (const space of [5, 15, 25, 35]) {
      expect(BOARD_LAYOUT[space].name).toMatch(/Railroad|Short Line/);
      expect(BOARD_LAYOUT[space].colorGroup).toBeNull();
    }
    // Utilities: UTILITY_SPACES = [12, 28]
    for (const space of [12, 28]) {
      expect(BOARD_LAYOUT[space].colorGroup).toBeNull();
    }
  });
});

describe("spaceName", () => {
  it("returns the known name for a valid index", () => {
    expect(spaceName(39)).toBe("Boardwalk");
  });
});
