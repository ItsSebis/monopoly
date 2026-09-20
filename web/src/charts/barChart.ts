import { Chart } from "./chartSetup";
import { colorForIndex } from "./colors";

export interface BarSeries {
  label: string;
  data: number[];
  /** A single color for the whole series, or one color per bar (for a
   * single-series chart comparing named categories, e.g. "win rate by
   * strategy") - Chart.js only auto-colors by *dataset*, never by bar
   * within one dataset, so a single-series chart with no color here renders
   * every bar in the same flat default color. */
  color?: string | string[];
}

/** A bar chart with one or more series - used directly for win-rate/ROI/dice
 * bars, and with `stacked: true` for cash-flow-by-category. */
export function renderBarChart(
  canvas: HTMLCanvasElement,
  labels: string[],
  series: BarSeries[],
  opts: { stacked?: boolean; indexAxis?: "x" | "y" } = {},
): Chart {
  return new Chart(canvas, {
    type: "bar",
    data: {
      labels,
      // Falls back to the same qualitative palette used everywhere else
      // (net worth lines, player tokens) when a caller doesn't pick a color:
      // Chart.js only auto-colors per *dataset*, so a bar chart with no
      // color at all renders every bar in one flat default shade.
      datasets: series.map((s, i) => ({ label: s.label, data: s.data, backgroundColor: s.color ?? colorForIndex(i) })),
    },
    options: {
      indexAxis: opts.indexAxis ?? "x",
      responsive: true,
      scales: {
        x: { stacked: opts.stacked ?? false },
        y: { stacked: opts.stacked ?? false, beginAtZero: true },
      },
    },
  });
}
