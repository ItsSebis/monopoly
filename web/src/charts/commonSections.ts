// The dice-roll and landing-distribution sections are identical in shape and
// meaning whether the counts come from one game (`PerGameStats`) or a whole
// batch (`AggregateStats`) - singleRunCharts.ts and batchCharts.ts share this
// one function instead of each rendering their own copy.
import { renderBarChart } from "./barChart";
import { canvasIn, chartSection, trackChart } from "./domHelpers";
import { renderBoardHeatmap } from "./heatmap";

export function renderDiceAndLandingSections(
  container: HTMLElement,
  diceRollCounts: number[],
  landingCounts: number[],
): void {
  trackChart(
    container,
    renderBarChart(
      canvasIn(chartSection(container, "Dice roll distribution")),
      Array.from({ length: 11 }, (_, i) => String(i + 2)),
      [{ label: "Rolls", data: diceRollCounts }],
    ),
  );

  renderBoardHeatmap(chartSection(container, "Landing distribution"), landingCounts);
}
