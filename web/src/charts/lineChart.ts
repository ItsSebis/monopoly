import { Chart } from "./chartSetup";

export interface LineSeries {
  label: string;
  data: number[];
  color: string;
}

/** One line per series, sharing a common set of x-axis labels - used for net
 * worth over time (one line per player). */
export function renderLineChart(canvas: HTMLCanvasElement, labels: string[], series: LineSeries[]): Chart {
  return new Chart(canvas, {
    type: "line",
    data: {
      labels,
      datasets: series.map((s) => ({
        label: s.label,
        data: s.data,
        borderColor: s.color,
        backgroundColor: s.color,
        pointRadius: 0,
        tension: 0.1,
      })),
    },
    options: { responsive: true, scales: { y: { beginAtZero: true } } },
  });
}
