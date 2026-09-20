// A localStorage cache of recently-seen run summaries, per docs/frontend.md's
// "History browser": a convenience for instant display and a fallback when
// the server can't be reached, never a source of truth (the server always
// wins when reachable).
import type { RunSummary } from "../types";

const CACHE_KEY = "monopoly:recentRuns";
const MAX_ENTRIES = 20;

export function pushRecent(entry: RunSummary): void {
  try {
    const existing = listRecent().filter((r) => r.id !== entry.id);
    const updated = [entry, ...existing].slice(0, MAX_ENTRIES);
    localStorage.setItem(CACHE_KEY, JSON.stringify(updated));
  } catch {
    // Best-effort convenience cache - a write failure just means this run
    // won't show up in the offline fallback later.
  }
}

export function listRecent(): RunSummary[] {
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

export function removeRecent(id: string): void {
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(listRecent().filter((r) => r.id !== id)));
  } catch {
    // See pushRecent.
  }
}
