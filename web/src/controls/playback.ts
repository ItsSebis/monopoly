// Owns the event buffer and its drain timer (docs/frontend.md): the worker
// races ahead independently, posting each turn's events as it produces them;
// this is what actually paces the animation, draining one buffered event at
// a time at a speed-controlled rate. Pause only stops this drain - it never
// tells the worker to stop simulating.
import type { EventEnvelope, GameState } from "../types";

const BASE_INTERVAL_MS = 500; // interval per event at 1x speed

interface PendingTurnState {
  afterSeq: number;
  state: GameState;
}

export class PlaybackController {
  private readonly queue: EventEnvelope[] = [];
  private readonly pendingStates: PendingTurnState[] = [];
  private timer: ReturnType<typeof setInterval> | null = null;
  private intervalMs = BASE_INTERVAL_MS / 4; // matches the config panel's default (4x)
  private playing = true;

  constructor(
    private readonly onEvent: (env: EventEnvelope) => void,
    private readonly onTurnBoundary: (state: GameState) => void,
  ) {}

  /** One `step_turn()` call's worth of events, plus the resulting state to
   * apply once every one of those events has been drained. */
  enqueueTurn(events: EventEnvelope[], state: GameState): void {
    this.queue.push(...events);
    const last = events[events.length - 1];
    if (last) {
      this.pendingStates.push({ afterSeq: last.seq, state });
    } else {
      this.onTurnBoundary(state);
    }
    this.ensureRunning();
  }

  setSpeedMultiplier(multiplier: number): void {
    this.intervalMs = multiplier === 0 ? 0 : BASE_INTERVAL_MS / multiplier;
    this.clearTimer();
    this.ensureRunning();
  }

  play(): void {
    this.playing = true;
    this.ensureRunning();
  }

  pause(): void {
    this.playing = false;
    this.clearTimer();
  }

  isPlaying(): boolean {
    return this.playing;
  }

  /** Whether every buffered event (and its turn boundary) has been drained -
   * used to defer showing a "game over" banner until playback has visually
   * caught up, rather than the instant the worker itself finishes (which can
   * be well ahead of what's on screen at slower speeds). */
  isDrained(): boolean {
    return this.queue.length === 0 && this.pendingStates.length === 0;
  }

  /** Drains exactly one buffered event, regardless of play/pause state. */
  stepOnce(): void {
    this.drainOne();
  }

  private ensureRunning(): void {
    if (!this.playing || this.timer !== null) return;
    if (this.intervalMs === 0) {
      while (this.queue.length > 0) this.drainOne();
      return;
    }
    this.timer = setInterval(() => {
      if (this.queue.length === 0) {
        this.clearTimer();
        return;
      }
      this.drainOne();
    }, this.intervalMs);
  }

  private clearTimer(): void {
    if (this.timer !== null) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  private drainOne(): void {
    const env = this.queue.shift();
    if (!env) return;
    this.onEvent(env);
    if (this.pendingStates[0]?.afterSeq === env.seq) {
      this.onTurnBoundary(this.pendingStates.shift()!.state);
    }
  }
}
