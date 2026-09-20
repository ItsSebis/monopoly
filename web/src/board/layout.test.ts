import { describe, expect, it } from "vitest";
import { BOARD_LAYOUT, displayName, spaceName } from "./layout";

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

describe("nameDe / displayName", () => {
  it("has a German name for every one of the 40 spaces, non-empty", () => {
    for (const space of BOARD_LAYOUT) {
      expect(space.nameDe.length).toBeGreaterThan(0);
    }
  });

  it("lines up special-space categories with NAMES's positions, guarding against a transcription mistake in the German list", () => {
    // Chance/Community Chest/railroads/utilities each share one German name
    // across all their occurrences - if the German list were shifted or
    // reordered relative to NAMES, at least one of these would land on a
    // colored street index instead (colorGroup wouldn't be null) or vice versa.
    const chance = [7, 22, 36];
    const communityChest = [2, 17, 33];
    const railroads = [5, 15, 25, 35];
    const utilities = [12, 28];
    for (const space of [...chance, ...communityChest, ...railroads, ...utilities]) {
      expect(BOARD_LAYOUT[space].colorGroup).toBeNull();
    }
    for (const space of chance) expect(BOARD_LAYOUT[space].nameDe).toBe("Ereignisfeld");
    for (const space of communityChest) expect(BOARD_LAYOUT[space].nameDe).toBe("Gemeinschaftsfeld");
    for (const space of railroads) expect(BOARD_LAYOUT[space].nameDe).toMatch(/bahnhof/i);
    for (const space of utilities) expect(BOARD_LAYOUT[space].nameDe).toMatch(/werk/i);
    // Every colored street has its own distinct German name (not a leftover
    // special-space placeholder bleeding into a street position).
    for (const space of BOARD_LAYOUT) {
      if (space.colorGroup !== null) {
        expect(["Ereignisfeld", "Gemeinschaftsfeld"]).not.toContain(space.nameDe);
      }
    }
  });

  it("displayName picks English or German by lang, defaulting neither", () => {
    const boardwalk = BOARD_LAYOUT[39];
    expect(displayName(boardwalk, "en")).toBe("Boardwalk");
    expect(displayName(boardwalk, "de")).toBe("Schlossallee");
  });
});
