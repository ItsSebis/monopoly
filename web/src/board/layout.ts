// Purely cosmetic board metadata (names, colors, grid position) - the engine
// has no concept of any of this, only the mechanical data (price, rent,
// group) needed to run the game (see docs/architecture.md: "no second rules
// implementation" is about *decisions*, not display strings). Keyed by the
// same space index (0-39) the engine uses everywhere.

export type ColorGroup =
  | "Brown"
  | "LightBlue"
  | "Pink"
  | "Orange"
  | "Red"
  | "Yellow"
  | "Green"
  | "DarkBlue"
  | null;

export interface SpaceLayout {
  index: number;
  name: string;
  colorGroup: ColorGroup;
  row: number;
  col: number;
}

const NAMES: [string, ColorGroup][] = [
  ["GO", null],
  ["Mediterranean Avenue", "Brown"],
  ["Community Chest", null],
  ["Baltic Avenue", "Brown"],
  ["Income Tax", null],
  ["Reading Railroad", null],
  ["Oriental Avenue", "LightBlue"],
  ["Chance", null],
  ["Vermont Avenue", "LightBlue"],
  ["Connecticut Avenue", "LightBlue"],
  ["Jail / Just Visiting", null],
  ["St. Charles Place", "Pink"],
  ["Electric Company", null],
  ["States Avenue", "Pink"],
  ["Virginia Avenue", "Pink"],
  ["Pennsylvania Railroad", null],
  ["St. James Place", "Orange"],
  ["Community Chest", null],
  ["Tennessee Avenue", "Orange"],
  ["New York Avenue", "Orange"],
  ["Free Parking", null],
  ["Kentucky Avenue", "Red"],
  ["Chance", null],
  ["Indiana Avenue", "Red"],
  ["Illinois Avenue", "Red"],
  ["B&O Railroad", null],
  ["Atlantic Avenue", "Yellow"],
  ["Ventnor Avenue", "Yellow"],
  ["Water Works", null],
  ["Marvin Gardens", "Yellow"],
  ["Go To Jail", null],
  ["Pacific Avenue", "Green"],
  ["North Carolina Avenue", "Green"],
  ["Community Chest", null],
  ["Pennsylvania Avenue", "Green"],
  ["Short Line Railroad", null],
  ["Chance", null],
  ["Park Place", "DarkBlue"],
  ["Luxury Tax", null],
  ["Boardwalk", "DarkBlue"],
];

/** The classic 11x11 perimeter, GO at the bottom-right corner, going
 * counter-clockwise by increasing index (matching the engine's direction of
 * play): bottom row right-to-left, left column bottom-to-top, top row
 * left-to-right, right column top-to-bottom. */
function gridPosition(index: number): { row: number; col: number } {
  if (index <= 10) return { row: 10, col: 10 - index };
  if (index <= 20) return { row: 20 - index, col: 0 };
  if (index <= 30) return { row: 0, col: index - 20 };
  return { row: index - 30, col: 10 };
}

export const BOARD_LAYOUT: SpaceLayout[] = NAMES.map(([name, colorGroup], index) => ({
  index,
  name,
  colorGroup,
  ...gridPosition(index),
}));

export function spaceName(index: number): string {
  return BOARD_LAYOUT[index]?.name ?? `Space ${index}`;
}
