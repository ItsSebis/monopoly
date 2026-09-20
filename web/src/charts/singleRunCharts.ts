// Renders every per-run metric in docs/analysis-and-metrics.md's "Per-run
// metrics" table from a `PerGameStats`. Purely a projection of numbers the
// engine already computed onto charts/tables - no stat is recomputed here.
import { spaceName } from "../board/layout";
import { renderBarChart } from "./barChart";
import { colorForIndex } from "./colors";
import { canvasIn, chartSection, renderTable } from "./domHelpers";
import { renderBoardHeatmap } from "./heatmap";
import { renderLineChart } from "./lineChart";
import type { PerGameStats } from "../types";

const CASH_FLOW_CATEGORIES: { key: keyof PerGameStats["cash_flow"][number]; label: string }[] = [
  { key: "rent_paid", label: "Rent paid" },
  { key: "rent_received", label: "Rent received" },
  { key: "tax_paid", label: "Tax paid" },
  { key: "card_net", label: "Card net" },
  { key: "go_salary_collected", label: "GO salary" },
];

export function renderSingleRunCharts(container: HTMLElement, stats: PerGameStats, playerNames: string[]): void {
  container.innerHTML = "";

  const summary = chartSection(container, "Result");
  const winner = stats.winner === null ? "No winner (max turns reached)" : playerNames[stats.winner];
  summary.appendChild(document.createTextNode(`${winner} — ${stats.turns} turns`));

  renderLineChart(
    canvasIn(chartSection(container, "Net worth over time")),
    stats.net_worth_by_turn.map((_, i) => String(i)),
    playerNames.map((name, i) => ({
      label: name,
      data: stats.net_worth_by_turn.map((row) => row[i]),
      color: colorForIndex(i),
    })),
  );

  renderBarChart(
    canvasIn(chartSection(container, "Cash flow breakdown")),
    playerNames,
    CASH_FLOW_CATEGORIES.map((c) => ({
      label: c.label,
      data: stats.cash_flow.map((flow) => flow[c.key] as number),
    })),
    { stacked: true },
  );

  const timelineSection = chartSection(container, "Property & monopoly timeline");
  renderTable(
    timelineSection,
    ["Turn", "Property", "Owner"],
    stats.property_timeline.map((p) => [p.turn, spaceName(p.space), playerNames[p.owner]]),
  );
  renderTable(
    timelineSection,
    ["Turn", "Monopoly completed", "Owner"],
    stats.monopolies_completed.map((m) => [m.turn, String(m.group), playerNames[m.owner]]),
  );

  renderTable(
    chartSection(container, "Bankruptcies"),
    ["Turn", "Player", "Paid to"],
    stats.bankruptcies.map((b) => [
      b.turn,
      playerNames[b.player],
      b.payee === null ? "the bank" : playerNames[b.payee],
    ]),
  );

  renderBarChart(
    canvasIn(chartSection(container, "Dice roll distribution")),
    Array.from({ length: 11 }, (_, i) => String(i + 2)),
    [{ label: "Rolls", data: stats.dice_roll_counts }],
  );

  renderBoardHeatmap(chartSection(container, "Landing distribution"), stats.landing_counts);

  const roiByProperty = stats.property_roi
    .map((r) => ({ ...r, roi: r.cost_basis > 0 ? r.rent_collected / r.cost_basis : 0 }))
    .sort((a, b) => b.roi - a.roi);
  renderBarChart(
    canvasIn(chartSection(container, "ROI by property")),
    roiByProperty.map((r) => spaceName(r.space)),
    [{ label: "Rent collected / cost basis", data: roiByProperty.map((r) => r.roi) }],
    { indexAxis: "y" },
  );
}
