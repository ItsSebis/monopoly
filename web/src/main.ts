import "./style.css";
import { BoardView } from "./board/board";
import { ConfigForm, type StartPayload } from "./controls/configForm";
import { PlaybackController } from "./controls/playback";
import { formatEvent, formatGameEnded } from "./eventLog";
import type { EventEnvelope, GameState } from "./types";
import type { WorkerResponse } from "./worker/simWorker";

/** All ids below are static markup in index.html, so the lookup is
 * infallible in practice - a single generic cast here beats repeating `!`
 * or `as HTMLXxxElement` at every call site. */
function el<T extends HTMLElement = HTMLElement>(id: string): T {
  return document.getElementById(id) as T;
}

const configPanel = el("config-panel");
const gamePanel = el("game-panel");
const boardContainer = el("board");
const playerPanel = el("player-panel");
const eventLogEl = el("event-log");
const turnIndicator = el("turn-indicator");
const gameOverBanner = el("game-over-banner");
const playPauseButton = el<HTMLButtonElement>("play-pause-button");
const stepButton = el<HTMLButtonElement>("step-button");
const speedSelect = el<HTMLSelectElement>("speed-select");
const newGameButton = el<HTMLButtonElement>("new-game-button");

const board = new BoardView(boardContainer);
let worker: Worker;
let playback: PlaybackController | null = null;
let playerNames: string[] = [];
let pendingGameOver: { winner: number | null; turns: number } | null = null;

function newWorker(): Worker {
  const w = new Worker(new URL("./worker/simWorker.ts", import.meta.url), { type: "module" });
  w.addEventListener("message", (event: MessageEvent<WorkerResponse>) => handleWorkerMessage(event.data));
  return w;
}

const configForm = new ConfigForm(el<HTMLFormElement>("config-form"), startGame);

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
    case "error":
      configForm.showError(message.message);
      break;
  }
}

function startGame(payload: StartPayload): void {
  playerNames = payload.config.players.map((p) => p.name);
  pendingGameOver = null;
  configPanel.hidden = true;
  gamePanel.hidden = false;
  gameOverBanner.hidden = true;
  eventLogEl.innerHTML = "";

  playback = new PlaybackController(onEvent, onTurnBoundary);
  playback.setSpeedMultiplier(Number(speedSelect.value));
  playPauseButton.textContent = "Pause";

  worker.postMessage({ type: "start", config: payload.config, seed: payload.seed });
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
  worker.terminate();
  playback?.pause();
  playback = null;
  configForm.awaitReady();
  worker = newWorker();
  configPanel.hidden = false;
  gamePanel.hidden = true;
});
