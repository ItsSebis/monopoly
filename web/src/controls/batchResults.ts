// Renders a finished batch run: the aggregate dashboard (batchCharts.ts)
// plus a per-game table whose rows can be replayed on the board - reusing
// the batch's own (rule_set, players, seed), not a second, event-log-driven
// renderer (docs/frontend.md's replay-pipeline-reuse decision).
import { renderBatchCharts } from "../charts/batchCharts";
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

  const table = document.createElement("table");
  const headRow = document.createElement("tr");
  ["Seed", "Winner", "Turns", ""].forEach((h) => {
    const th = document.createElement("th");
    th.textContent = h;
    headRow.appendChild(th);
  });
  table.appendChild(headRow);

  for (const game of record.per_game_summary) {
    const tr = document.createElement("tr");
    [String(game.seed), game.winner === null ? "—" : playerNames[game.winner], String(game.turns)].forEach(
      (text) => {
        const td = document.createElement("td");
        td.textContent = text;
        tr.appendChild(td);
      },
    );
    const actionTd = document.createElement("td");
    const replayButton = document.createElement("button");
    replayButton.type = "button";
    replayButton.textContent = "Replay";
    replayButton.addEventListener("click", () => onReplay(game.seed));
    actionTd.appendChild(replayButton);
    tr.appendChild(actionTd);
    table.appendChild(tr);
  }
  container.appendChild(table);
}
