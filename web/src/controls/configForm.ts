// The config screen is a direct editor for RuleSet + PlayerConfig
// (docs/frontend.md) - every field has a form control, and submitting
// produces exactly the JSON GameConfig shape the engine expects, no
// UI-specific intermediate format.
import type { GameConfig, IncomeTaxMode, PlayerConfig, RuleSet } from "../types";

/** `seed` is a `bigint`, matching every other seed in the app (see
 * types.ts's `SingleRunRecord.seed`) so `startReplay()` has one seed type
 * regardless of whether a game is freshly started or replayed. */
export interface StartPayload {
  config: GameConfig;
  seed: bigint;
}

export interface BatchPayload {
  ruleSet: RuleSet;
  players: PlayerConfig[];
  gameCount: number;
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

/** `game_count` is only read in "batch" mode - a plain `Number()` mapping,
 * kept alongside `buildRuleSet`/`buildPlayers` so it's covered by the same
 * DOM-free pure-function tests. */
export function buildGameCount(data: FormData): number {
  return Number(data.get("game_count"));
}

export interface ConfigFormHandlers {
  onStart: (payload: StartPayload) => void;
  onRunBatch: (payload: BatchPayload) => void;
}

export class ConfigForm {
  private strategyIds: string[] = [];
  private readonly playerRowsEl: HTMLElement;
  private readonly errorEl: HTMLElement;
  private readonly startButton: HTMLButtonElement;
  private readonly seedField: HTMLElement;
  private readonly gameCountField: HTMLElement;
  private rowCount = 0;

  constructor(
    private readonly formEl: HTMLFormElement,
    private readonly handlers: ConfigFormHandlers,
  ) {
    this.playerRowsEl = formEl.querySelector("#player-rows")!;
    this.errorEl = formEl.querySelector("#config-error")!;
    this.startButton = formEl.querySelector<HTMLButtonElement>("#start-button")!;
    this.seedField = formEl.querySelector("#seed-field")!;
    this.gameCountField = formEl.querySelector("#game-count-field")!;

    formEl.querySelector("#add-player")!.addEventListener("click", () => this.addPlayerRow());
    formEl.querySelectorAll<HTMLInputElement>('input[name="run_mode"]').forEach((radio) => {
      radio.addEventListener("change", () => this.updateModeFields());
    });
    this.updateModeFields();
    formEl.addEventListener("submit", (event) => {
      event.preventDefault();
      this.submit();
    });
  }

  /** Disables the submit button while a batch request is in flight - a
   * batch is one synchronous request/response (docs/frontend.md), so this is
   * the whole of the "progress indicator" for it. */
  setBusy(busy: boolean): void {
    this.startButton.disabled = busy;
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

  private mode(): "live" | "batch" {
    const checked = this.formEl.querySelector<HTMLInputElement>('input[name="run_mode"]:checked');
    return checked?.value === "batch" ? "batch" : "live";
  }

  private updateModeFields(): void {
    const batch = this.mode() === "batch";
    this.seedField.hidden = batch;
    this.gameCountField.hidden = !batch;
    this.startButton.textContent = batch ? "Run batch" : "Start";
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

    if (this.mode() === "batch") {
      this.handlers.onRunBatch({ ruleSet: rules, players, gameCount: buildGameCount(data) });
      return;
    }

    const seedInput = data.get("seed");
    const seed = seedInput ? BigInt(seedInput as string) : BigInt(Math.floor(Math.random() * Number.MAX_SAFE_INTEGER));
    this.handlers.onStart({ config: { rules, players }, seed });
  }
}
