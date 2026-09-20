import { Chart } from "./chartSetup";

export interface BarSeries {
  label: string;
  data: number[];
  color?: string;
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
      datasets: series.map((s) => ({ label: s.label, data: s.data, backgroundColor: s.color })),
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
