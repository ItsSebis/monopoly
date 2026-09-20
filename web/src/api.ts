// Thin fetch wrappers over `docs/api.md`'s six endpoints. The server is
// optional infrastructure (docs/frontend.md) - every function here can
// reject (network error, non-2xx), and callers decide how to degrade
// (e.g. the history panel falls back to the localStorage cache).
import { parsePreservingSeeds, stringifyPreservingSeeds } from "./bigJson";
import type { BatchRunRecord, PlayerConfig, RunDetail, RunSummary, RuleSet, SingleRunRecord } from "./types";

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

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${getServerUrl()}${path}`, {
    headers: init?.body ? { "Content-Type": "application/json" } : undefined,
    ...init,
  });
  const text = await response.text();
  if (!response.ok) {
    const body = text ? parsePreservingSeeds(text) : null;
    const message = (body as { error?: string } | null)?.error;
    throw new Error(message ?? `request to ${path} failed with ${response.status}`);
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
