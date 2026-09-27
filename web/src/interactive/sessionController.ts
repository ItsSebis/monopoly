// Drives a Phase 9 interactive session: `POST /sessions` to start, then
// polls `GET /sessions/{id}?since_seq=` on an interval (CPU turns resolve
// near-instantly server-side, so plain polling is the whole design - see
// docs/api.md#interactive-sessions-phase-9). Feeds every poll's events/state
// into the same `PlaybackController` live play already uses (docs/frontend.md's
// replay-pipeline-reuse decision extends naturally here: this is just a
// second *driver* of that same controller, not a second renderer).
import * as api from "../api";
import { ApiError } from "../api";
import { PlaybackController } from "../controls/playback";
import { errorMessage } from "../errorMessage";
import type { DecisionAnswer, EventEnvelope, GameConfig, GameOver, GameState, PendingDecision } from "../types";

const POLL_INTERVAL_MS = 400;

export interface SessionCallbacks {
  onEvent: (env: EventEnvelope) => void;
  onTurnBoundary: (state: GameState) => void;
  /** Fired once, the moment a pending decision's own lead-up events have
   * finished animating (`PlaybackController.isDrained()`) - never while the
   * board is still catching up to what actually happened. */
  onPendingDecision: (pending: PendingDecision, state: GameState) => void;
  /** Fired whenever a previously-shown decision stops being the accurate one
   * to answer - it was answered elsewhere, superseded, or the session ended
   * - so the caller can hide a prompt that's no longer valid to submit. */
  onHidePrompt: () => void;
  onGameOver: (over: GameOver) => void;
  /** A session-ending failure (404 - reaped/deleted, or the session thread
   * itself panicked) as well as a transient one (network error, a rejected
   * decision) - both surfaced identically; only the former also stops
   * polling (see `handleRequestFailure`). */
  onError: (message: string) => void;
}

interface SessionSnapshotLike {
  events: EventEnvelope[];
  state: GameState;
  pending: PendingDecision | null;
  seq: number;
  game_over: GameOver | null;
  errored: string | null;
}

function decisionKey(pending: PendingDecision | null): string | null {
  return pending ? JSON.stringify(pending) : null;
}

export class SessionController {
  readonly playback: PlaybackController;
  private id: string | null = null;
  private sinceSeq = 0;
  private pollTimer: ReturnType<typeof setInterval> | null = null;
  private pendingDecision: PendingDecision | null = null;
  /** The `PendingDecision` (as a structural-equality key) actually shown to
   * the user right now, `null` if nothing is. Distinct from `pendingDecision`
   * itself so a snapshot that reports a *different* pending decision than
   * what's on screen (or none at all) can tell `applySnapshot` to hide the
   * stale one, rather than leaving it up and clickable with nothing behind
   * it - see the fix for the promptShown-gets-stuck race in review. */
  private shownKey: string | null = null;
  private latestState: GameState | null = null;
  private stopped = false;
  /** Bumped by every request this controller issues (`poll()`/`answer()`).
   * A response is only applied if it's still the most recent one issued -
   * this is what keeps a slow GET that started before an `answer()` call
   * from re-applying a decision the user has already answered once it
   * finally resolves (requests aren't otherwise cancellable). */
  private requestId = 0;
  private inFlight = 0;

  constructor(private readonly callbacks: SessionCallbacks) {
    this.playback = new PlaybackController(callbacks.onEvent, callbacks.onTurnBoundary);
  }

  /** Creates the session and starts polling. */
  async start(config: GameConfig, humanSeat: number): Promise<void> {
    const snapshot = await api.createSession({
      rules: config.rules,
      players: config.players,
      human_seat: humanSeat,
    });
    this.id = snapshot.id;
    this.applySnapshot(snapshot);
    this.pollTimer = setInterval(() => void this.tick(), POLL_INTERVAL_MS);
  }

  /** Posts an answer to whatever's currently pending. Safe to call only
   * while a decision is actually being shown - the server itself is the
   * backstop for a stale/duplicate answer (409 `KindMismatch`/`Finished`),
   * surfaced through `onError` like any other request failure. */
  answer(answer: DecisionAnswer): void {
    if (!this.id || this.stopped) return;
    this.pendingDecision = null;
    this.shownKey = null;
    const id = this.id;
    const sinceSeq = this.sinceSeq;
    const requestId = ++this.requestId;
    this.inFlight++;
    api
      .postDecision(id, sinceSeq, answer)
      .then((snapshot) => {
        if (requestId !== this.requestId) return;
        this.applySnapshot(snapshot);
      })
      .catch((err) => {
        if (requestId === this.requestId) this.handleRequestFailure(err);
      })
      .finally(() => {
        this.inFlight--;
      });
  }

  /** Tears the session down (best-effort - a failed `DELETE` just means the
   * server's own 30-minute idle reaper cleans it up later). Idempotent.
   *
   * Also used for a session-ending failure, not just user-initiated
   * teardown (`handleRequestFailure`'s 404 case, `applySnapshot`'s
   * `errored` case) - either way, a decision already on screen is no
   * longer answerable (there's nothing left to answer it), so it's hidden
   * here too rather than left up and clickable with nothing behind it. */
  stop(): void {
    if (this.stopped) return;
    this.stopped = true;
    this.requestId++; // discards any still-outstanding GET/POST's response
    this.clearPolling();
    this.playback.pause();
    if (this.shownKey !== null) {
      this.shownKey = null;
      this.callbacks.onHidePrompt();
    }
    if (this.id) {
      const id = this.id;
      this.id = null;
      void api.deleteSession(id).catch(() => {});
    }
  }

  private clearPolling(): void {
    if (this.pollTimer !== null) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
  }

  private async tick(): Promise<void> {
    this.maybeShowPrompt();
    // Serializes requests: skip this tick entirely while a GET or POST from
    // an earlier tick/answer() is still outstanding, so at most one request
    // is ever in flight - this is what actually prevents the GET/POST
    // interleaving race in review (a generation check alone still lets two
    // requests race on the wire; not overlapping them at all is simpler and
    // sufficient at this polling rate).
    if (!this.id || this.stopped || this.inFlight > 0) return;
    const id = this.id;
    const sinceSeq = this.sinceSeq;
    const requestId = ++this.requestId;
    this.inFlight++;
    try {
      const snapshot = await api.getSessionSnapshot(id, sinceSeq);
      if (requestId !== this.requestId) return;
      this.applySnapshot(snapshot);
    } catch (err) {
      if (requestId === this.requestId) this.handleRequestFailure(err);
    } finally {
      this.inFlight--;
    }
  }

  private handleRequestFailure(err: unknown): void {
    this.callbacks.onError(errorMessage(err));
    if (err instanceof ApiError && err.status === 404) {
      // The session is already gone server-side (reaped after 30 minutes
      // idle, or deleted out from under this poll) - nothing will ever
      // answer another request for it, so this is terminal, not transient.
      this.stop();
    }
  }

  private applySnapshot(snapshot: SessionSnapshotLike): void {
    if (this.stopped) return;

    if (snapshot.errored) {
      // `stop()` itself hides a currently-shown prompt (see its own doc
      // comment) - no separate `hideStalePrompt()` call needed here.
      this.stop();
      this.callbacks.onError(snapshot.errored);
      return;
    }

    this.latestState = snapshot.state;
    this.hideStalePrompt(snapshot.pending);
    this.pendingDecision = snapshot.pending;

    // Filtered/monotonic defensively, not just trusting the server sent
    // exactly `seq > sinceSeq` - a response can arrive describing less
    // progress than one already applied (its own request went out before a
    // later one that's already resolved), and `snapshot.seq` (the session's
    // own watermark) is the authoritative "how far this snapshot itself
    // goes", not just the last event happening to be in `events`.
    const newEvents = snapshot.events.filter((e) => e.seq > this.sinceSeq);
    this.sinceSeq = Math.max(this.sinceSeq, snapshot.seq);
    this.playback.enqueueTurn(newEvents, snapshot.state);

    if (snapshot.game_over) {
      // Stops polling but - unlike `stop()` - doesn't pause playback or
      // delete the session: playback should keep draining to the end
      // exactly as it would mid-game, and there's no decision left the
      // server could ever ask for, so leaving the (already-finished)
      // session for the idle reaper to eventually collect is fine.
      this.clearPolling();
      this.callbacks.onGameOver(snapshot.game_over);
    }
    this.maybeShowPrompt();
  }

  /** Hides whatever's currently shown if it no longer matches `pending` (it
   * was answered, superseded, or cleared) - `maybeShowPrompt` only ever
   * *shows*, so without this a stale prompt from an outdated snapshot would
   * stay on screen (and clickable) even once it stops being accurate. */
  private hideStalePrompt(pending: PendingDecision | null): void {
    const key = decisionKey(pending);
    if (this.shownKey !== null && this.shownKey !== key) {
      this.shownKey = null;
      this.callbacks.onHidePrompt();
    }
  }

  private maybeShowPrompt(): void {
    if (this.stopped) return;
    if (!this.pendingDecision || !this.latestState) return;
    const key = decisionKey(this.pendingDecision);
    if (key === this.shownKey) return;
    if (!this.playback.isDrained()) return;
    this.shownKey = key;
    this.callbacks.onPendingDecision(this.pendingDecision, this.latestState);
  }
}
