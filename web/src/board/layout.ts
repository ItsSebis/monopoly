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
  nameDe: string;
  colorGroup: ColorGroup;
  row: number;
  col: number;
}

/** The board-only language toggle's two options - see `getBoardLang`/
 * `setBoardLang`/`displayName` below. Deliberately doesn't touch
 * `spaceName()`: the event log, the stats-chart board heatmap, and
 * single-run property charts all keep using English regardless of this
 * setting (the user asked for the board display only). */
export type BoardLang = "en" | "de";

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

/** The German standard edition's names, index-for-index matching `NAMES`
 * (same 40 slots, same color groups/special spaces) - street/tax/card/
 * corner names are cross-checked against the German edition's official
 * price list (each name's price matches its English counterpart's price
 * exactly, e.g. Badstraße/Turmstraße at 60 like Mediterranean/Baltic,
 * through Schlossallee at 400 like Boardwalk), which pins them unambiguously
 * even where multiple German-language sources disagreed on board order.
 * The one exception: the 4 railroad names (all priced identically at 200,
 * like their English counterparts, so price can't disambiguate order among
 * them) are assigned Südbahnhof/Westbahnhof/Nordbahnhof/Hauptbahnhof in that
 * position order as a best-effort guess - sources agree on the 4 names but
 * not on which occupies which of the 4 slots. `layout.test.ts` only checks
 * structural shape (color group/special-space alignment with `NAMES`), not
 * these specific 4 names, since that's the part not independently verified. */
const NAMES_DE: string[] = [
  "Los",
  "Badstraße",
  "Gemeinschaftsfeld",
  "Turmstraße",
  "Einkommensteuer",
  "Südbahnhof",
  "Chausseestraße",
  "Ereignisfeld",
  "Elisenstraße",
  "Poststraße",
  "Gefängnis / Nur zu Besuch",
  "Seestraße",
  "Elektrizitätswerk",
  "Hafenstraße",
  "Neue Straße",
  "Westbahnhof",
  "Münchener Straße",
  "Gemeinschaftsfeld",
  "Wiener Straße",
  "Berliner Straße",
  "Frei Parken",
  "Theaterstraße",
  "Ereignisfeld",
  "Museumstraße",
  "Opernplatz",
  "Nordbahnhof",
  "Lessingstraße",
  "Schillerstraße",
  "Wasserwerk",
  "Goethestraße",
  "Gehe ins Gefängnis",
  "Rathausplatz",
  "Hauptstraße",
  "Gemeinschaftsfeld",
  "Bahnhofstraße",
  "Hauptbahnhof",
  "Ereignisfeld",
  "Parkstraße",
  "Zusatzsteuer",
  "Schlossallee",
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
  nameDe: NAMES_DE[index],
  colorGroup,
  ...gridPosition(index),
}));

export function spaceName(index: number): string {
  return BOARD_LAYOUT[index]?.name ?? `Space ${index}`;
}

/** English (this project's one existing, un-language-tagged name) or the
 * German standard-edition name - used only by the live board (`board.ts`);
 * every other space-name consumer keeps calling `spaceName()` directly. */
export function displayName(space: SpaceLayout, lang: BoardLang): string {
  return lang === "de" ? space.nameDe : space.name;
}

const BOARD_LANG_KEY = "monopoly:boardLang";

/** Narrows an arbitrary stored or `<select>` value to a `BoardLang`,
 * English for anything unrecognized. */
export function parseBoardLang(value: string | null): BoardLang {
  return value === "de" ? "de" : "en";
}

/** localStorage-backed, mirroring `api.ts`'s `getServerUrl`/`setServerUrl`
 * pattern - the board language is a per-viewer convenience, not game state. */
export function getBoardLang(): BoardLang {
  try {
    return parseBoardLang(localStorage.getItem(BOARD_LANG_KEY));
  } catch {
    return "en";
  }
}

export function setBoardLang(lang: BoardLang): void {
  try {
    localStorage.setItem(BOARD_LANG_KEY, lang);
  } catch {
    // Best-effort persistence only - a private window or blocked storage
    // just means the choice won't survive a reload, not a broken feature.
  }
}
