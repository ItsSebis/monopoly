import { beforeEach, describe, expect, it } from "vitest";
import { listRecent, pushRecent, removeRecent } from "./recentRunsCache";
import type { RunSummary } from "../types";

function summary(id: string): RunSummary {
  return { id, kind: "single", created_at: "2026-01-01T00:00:00Z", rule_set: {} as never, players: [] };
}

beforeEach(() => {
  localStorage.clear();
});

describe("recentRunsCache", () => {
  it("lists pushed entries newest-first", () => {
    pushRecent(summary("a"));
    pushRecent(summary("b"));
    expect(listRecent().map((r) => r.id)).toEqual(["b", "a"]);
  });

  it("de-duplicates by id, moving the re-pushed entry to the front", () => {
    pushRecent(summary("a"));
    pushRecent(summary("b"));
    pushRecent(summary("a"));
    expect(listRecent().map((r) => r.id)).toEqual(["a", "b"]);
  });

  it("caps at 20 entries, dropping the oldest", () => {
    for (let i = 0; i < 25; i++) pushRecent(summary(`id${i}`));
    const ids = listRecent().map((r) => r.id);
    expect(ids).toHaveLength(20);
    expect(ids[0]).toBe("id24");
    expect(ids).not.toContain("id0");
  });

  it("removes an entry by id", () => {
    pushRecent(summary("a"));
    pushRecent(summary("b"));
    removeRecent("a");
    expect(listRecent().map((r) => r.id)).toEqual(["b"]);
  });
});
