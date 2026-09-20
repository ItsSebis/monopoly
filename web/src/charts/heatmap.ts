// Chart.js has no matrix/heatmap chart type, so the board heatmap (dice
// landing frequency, per-property values) and the strategy head-to-head
// matrix are plain CSS-colored grids instead of a second charting library
// (docs/frontend.md's Phase 6 scoping decision).
import { BOARD_LAYOUT } from "../board/layout";

/** A single-hue intensity scale, `value` relative to the `[min, max]` range
 * (0 when the range is empty, so an all-equal series renders as the scale's
 * lightest color rather than dividing by zero). `min` defaults to 0, so
 * every existing absolute-scale caller (e.g. a 0-100 win rate) is
 * unaffected; a caller with a narrow-range series (e.g. landing counts that
 * never approach 0) passes the series' own minimum so the visible range
 * actually stretches across the full scale instead of clustering in one
 * shade. Pure so the color mapping is unit-testable. */
export function colorFor(value: number, max: number, min: number = 0): string {
  const span = max - min;
  const ratio = span > 0 ? Math.min(1, Math.max(0, (value - min) / span)) : 0;
  const lightness = 92 - ratio * 57; // 92% (near-white) down to 35% (saturated)
  return `hsl(210, 80%, ${lightness}%)`;
}

/** Renders a 40-space grid shaded by `values[space]` into its own child
 * element - reuses the same layout table the live board uses, but with no
 * tokens/ownership since this is a read-only overlay, not a game in
 * progress. Appends a new grid rather than repurposing `container` itself,
 * since callers pass a `chartSection()` result that already holds a
 * heading - clearing/reclassing `container` directly would wipe it. */
export function renderBoardHeatmap(container: HTMLElement, values: number[]): void {
  const grid = document.createElement("div");
  grid.className = "board-grid";
  const max = Math.max(0, ...values);
  const min = values.length > 0 ? Math.min(...values) : 0;
  for (const space of BOARD_LAYOUT) {
    const value = values[space.index] ?? 0;
    const cell = document.createElement("div");
    cell.className = "heatmap-space";
    cell.style.gridRow = String(space.row + 1);
    cell.style.gridColumn = String(space.col + 1);
    cell.style.background = colorFor(value, max, min);
    cell.title = `${space.name}: ${value}`;
    cell.textContent = String(value);
    grid.appendChild(cell);
  }
  container.appendChild(grid);
}
