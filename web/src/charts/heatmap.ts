// Chart.js has no matrix/heatmap chart type, so the board heatmap (dice
// landing frequency, per-property values) and the strategy head-to-head
// matrix are plain CSS-colored grids instead of a second charting library
// (docs/frontend.md's Phase 6 scoping decision).
import { BOARD_LAYOUT } from "../board/layout";

/** A single-hue intensity scale, `value` relative to `max` (0 when `max` is
 * 0, so an all-zero series renders as the scale's lightest color rather than
 * dividing by zero). Pure so the color mapping is unit-testable. */
export function colorFor(value: number, max: number): string {
  const ratio = max > 0 ? Math.min(1, value / max) : 0;
  const lightness = 92 - ratio * 57; // 92% (near-white) down to 35% (saturated)
  return `hsl(210, 80%, ${lightness}%)`;
}

/** Renders a 40-space grid shaded by `values[space]` - reuses the same
 * layout table the live board uses, but with no tokens/ownership since this
 * is a read-only overlay, not a game in progress. */
export function renderBoardHeatmap(container: HTMLElement, values: number[]): void {
  container.innerHTML = "";
  container.className = "board-grid";
  const max = Math.max(0, ...values);
  for (const space of BOARD_LAYOUT) {
    const value = values[space.index] ?? 0;
    const cell = document.createElement("div");
    cell.className = "heatmap-space";
    cell.style.gridRow = String(space.row + 1);
    cell.style.gridColumn = String(space.col + 1);
    cell.style.background = colorFor(value, max);
    cell.title = `${space.name}: ${value}`;
    cell.textContent = String(value);
    container.appendChild(cell);
  }
}
