// The config screen is a direct editor for RuleSet + PlayerConfig
// (docs/frontend.md) - every field has a form control, and submitting
// produces exactly the JSON GameConfig shape the engine expects, no
// UI-specific intermediate format.
import type { GameConfig, IncomeTaxMode, PlayerConfig, RuleSet } from "../types";

/** Mirrors `engine::strategies::STRATEGY_IDS` (`crates/engine/src/strategies/mod.rs`,
 * which carries a comment pointing back here) so the form doesn't have to
 * wait on a Worker's wasm init just to learn a static list - see
 * `setStrategyIds()`'s own comment for why this is the form's *first* answer
 * for every mode now, not just interactive's. Kept as a literal duplicate
 * rather than a build-time codegen step, matching this repo's existing
 * small-surface bias; drift from the Rust list is caught by
 * `configForm.test.ts`'s `STATIC_STRATEGY_IDS` test (which reads and
 * regex-parses `strategies/mod.rs` directly) and, as a runtime backstop,
 * `setStrategyIds()`'s own mismatch warning below. */
export const STATIC_STRATEGY_IDS = ["buy_all", "buy_good", "buy_bad", "buy_none", "buy_shrewd", "buy_optimal"];

export type RunMode = "live" | "batch" | "interactive";

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

/** The human seat's own `strategy` is a placeholder the server ignores
 * (docs/api.md#interactive-sessions-phase-9) - set to `"human"` below purely
 * for a clearer wire payload, not because anything reads it back. */
export interface StartInteractivePayload {
  config: GameConfig;
  humanSeat: number;
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
    double_go_salary: data.get("double_go_salary") === "on",
    unlimited_houses: data.get("unlimited_houses") === "on",
    trading_enabled: data.get("trading_enabled") === "on",
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
  onStartInteractive: (payload: StartInteractivePayload) => void;
}

export class ConfigForm {
  private strategyIds: string[] = STATIC_STRATEGY_IDS;
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
    formEl.addEventListener("submit", (event) => {
      event.preventDefault();
      this.submit();
    });

    // Every registered strategy id is known statically (`STATIC_STRATEGY_IDS`
    // above) - the form doesn't actually need a Worker's wasm init to be
    // usable, only "live" mode's Start does (it drives that same Worker).
    // Seeding 2 default rows and enabling controls immediately means every
    // mode, live included, is interactable right away; `setStrategyIds()`
    // (below) just reconciles the list once the Worker's `ready` message
    // actually arrives, which no longer gates anything.
    this.addPlayerRow();
    this.addPlayerRow();
    this.updateModeFields();
  }

  /** Disables the submit button while a batch request is in flight - a
   * batch is one synchronous request/response (docs/frontend.md), so this is
   * the whole of the "progress indicator" for it. */
  setBusy(busy: boolean): void {
    this.startButton.disabled = busy;
  }

  /** Called once the Worker reports the engine's registered strategy ids -
   * reconciles `STATIC_STRATEGY_IDS`'s copy with the real list for any
   * *future* player row (existing rows/selections are left alone). Since the
   * two are meant to be kept identical, this is normally a no-op in
   * practice; it's not load-bearing for the form's usability the way it was
   * before (see the constructor). A mismatch here means the Rust and TS
   * lists have drifted apart *and* `configForm.test.ts`'s own drift test
   * somehow didn't catch it (e.g. it wasn't run) - surfaced loudly enough to
   * notice without failing a real user's session over it. */
  setStrategyIds(ids: string[]): void {
    if (ids.length !== this.strategyIds.length || ids.some((id, i) => id !== this.strategyIds[i])) {
      console.warn(
        "STATIC_STRATEGY_IDS (configForm.ts) has drifted from the Worker's real strategy ids.",
        { static: this.strategyIds, real: ids },
      );
    }
    this.strategyIds = ids;
  }

  showError(message: string): void {
    this.errorEl.textContent = message;
  }

  /** Resets the player-rows editor to a fresh default pair - called when
   * "New game" tears down and restarts the live Worker, so the next game
   * doesn't inherit the just-finished one's roster. */
  resetPlayerRows(): void {
    this.playerRowsEl.innerHTML = "";
    this.rowCount = 0;
    this.addPlayerRow();
    this.addPlayerRow();
  }

  private mode(): RunMode {
    const checked = this.formEl.querySelector<HTMLInputElement>('input[name="run_mode"]:checked');
    if (checked?.value === "batch") return "batch";
    if (checked?.value === "interactive") return "interactive";
    return "live";
  }

  private updateModeFields(): void {
    const mode = this.mode();
    const startLabel: Record<RunMode, string> = { live: "Start", batch: "Run batch", interactive: "Play" };
    this.formEl.dataset.mode = mode;
    this.seedField.hidden = mode !== "live";
    this.gameCountField.hidden = mode !== "batch";
    this.startButton.textContent = startLabel[mode];
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

    // Only relevant/visible in interactive mode (`.human-seat-field`'s CSS,
    // toggled by `#config-form[data-mode]`) - picks which row's seat the
    // human client plays, the rest are CPU-controlled via their own
    // `strategySelect` above.
    const humanField = document.createElement("label");
    humanField.className = "human-seat-field";
    const humanRadio = document.createElement("input");
    humanRadio.type = "radio";
    humanRadio.name = "human_seat";
    humanRadio.checked = index === 0;
    humanField.append(humanRadio, " You");

    const removeButton = document.createElement("button");
    removeButton.type = "button";
    removeButton.textContent = "Remove";
    removeButton.addEventListener("click", () => {
      const wasHuman = humanRadio.checked;
      row.remove();
      if (wasHuman) {
        this.playerRowsEl.querySelector<HTMLInputElement>('input[name="human_seat"]')?.click();
      }
    });

    row.append(nameInput, strategySelect, humanField, removeButton);
    this.playerRowsEl.appendChild(row);
  }

  private submit(): void {
    this.errorEl.textContent = "";
    const rowEls = Array.from(this.playerRowsEl.querySelectorAll<HTMLElement>(".player-row"));
    const rows = rowEls.map((row) => ({
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
    const mode = this.mode();

    if (mode === "batch") {
      this.handlers.onRunBatch({ ruleSet: rules, players, gameCount: buildGameCount(data) });
      return;
    }

    if (mode === "interactive") {
      const humanRow = rowEls.findIndex((row) => row.querySelector<HTMLInputElement>('input[name="human_seat"]')?.checked);
      const humanSeat = humanRow === -1 ? 0 : humanRow;
      players[humanSeat] = { ...players[humanSeat], strategy: "human" };
      this.handlers.onStartInteractive({ config: { rules, players }, humanSeat });
      return;
    }

    const seedInput = data.get("seed");
    const seed = seedInput ? BigInt(seedInput as string) : BigInt(Math.floor(Math.random() * Number.MAX_SAFE_INTEGER));
    this.handlers.onStart({ config: { rules, players }, seed });
  }
}
