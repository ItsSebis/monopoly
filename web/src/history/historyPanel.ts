// The server-backed history browser (docs/frontend.md#history-browser):
// lists archived runs, falls back to the localStorage cache when the server
// can't be reached, and opens a detail view (stats charts + a board replay
// reusing the same worker/WasmGame pipeline as live play).
import { deleteRun, getRun, getRuns } from "../api";
import { renderBatchResults } from "../controls/batchResults";
import { renderSingleRunCharts } from "../charts/singleRunCharts";
import { actionsCell, renderTable } from "../charts/domHelpers";
import { listRecent, pushRecent, removeRecent, summaryOf } from "./recentRunsCache";
import type { PlayerConfig, RunDetail, RunSummary, RuleSet } from "../types";

export interface HistoryPanelHandlers {
  onReplay: (ruleSet: RuleSet, players: PlayerConfig[], seed: bigint, playerNames: string[]) => void;
}

function resultFor(run: RunSummary): string {
  if (run.kind === "batch") return `${run.games ?? "?"} games`;
  if (run.winner == null) return "No winner";
  return run.players[run.winner]?.name ?? `Player ${run.winner}`;
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

    renderTable(
      this.listEl,
      ["Created", "Kind", "Strategies", "Result", ""],
      runs.map((run) => {
        const openButton = document.createElement("button");
        openButton.type = "button";
        openButton.textContent = "Open";
        openButton.addEventListener("click", () => this.open(run.id));
        const deleteButton = document.createElement("button");
        deleteButton.type = "button";
        deleteButton.textContent = "Delete";
        deleteButton.addEventListener("click", () => this.remove(run.id));

        return [
          run.created_at,
          run.kind,
          run.players.map((p) => p.strategy).join(", "),
          resultFor(run),
          actionsCell(openButton, deleteButton),
        ];
      }),
    );
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
