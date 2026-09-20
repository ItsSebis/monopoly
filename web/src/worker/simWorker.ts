// Runs the simulation off the main thread (docs/architecture.md: live
// playback happens via engine-wasm inside a Web Worker so board rendering
// never blocks on simulation). The worker races ahead independently of what
// the main thread has drained - see docs/frontend.md's playback section for
// why that's an explicitly allowed implementation choice.
import init, { WasmGame, build_single_run_record, strategy_ids } from "../wasm/monopoly_engine_wasm.js";
import type { EventEnvelope, GameConfig, GameState } from "../types";

export type WorkerRequest =
  | { type: "start"; config: GameConfig; seed: number }
  | { type: "buildRecord"; config: GameConfig; seed: number };

export type WorkerResponse =
  | { type: "ready"; strategyIds: string[] }
  | { type: "turn"; events: EventEnvelope[]; state: GameState }
  | { type: "done"; winner: number | null; turns: number }
  | { type: "record"; recordJson: string }
  | { type: "error"; message: string };

function post(message: WorkerResponse): void {
  // `self` inside a worker has no direct-call `postMessage(message)` overload
  // in the DOM lib alone (only the main-thread `Worker` handle does) - this
  // crate intentionally avoids adding the "webworker" lib project-wide (see
  // tsconfig.json) just for this one file, so a narrow cast here is simpler.
  (self as unknown as Worker).postMessage(message);
}

function runGame(config: GameConfig, seed: number): void {
  let game: WasmGame;
  try {
    game = new WasmGame(JSON.stringify(config), BigInt(seed));
  } catch (err) {
    post({ type: "error", message: err instanceof Error ? err.message : String(err) });
    return;
  }

  while (!game.is_over()) {
    const events = JSON.parse(game.step_turn()) as EventEnvelope[];
    const state = JSON.parse(game.state()) as GameState;
    post({ type: "turn", events, state });
  }

  // `Event::GameEnded` is only ever recorded by `Game::run_to_completion`
  // (see game.rs), never by `step_turn` - live playback drives `step_turn`
  // itself, so it never sees that event and has to work out the winner the
  // same way `run_to_completion` does: the one non-bankrupt player, if
  // exactly one remains.
  const finalState = JSON.parse(game.state()) as GameState;
  const survivors = finalState.players.filter((p) => !p.bankrupt);
  const winner = survivors.length === 1 ? finalState.players.indexOf(survivors[0]) : null;
  post({ type: "done", winner, turns: finalState.turn });
}

/** Deterministically re-simulates `(config, seed)` and returns the canonical
 * `SingleRunRecord` JSON - for "save this run" and "view stats", both of
 * which need `final_stats`, which live playback's `step_turn` loop never
 * computes (see docs/frontend.md). Re-running the game is effectively free
 * (well under a millisecond - see the Phase 6 plan's batch benchmark). */
function buildRecord(config: GameConfig, seed: number): void {
  try {
    const recordJson = build_single_run_record(JSON.stringify(config), BigInt(seed));
    post({ type: "record", recordJson });
  } catch (err) {
    post({ type: "error", message: err instanceof Error ? err.message : String(err) });
  }
}

async function main(): Promise<void> {
  await init();
  post({ type: "ready", strategyIds: JSON.parse(strategy_ids()) as string[] });

  self.addEventListener("message", (ev: MessageEvent) => {
    const request = ev.data as WorkerRequest;
    if (request.type === "start") runGame(request.config, request.seed);
    else if (request.type === "buildRecord") buildRecord(request.config, request.seed);
  });
}

main();
