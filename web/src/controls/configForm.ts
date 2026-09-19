// The config screen is a direct editor for RuleSet + PlayerConfig
// (docs/frontend.md) - every field has a form control, and submitting
// produces exactly the JSON GameConfig shape the engine expects, no
// UI-specific intermediate format.
import type { GameConfig, IncomeTaxMode, PlayerConfig, RuleSet } from "../types";

export interface StartPayload {
  config: GameConfig;
  seed: number;
}

// The next three functions are pure `FormData` -> engine-shape mappings,
// exported so their tests don't need a DOM.

export function buildIncomeTaxMode(data: FormData): IncomeTaxMode {
  const flatAmount = Number(data.get("income_tax_flat_amount"));
  const rate = Number(data.get("income_tax_rate"));
  switch (data.get("income_tax_mode")) {
    case "flat":
      return { mode: "flat", amount: flatAmount };
    case "percentage":
      return { mode: "percentage", rate };
    default:
      return { mode: "choice", flat_amount: flatAmount };
  }
}

export function buildRuleSet(data: FormData): RuleSet {
  return {
    starting_cash: Number(data.get("starting_cash")),
    go_salary: Number(data.get("go_salary")),
    jail_fine: Number(data.get("jail_fine")),
    luxury_tax: Number(data.get("luxury_tax")),
    income_tax_mode: buildIncomeTaxMode(data),
    even_build_rule: data.get("even_build_rule") === "on",
    auction_on_decline: data.get("auction_on_decline") === "on",
    free_parking_pot: data.get("free_parking_pot") === "on",
    max_turns: data.get("max_turns") ? Number(data.get("max_turns")) : null,
  };
}

export function buildPlayers(rows: { name: string; strategy: string }[]): PlayerConfig[] {
  return rows.map((row) => ({ name: row.name, strategy: row.strategy }));
}

export class ConfigForm {
  private strategyIds: string[] = [];
  private readonly playerRowsEl: HTMLElement;
  private readonly errorEl: HTMLElement;
  private rowCount = 0;

  constructor(
    private readonly formEl: HTMLFormElement,
    private readonly onStart: (payload: StartPayload) => void,
  ) {
    this.playerRowsEl = formEl.querySelector("#player-rows")!;
    this.errorEl = formEl.querySelector("#config-error")!;
    formEl.querySelector("#add-player")!.addEventListener("click", () => this.addPlayerRow());
    formEl.addEventListener("submit", (event) => {
      event.preventDefault();
      this.submit();
    });
  }

  /** Called once the worker reports the engine's registered strategy ids
   * (docs/frontend.md's "strategy dropdown"); seeds the form with 2 default
   * player rows and enables controls that need those ids to make sense. */
  setStrategyIds(ids: string[]): void {
    this.strategyIds = ids;
    this.addPlayerRow();
    this.addPlayerRow();
    this.formEl.querySelector<HTMLButtonElement>("#add-player")!.disabled = false;
    this.formEl.querySelector<HTMLButtonElement>("#start-button")!.disabled = false;
    this.formEl.querySelector<HTMLElement>("#loading-note")!.hidden = true;
  }

  showError(message: string): void {
    this.errorEl.textContent = message;
  }

  /** Re-disables controls that depend on a worker's `ready` message - used
   * when starting a fresh worker for a new game, so Start can't race ahead
   * of that worker's own wasm init. */
  awaitReady(): void {
    this.formEl.querySelector<HTMLButtonElement>("#add-player")!.disabled = true;
    this.formEl.querySelector<HTMLButtonElement>("#start-button")!.disabled = true;
    this.formEl.querySelector<HTMLElement>("#loading-note")!.hidden = false;
    this.playerRowsEl.innerHTML = "";
    this.rowCount = 0;
  }

  private addPlayerRow(): void {
    const index = this.rowCount++;
    const row = document.createElement("div");
    row.className = "player-row";

    const nameInput = document.createElement("input");
    nameInput.type = "text";
    nameInput.value = `P${index + 1}`;
    nameInput.required = true;
    nameInput.className = "player-name";

    const strategySelect = document.createElement("select");
    strategySelect.className = "player-strategy";
    for (const id of this.strategyIds) {
      const option = document.createElement("option");
      option.value = id;
      option.textContent = id;
      strategySelect.appendChild(option);
    }

    const removeButton = document.createElement("button");
    removeButton.type = "button";
    removeButton.textContent = "Remove";
    removeButton.addEventListener("click", () => row.remove());

    row.append(nameInput, strategySelect, removeButton);
    this.playerRowsEl.appendChild(row);
  }

  private submit(): void {
    this.errorEl.textContent = "";
    const rows = Array.from(this.playerRowsEl.querySelectorAll<HTMLElement>(".player-row")).map((row) => ({
      name: row.querySelector<HTMLInputElement>(".player-name")!.value,
      strategy: row.querySelector<HTMLSelectElement>(".player-strategy")!.value,
    }));
    if (rows.length < 2) {
      this.showError("Need at least 2 players.");
      return;
    }

    const data = new FormData(this.formEl);
    const rules = buildRuleSet(data);
    const players = buildPlayers(rows);

    const seedInput = data.get("seed");
    const seed = seedInput ? Number(seedInput) : Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);

    this.onStart({ config: { rules, players }, seed });
  }
}
