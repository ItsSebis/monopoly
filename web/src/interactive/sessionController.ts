// Drives a Phase 9 interactive session: `POST /sessions` to start, then
// polls `GET /sessions/{id}?since_seq=` on an interval (CPU turns resolve
// near-instantly server-side, so plain polling is the whole design - see
// docs/api.md#interactive-sessions-phase-9). Feeds every poll's events/state
// into the same `PlaybackController` live play already uses (docs/frontend.md's
// replay-pipeline-reuse decision extends naturally here: this is just a
// second *driver* of that same controller, not a second renderer).
import * as api from "../api";
import { PlaybackController } from "../controls/playback";
import type {
  BoardSpaceDto,
  DecisionAnswer,
  EventEnvelope,
  GameConfig,
  GameOver,
  GameState,
  PendingDecision,
} from "../types";

const POLL_INTERVAL_MS = 400;

export interface SessionCallbacks {
  onEvent: (env: EventEnvelope) => void;
  onTurnBoundary: (state: GameState) => void;
  /** Fired once, the moment a pending decision's own lead-up events have
   * finished animating (`PlaybackController.isDrained()`) - never while the
   * board is still catching up to what actually happened. */
  onPendingDecision: (pending: PendingDecision, state: GameState, boardData: BoardSpaceDto[]) => void;
  onGameOver: (over: GameOver) => void;
  onError: (message: string) => void;
}

export class SessionController {
  readonly playback: PlaybackController;
  private id: string | null = null;
  private humanSeat = 0;
  private sinceSeq = 0;
  private pollTimer: ReturnType<typeof setInterval> | null = null;
  private pendingDecision: PendingDecision | null = null;
  private promptShown = false;
  private latestState: GameState | null = null;
  private boardData: BoardSpaceDto[] = [];
  private stopped = false;

  constructor(private readonly callbacks: SessionCallbacks) {
    this.playback = new PlaybackController(callbacks.onEvent, callbacks.onTurnBoundary);
  }

  /** Fetches the static board once, creates the session, and starts polling.
   * Returns the session's seat assignment so the caller can label "you". */
  async start(config: GameConfig, humanSeat: number): Promise<{ id: string; humanSeat: number }> {
    this.boardData = await api.getBoard().catch(() => []);
    const snapshot = await api.createSession({
      rules: config.rules,
      players: config.players,
      human_seat: humanSeat,
    });
    this.id = snapshot.id;
    this.humanSeat = snapshot.human_seat;
    this.applySnapshot(snapshot);
    this.pollTimer = setInterval(() => void this.tick(), POLL_INTERVAL_MS);
    return { id: snapshot.id, humanSeat: snapshot.human_seat };
  }

  getBoardData(): BoardSpaceDto[] {
    return this.boardData;
  }

  getHumanSeat(): number {
    return this.humanSeat;
  }

  /** Posts an answer to whatever's currently pending. Safe to call only
   * while a decision is actually being shown - the server itself is the
   * backstop for a stale/duplicate answer (409 `KindMismatch`/`Finished`),
   * surfaced through `onError` like any other request failure. */
  answer(answer: DecisionAnswer): void {
    if (!this.id) return;
    this.pendingDecision = null;
    this.promptShown = false;
    const id = this.id;
    const sinceSeq = this.sinceSeq;
    api
      .postDecision(id, sinceSeq, answer)
      .then((snapshot) => this.applySnapshot(snapshot))
      .catch((err) => this.callbacks.onError(err instanceof Error ? err.message : String(err)));
  }

  /** Tears the session down (best-effort - a failed `DELETE` just means the
   * server's own 30-minute idle reaper cleans it up later). Idempotent. */
  stop(): void {
    if (this.stopped) return;
    this.stopped = true;
    if (this.pollTimer !== null) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
    this.playback.pause();
    if (this.id) {
      const id = this.id;
      this.id = null;
      void api.deleteSession(id).catch(() => {});
    }
  }

  private async tick(): Promise<void> {
    this.maybeShowPrompt();
    if (!this.id) return;
    try {
      const snapshot = await api.getSessionSnapshot(this.id, this.sinceSeq);
      this.applySnapshot(snapshot);
    } catch (err) {
      this.callbacks.onError(err instanceof Error ? err.message : String(err));
    }
  }

  private applySnapshot(snapshot: {
    events: EventEnvelope[];
    state: GameState;
    pending: PendingDecision | null;
    game_over: GameOver | null;
    errored: string | null;
  }): void {
    if (snapshot.errored) {
      this.stop();
      this.callbacks.onError(snapshot.errored);
      return;
    }
    this.latestState = snapshot.state;
    this.pendingDecision = snapshot.pending;
    const last = snapshot.events[snapshot.events.length - 1];
    if (last) this.sinceSeq = last.seq;
    // `enqueueTurn` handles an empty `events` array itself (applies `state`
    // immediately via `onTurnBoundary`) - every poll response becomes one
    // enqueued batch regardless of whether it's a full turn's worth of
    // events or a mid-turn slice ending in a pending decision.
    this.playback.enqueueTurn(snapshot.events, snapshot.state);
    if (snapshot.game_over) this.callbacks.onGameOver(snapshot.game_over);
    this.maybeShowPrompt();
  }

  private maybeShowPrompt(): void {
    if (!this.pendingDecision || !this.latestState || this.promptShown) return;
    if (!this.playback.isDrained()) return;
    this.promptShown = true;
    this.callbacks.onPendingDecision(this.pendingDecision, this.latestState, this.boardData);
  }
}
