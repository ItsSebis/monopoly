// The server-backed history browser (docs/frontend.md#history-browser):
// lists archived runs, falls back to the localStorage cache when the server
// can't be reached, and opens a detail view (stats charts + a board replay
// reusing the same worker/WasmGame pipeline as live play).
import { deleteRun, getRun, getRuns } from "../api";
import { renderBatchResults } from "../controls/batchResults";
import { renderSingleRunCharts } from "../charts/singleRunCharts";
import { listRecent, pushRecent, removeRecent, summaryOf } from "./recentRunsCache";
import type { PlayerConfig, RunDetail, RunSummary, RuleSet } from "../types";

export interface HistoryPanelHandlers {
  onReplay: (ruleSet: RuleSet, players: PlayerConfig[], seed: number, playerNames: string[]) => void;
}

export class HistoryPanel {
  private readonly listEl: HTMLElement;
  private readonly detailEl: HTMLElement;

  constructor(
    container: HTMLElement,
    private readonly handlers: HistoryPanelHandlers,
  ) {
    this.listEl = container.querySelector("#history-list")!;
    this.detailEl = container.querySelector("#history-detail")!;
  }

  async refresh(): Promise<void> {
    this.detailEl.innerHTML = "";
    this.listEl.textContent = "Loading…";
    try {
      this.renderList(await getRuns(), false);
    } catch {
      this.renderList(listRecent(), true);
    }
  }

  private renderList(runs: RunSummary[], offline: boolean): void {
    this.listEl.innerHTML = "";
    if (offline) {
      const note = document.createElement("p");
      note.textContent = "Server unreachable — showing recently viewed runs from this browser only.";
      this.listEl.appendChild(note);
    }

    const table = document.createElement("table");
    const headRow = document.createElement("tr");
    ["Created", "Kind", "Strategies", "Result", ""].forEach((h) => {
      const th = document.createElement("th");
      th.textContent = h;
      headRow.appendChild(th);
    });
    table.appendChild(headRow);

    for (const run of runs) {
      const tr = document.createElement("tr");
      const result =
        run.kind === "single"
          ? run.winner == null
            ? "No winner"
            : (run.players[run.winner]?.name ?? `Player ${run.winner}`)
          : `${run.games ?? "?"} games`;
      [run.created_at, run.kind, run.players.map((p) => p.strategy).join(", "), result].forEach((text) => {
        const td = document.createElement("td");
        td.textContent = text;
        tr.appendChild(td);
      });

      const actionTd = document.createElement("td");
      const openButton = document.createElement("button");
      openButton.type = "button";
      openButton.textContent = "Open";
      openButton.addEventListener("click", () => this.open(run.id));
      const deleteButton = document.createElement("button");
      deleteButton.type = "button";
      deleteButton.textContent = "Delete";
      deleteButton.addEventListener("click", () => this.remove(run.id));
      actionTd.append(openButton, deleteButton);
      tr.appendChild(actionTd);
      table.appendChild(tr);
    }
    this.listEl.appendChild(table);
  }

  private async open(id: string): Promise<void> {
    this.detailEl.textContent = "Loading…";
    try {
      const detail = await getRun(id);
      pushRecent(summaryOf(detail));
      this.renderDetail(detail);
    } catch (err) {
      this.detailEl.textContent = err instanceof Error ? err.message : "Failed to load run.";
    }
  }

  private renderDetail(detail: RunDetail): void {
    this.detailEl.innerHTML = "";
    const playerNames = detail.players.map((p) => p.name);

    if (detail.kind === "single") {
      const replayButton = document.createElement("button");
      replayButton.type = "button";
      replayButton.textContent = "Replay on board";
      replayButton.addEventListener("click", () =>
        this.handlers.onReplay(detail.rule_set, detail.players, detail.seed, playerNames),
      );
      this.detailEl.appendChild(replayButton);

      const chartsEl = document.createElement("div");
      this.detailEl.appendChild(chartsEl);
      renderSingleRunCharts(chartsEl, detail.final_stats, playerNames);
    } else {
      renderBatchResults(this.detailEl, detail, playerNames, (seed) =>
        this.handlers.onReplay(detail.rule_set, detail.players, seed, playerNames),
      );
    }
  }

  private async remove(id: string): Promise<void> {
    try {
      await deleteRun(id);
    } catch (err) {
      this.detailEl.textContent = err instanceof Error ? err.message : "Failed to delete run.";
      return;
    }
    removeRecent(id);
    await this.refresh();
  }
}
