// Renders a finished batch run: the aggregate dashboard (batchCharts.ts)
// plus a per-game table whose rows can be replayed on the board - reusing
// the batch's own (rule_set, players, seed), not a second, event-log-driven
// renderer (docs/frontend.md's replay-pipeline-reuse decision).
import { renderBatchCharts } from "../charts/batchCharts";
import { actionsCell, renderTable } from "../charts/domHelpers";
import type { BatchRunRecord } from "../types";

export function renderBatchResults(
  container: HTMLElement,
  record: BatchRunRecord,
  playerNames: string[],
  onReplay: (seed: number) => void,
): void {
  container.innerHTML = "";

  const chartsEl = document.createElement("div");
  container.appendChild(chartsEl);
  renderBatchCharts(chartsEl, record.aggregate_stats);

  const heading = document.createElement("h3");
  heading.textContent = "Games";
  container.appendChild(heading);

  renderTable(
    container,
    ["Seed", "Winner", "Turns", ""],
    record.per_game_summary.map((game) => {
      const replayButton = document.createElement("button");
      replayButton.type = "button";
      replayButton.textContent = "Replay";
      replayButton.addEventListener("click", () => onReplay(game.seed));
      return [
        game.seed,
        game.winner === null ? "—" : playerNames[game.winner],
        game.turns,
        actionsCell(replayButton),
      ];
    }),
  );
}
