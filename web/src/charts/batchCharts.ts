// Renders every batch-only metric in docs/analysis-and-metrics.md's
// "Batch-only metrics" table from an `AggregateStats`. Purely a projection
// of numbers the engine already computed onto charts/tables.
import { renderBarChart } from "./barChart";
import { canvasIn, chartSection, destroyCharts, trackChart } from "./domHelpers";
import { colorFor, renderBoardHeatmap } from "./heatmap";
import { bucket } from "./histogram";
import type { AggregateStats } from "../types";

function sortedKeys(map: Record<string, unknown>): string[] {
  return Object.keys(map).sort();
}

function renderHistogramSection(container: HTMLElement, title: string, values: number[]): void {
  const { labels, counts } = bucket(values);
  const section = chartSection(container, title);
  if (labels.length === 0) {
    section.appendChild(document.createTextNode("No data"));
    return;
  }
  trackChart(container, renderBarChart(canvasIn(section), labels, [{ label: title, data: counts }]));
}

export function renderBatchCharts(container: HTMLElement, stats: AggregateStats): void {
  destroyCharts(container);
  container.innerHTML = "";

  const summary = chartSection(container, "Result");
  summary.appendChild(document.createTextNode(`${stats.games} games`));

  const strategies = sortedKeys(stats.win_rate_by_strategy);

  trackChart(
    container,
    renderBarChart(
      canvasIn(chartSection(container, "Win rate by strategy")),
      strategies,
      [{ label: "Win rate", data: strategies.map((s) => stats.win_rate_by_strategy[s] * 100) }],
    ),
  );

  const matrixSection = chartSection(container, "Strategy head-to-head");
  const table = document.createElement("table");
  table.className = "matrix-table";
  const headRow = document.createElement("tr");
  headRow.appendChild(document.createElement("th"));
  strategies.forEach((s) => {
    const th = document.createElement("th");
    th.textContent = s;
    headRow.appendChild(th);
  });
  table.appendChild(headRow);
  strategies.forEach((row) => {
    const tr = document.createElement("tr");
    const rowHeader = document.createElement("th");
    rowHeader.textContent = row;
    tr.appendChild(rowHeader);
    strategies.forEach((col) => {
      const cell = stats.head_to_head[row]?.[col];
      const td = document.createElement("td");
      if (cell && cell.total > 0) {
        const winRate = (cell.wins / cell.total) * 100;
        td.textContent = `${winRate.toFixed(0)}% (${cell.wins}/${cell.total})`;
        td.style.background = colorFor(winRate, 100);
      } else {
        td.textContent = "–";
      }
      tr.appendChild(td);
    });
    table.appendChild(tr);
  });
  matrixSection.appendChild(table);

  trackChart(
    container,
    renderBarChart(
      canvasIn(chartSection(container, "ROI by strategy")),
      strategies,
      [{ label: "Rent collected / cost basis", data: strategies.map((s) => stats.roi_by_strategy[s]) }],
    ),
  );

  trackChart(
    container,
    renderBarChart(
      canvasIn(chartSection(container, "Final net worth by strategy")),
      strategies,
      (["p10", "median", "mean", "p90"] as const).map((key) => ({
        label: key,
        data: strategies.map((s) => stats.final_net_worth_by_strategy[s][key]),
      })),
    ),
  );

  renderHistogramSection(container, "Game length distribution", stats.game_length);
  renderHistogramSection(container, "Bankruptcy turn distribution", stats.bankruptcy_turns);

  trackChart(
    container,
    renderBarChart(
      canvasIn(chartSection(container, "Dice roll distribution")),
      Array.from({ length: 11 }, (_, i) => String(i + 2)),
      [{ label: "Rolls", data: stats.dice_roll_counts }],
    ),
  );

  renderBoardHeatmap(chartSection(container, "Landing distribution"), stats.landing_counts);
}
