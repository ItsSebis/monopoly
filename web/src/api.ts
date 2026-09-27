// Thin fetch wrappers over `docs/api.md`'s endpoints (the 6 archive
// endpoints plus Phase 9's 5 interactive-session endpoints). The server is
// optional infrastructure (docs/frontend.md) - every function here can
// reject (network error, non-2xx), and callers decide how to degrade
// (e.g. the history panel falls back to the localStorage cache).
import { parsePreservingSeeds, stringifyPreservingSeeds } from "./bigJson";
import type {
  BatchRunRecord,
  BoardSpaceDto,
  CreateSessionRequest,
  DecisionAnswer,
  PlayerConfig,
  RunDetail,
  RunSummary,
  RuleSet,
  SessionSnapshot,
  SingleRunRecord,
} from "./types";

const SERVER_URL_KEY = "monopoly:serverUrl";
const DEFAULT_SERVER_URL = "http://localhost:3000";

export function getServerUrl(): string {
  try {
    return localStorage.getItem(SERVER_URL_KEY) || DEFAULT_SERVER_URL;
  } catch {
    return DEFAULT_SERVER_URL;
  }
}

export function setServerUrl(url: string): void {
  try {
    localStorage.setItem(SERVER_URL_KEY, url);
  } catch {
    // Per-viewer convenience only - a blocked/unavailable localStorage just
    // means the input reverts to the default next load, not a hard failure.
  }
}

/** Thrown by `request()` for any non-2xx response, carrying the HTTP status
 * alongside the server's own `{"error": "message"}` text - callers that need
 * to distinguish a 404 (e.g. `interactive/sessionController.ts` treating a
 * reaped/deleted session as terminal, not just another transient failure)
 * check `.status` instead of parsing `.message`. */
export class ApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${getServerUrl()}${path}`, {
    headers: init?.body ? { "Content-Type": "application/json" } : undefined,
    ...init,
  });
  const text = await response.text();
  if (!response.ok) {
    const body = text ? parsePreservingSeeds(text) : null;
    const message = (body as { error?: string } | null)?.error;
    throw new ApiError(message ?? `request to ${path} failed with ${response.status}`, response.status);
  }
  if (response.status === 204 || !text) return undefined as T;
  return parsePreservingSeeds(text) as T;
}

export function postRun(record: SingleRunRecord | BatchRunRecord): Promise<RunDetail> {
  return request("/runs", { method: "POST", body: stringifyPreservingSeeds(record) });
}

export function postRunsBatch(ruleSet: RuleSet, players: PlayerConfig[], gameCount: number): Promise<RunDetail> {
  return request("/runs/batch", {
    method: "POST",
    body: JSON.stringify({ rule_set: ruleSet, players, game_count: gameCount }),
  });
}

export function getRuns(params: { kind?: string; strategy?: string } = {}): Promise<RunSummary[]> {
  const query = new URLSearchParams();
  if (params.kind) query.set("kind", params.kind);
  if (params.strategy) query.set("strategy", params.strategy);
  const suffix = query.toString() ? `?${query}` : "";
  return request(`/runs${suffix}`);
}

export function getRun(id: string): Promise<RunDetail> {
  return request(`/runs/${encodeURIComponent(id)}`);
}

export function getRunGames(id: string, seed: bigint): Promise<SingleRunRecord> {
  return request(`/runs/${encodeURIComponent(id)}/games/${seed}`);
}

export function deleteRun(id: string): Promise<void> {
  return request(`/runs/${encodeURIComponent(id)}`, { method: "DELETE" });
}

// Interactive sessions (Phase 9/10, docs/api.md#interactive-sessions-phase-9).
// None of these shapes carry a `seed` field the way archive records do
// (a session is never replayed from one - see interactive/sessionController.ts),
// so the plain `request()` helper above (seed-preserving parse included) is
// safe to reuse unchanged.

export function getBoard(): Promise<BoardSpaceDto[]> {
  return request("/board");
}

export function createSession(body: CreateSessionRequest): Promise<SessionSnapshot> {
  return request("/sessions", { method: "POST", body: JSON.stringify(body) });
}

export function getSessionSnapshot(id: string, sinceSeq: number): Promise<SessionSnapshot> {
  return request(`/sessions/${encodeURIComponent(id)}?since_seq=${sinceSeq}`);
}

export function postDecision(id: string, sinceSeq: number, answer: DecisionAnswer): Promise<SessionSnapshot> {
  return request(`/sessions/${encodeURIComponent(id)}/decisions?since_seq=${sinceSeq}`, {
    method: "POST",
    body: JSON.stringify(answer),
  });
}

/** `keepalive` lets this survive a `beforeunload`/`pagehide`-triggered call
 * (`main.ts`) - a plain `fetch` there is routinely cancelled by the browser
 * once the page starts unloading, which would silently leave the session's
 * game thread running until the server's own 30-minute idle reaper. */
export function deleteSession(id: string): Promise<void> {
  return request(`/sessions/${encodeURIComponent(id)}`, { method: "DELETE", keepalive: true });
}
