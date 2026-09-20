import "./style.css";
import * as api from "./api";
import { BoardView } from "./board/board";
import { renderSingleRunCharts } from "./charts/singleRunCharts";
import { renderBatchResults } from "./controls/batchResults";
import { ConfigForm, type BatchPayload, type StartPayload } from "./controls/configForm";
import { PlaybackController } from "./controls/playback";
import { formatEvent, formatGameEnded } from "./eventLog";
import { HistoryPanel } from "./history/historyPanel";
import { pushRecent, summaryOf } from "./history/recentRunsCache";
import type { BatchRunRecord, EventEnvelope, GameConfig, GameState, PlayerConfig, RuleSet, SingleRunRecord } from "./types";
import type { WorkerResponse } from "./worker/simWorker";

/** All ids below are static markup in index.html, so the lookup is
 * infallible in practice - a single generic cast here beats repeating `!`
 * or `as HTMLXxxElement` at every call site. */
function el<T extends HTMLElement = HTMLElement>(id: string): T {
  return document.getElementById(id) as T;
}

const playView = el("play-view");
const historyPanelEl = el("history-panel");
const navPlay = el<HTMLButtonElement>("nav-play");
const navHistory = el<HTMLButtonElement>("nav-history");
const serverUrlInput = el<HTMLInputElement>("server-url-input");

const configPanel = el("config-panel");
const gamePanel = el("game-panel");
const batchResultsPanel = el("batch-results-panel");
const batchResultsEl = el("batch-results");
const batchNewButton = el<HTMLButtonElement>("batch-new-button");

const boardContainer = el("board");
const playerPanel = el("player-panel");
const eventLogEl = el("event-log");
const turnIndicator = el("turn-indicator");
const gameOverBanner = el("game-over-banner");
const gameOverActions = el("game-over-actions");
const viewStatsButton = el<HTMLButtonElement>("view-stats-button");
const saveRunButton = el<HTMLButtonElement>("save-run-button");
const saveRunNote = el("save-run-note");
const gameStatsEl = el("game-stats");
const playPauseButton = el<HTMLButtonElement>("play-pause-button");
const stepButton = el<HTMLButtonElement>("step-button");
const speedSelect = el<HTMLSelectElement>("speed-select");
const newGameButton = el<HTMLButtonElement>("new-game-button");

const board = new BoardView(boardContainer);
let worker: Worker;
let playback: PlaybackController | null = null;
let playerNames: string[] = [];
let pendingGameOver: { winner: number | null; turns: number } | null = null;
let currentConfig: GameConfig | null = null;
let currentSeed: number | null = null;
let pendingRecordRequest: { resolve: (r: SingleRunRecord) => void; reject: (e: Error) => void } | null = null;

function newWorker(): Worker {
  const w = new Worker(new URL("./worker/simWorker.ts", import.meta.url), { type: "module" });
  w.addEventListener("message", (event: MessageEvent<WorkerResponse>) => handleWorkerMessage(event.data));
  return w;
}

const configForm = new ConfigForm(el<HTMLFormElement>("config-form"), {
  onStart: (payload: StartPayload) =>
    startReplay(payload.config, payload.seed, payload.config.players.map((p) => p.name)),
  onRunBatch: (payload: BatchPayload) => void runBatch(payload),
});

const historyPanel = new HistoryPanel(historyPanelEl, {
  onReplay: (ruleSet, players, seed, names) => {
    showView("play");
    startReplay({ rules: ruleSet, players }, seed, names);
  },
});

worker = newWorker();

function handleWorkerMessage(message: WorkerResponse): void {
  switch (message.type) {
    case "ready":
      configForm.setStrategyIds(message.strategyIds);
      break;
    case "turn":
      playback?.enqueueTurn(message.events, message.state);
      break;
    case "done":
      pendingGameOver = { winner: message.winner, turns: message.turns };
      maybeShowGameOver();
      break;
    case "record":
      pendingRecordRequest?.resolve(JSON.parse(message.recordJson) as SingleRunRecord);
      pendingRecordRequest = null;
      break;
    case "error":
      if (pendingRecordRequest) {
        pendingRecordRequest.reject(new Error(message.message));
        pendingRecordRequest = null;
      } else {
        configForm.showError(message.message);
      }
      break;
  }
}

/** Asks the worker (which already has wasm loaded) to deterministically
 * re-simulate `(config, seed)` into a `SingleRunRecord` - used by both "View
 * stats" and "Save run" so neither needs to track the live game's own event
 * stream (docs/frontend.md).
 *
 * Only one `buildRecord` round trip is tracked at a time: if "View stats"
 * and "Save run" are both clicked before the first response lands, the
 * earlier request is rejected immediately (rather than silently orphaned)
 * so its caller's `finally` still runs and re-enables its button - without
 * this, whichever response arrived second would find `pendingRecordRequest`
 * already `null` and be dropped on the floor, leaving the first click's
 * promise (and its button) hung forever. See also `newGameButton`'s click
 * handler, which rejects any request still pending when the worker itself
 * is torn down. */
function requestRecord(config: GameConfig, seed: number): Promise<SingleRunRecord> {
  pendingRecordRequest?.reject(new Error("Superseded by a newer request."));
  return new Promise((resolve, reject) => {
    pendingRecordRequest = { resolve, reject };
    worker.postMessage({ type: "buildRecord", config, seed });
  });
}

/** The single entry point for "watch a game play out on the board" -
 * started fresh from the config form, or replayed from a batch drilldown or
 * a saved history run. Every caller already has `(config, seed)`; the game
 * itself is fully determined by it, so there is exactly one playback path
 * (docs/frontend.md's replay-pipeline-reuse decision). */
function startReplay(config: GameConfig, seed: number, names: string[]): void {
  playerNames = names;
  currentConfig = config;
  currentSeed = seed;
  pendingGameOver = null;

  configPanel.hidden = true;
  batchResultsPanel.hidden = true;
  gamePanel.hidden = false;
  gameOverBanner.hidden = true;
  gameOverActions.hidden = true;
  gameStatsEl.innerHTML = "";
  saveRunNote.textContent = "";
  eventLogEl.innerHTML = "";

  playback = new PlaybackController(onEvent, onTurnBoundary);
  playback.setSpeedMultiplier(Number(speedSelect.value));
  playPauseButton.textContent = "Pause";

  worker.postMessage({ type: "start", config, seed });
}

async function runBatch(payload: BatchPayload): Promise<void> {
  configForm.showError("");
  configForm.setBusy(true);
  try {
    const detail = await api.postRunsBatch(payload.ruleSet, payload.players, payload.gameCount);
    if (detail.kind !== "batch") throw new Error("server returned a non-batch record for a batch request");
    pushRecent(summaryOf(detail));
    showBatchResults(detail.rule_set, detail.players, detail);
  } catch (err) {
    configForm.showError(err instanceof Error ? err.message : String(err));
  } finally {
    configForm.setBusy(false);
  }
}

function showBatchResults(ruleSet: RuleSet, players: PlayerConfig[], record: BatchRunRecord): void {
  configPanel.hidden = true;
  gamePanel.hidden = true;
  batchResultsPanel.hidden = false;
  const names = players.map((p) => p.name);
  renderBatchResults(batchResultsEl, record, names, (seed) => startReplay({ rules: ruleSet, players }, seed, names));
}

function onEvent(env: EventEnvelope): void {
  board.applyEvent(env);
  const line = document.createElement("p");
  line.textContent = formatEvent(env, playerNames);
  eventLogEl.prepend(line);
}

function onTurnBoundary(state: GameState): void {
  board.renderState(state);
  renderPlayerPanel(state);
  turnIndicator.textContent = `Turn ${state.turn}`;
  maybeShowGameOver();
}

/** Shows the winner banner only once playback has actually caught up to the
 * worker's "done" message (docs/frontend.md's decoupled worker/main-thread
 * pacing means the worker can finish well before slower speeds finish
 * animating through it). */
function maybeShowGameOver(): void {
  if (!pendingGameOver || !playback?.isDrained()) return;
  gameOverBanner.hidden = false;
  gameOverBanner.textContent = formatGameEnded(pendingGameOver, playerNames);
  gameOverActions.hidden = false;
  pendingGameOver = null;
}

function renderPlayerPanel(state: GameState): void {
  playerPanel.innerHTML = "";
  state.players.forEach((player, index) => {
    const row = document.createElement("div");
    row.className = "player";
    if (index === state.current_player && !player.bankrupt) row.classList.add("current");
    if (player.bankrupt) row.classList.add("bankrupt");

    const swatch = document.createElement("span");
    swatch.className = "token";
    swatch.style.background = `var(--p${index})`;

    const label = document.createElement("span");
    label.textContent = `${player.name}: $${player.cash}${player.in_jail ? " (in jail)" : ""}`;

    row.append(swatch, label);
    playerPanel.appendChild(row);
  });
}

function showView(view: "play" | "history"): void {
  playView.hidden = view !== "play";
  historyPanelEl.hidden = view !== "history";
  navPlay.setAttribute("aria-current", String(view === "play"));
  navHistory.setAttribute("aria-current", String(view === "history"));
  if (view === "history") void historyPanel.refresh();
}

navPlay.addEventListener("click", () => showView("play"));
navHistory.addEventListener("click", () => showView("history"));

serverUrlInput.value = api.getServerUrl();
serverUrlInput.addEventListener("change", () => api.setServerUrl(serverUrlInput.value));

playPauseButton.addEventListener("click", () => {
  if (!playback) return;
  if (playback.isPlaying()) {
    playback.pause();
    playPauseButton.textContent = "Play";
  } else {
    playback.play();
    playPauseButton.textContent = "Pause";
  }
});

stepButton.addEventListener("click", () => playback?.stepOnce());

speedSelect.addEventListener("change", () => {
  playback?.setSpeedMultiplier(Number(speedSelect.value));
});

newGameButton.addEventListener("click", () => {
  // A terminated worker never posts back the "record"/"error" message a
  // pending `requestRecord()` is waiting on, so without this its promise
  // (and whichever of View stats/Save run triggered it) would hang forever.
  pendingRecordRequest?.reject(new Error("Game was reset before this finished."));
  pendingRecordRequest = null;
  worker.terminate();
  playback?.pause();
  playback = null;
  currentConfig = null;
  currentSeed = null;
  configForm.awaitReady();
  worker = newWorker();
  configPanel.hidden = false;
  gamePanel.hidden = true;
});

batchNewButton.addEventListener("click", () => {
  batchResultsPanel.hidden = true;
  configPanel.hidden = false;
});

viewStatsButton.addEventListener("click", async () => {
  if (!currentConfig || currentSeed === null) return;
  viewStatsButton.disabled = true;
  try {
    const record = await requestRecord(currentConfig, currentSeed);
    renderSingleRunCharts(gameStatsEl, record.final_stats, playerNames);
  } catch (err) {
    saveRunNote.textContent = err instanceof Error ? err.message : "Failed to compute stats.";
  } finally {
    viewStatsButton.disabled = false;
  }
});

saveRunButton.addEventListener("click", async () => {
  if (!currentConfig || currentSeed === null) return;
  saveRunButton.disabled = true;
  saveRunNote.textContent = "Saving…";
  try {
    const record = await requestRecord(currentConfig, currentSeed);
    const detail = await api.postRun(record);
    pushRecent(summaryOf(detail));
    saveRunNote.textContent = `Saved as ${detail.id}.`;
  } catch (err) {
    saveRunNote.textContent = err instanceof Error ? err.message : "Failed to save run.";
  } finally {
    saveRunButton.disabled = false;
  }
});
