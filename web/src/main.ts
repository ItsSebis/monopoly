import "./style.css";
import { BoardView } from "./board/board";
import { ConfigForm, type StartPayload } from "./controls/configForm";
import { PlaybackController } from "./controls/playback";
import { formatEvent } from "./eventLog";
import type { EventEnvelope, GameState } from "./types";
import type { WorkerResponse } from "./worker/simWorker";

const configPanel = document.getElementById("config-panel")!;
const gamePanel = document.getElementById("game-panel")!;
const boardContainer = document.getElementById("board")!;
const playerPanel = document.getElementById("player-panel")!;
const eventLogEl = document.getElementById("event-log")!;
const turnIndicator = document.getElementById("turn-indicator")!;
const gameOverBanner = document.getElementById("game-over-banner")!;
const playPauseButton = document.getElementById("play-pause-button") as HTMLButtonElement;
const stepButton = document.getElementById("step-button") as HTMLButtonElement;
const speedSelect = document.getElementById("speed-select") as HTMLSelectElement;
const newGameButton = document.getElementById("new-game-button") as HTMLButtonElement;

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

const configForm = new ConfigForm(document.getElementById("config-form") as HTMLFormElement, startGame);

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
  gameOverBanner.textContent = formatEvent(
    {
      turn: 0,
      player: 0,
      seq: 0,
      event: {
        type: "GameEnded",
        payload: { winner: pendingGameOver.winner, turns: pendingGameOver.turns },
      },
    },
    playerNames,
  );
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
