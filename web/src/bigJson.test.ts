import { describe, expect, it } from "vitest";
import { parsePreservingSeeds, stringifyPreservingSeeds } from "./bigJson";

// A u64 seed larger than Number.MAX_SAFE_INTEGER (2^53-1) - exactly the
// class of value plain JSON.parse/JSON.stringify would silently round.
const BIG_SEED = 14819371645677800001n;

describe("parsePreservingSeeds", () => {
  it("parses a scalar seed as a bigint, exactly", () => {
    const text = `{"rule_set":{},"seed":${BIG_SEED},"events":[]}`;
    const result = parsePreservingSeeds(text) as { seed: bigint };
    expect(result.seed).toBe(BIG_SEED);
  });

  it("parses a seeds array as bigints, exactly", () => {
    const text = `{"seeds":[${BIG_SEED},1,2]}`;
    const result = parsePreservingSeeds(text) as { seeds: bigint[] };
    expect(result.seeds).toEqual([BIG_SEED, 1n, 2n]);
  });

  it("parses a seed inside a nested per_game_summary array", () => {
    const text = `{"per_game_summary":[{"seed":${BIG_SEED},"winner":0,"turns":42}]}`;
    const result = parsePreservingSeeds(text) as { per_game_summary: { seed: bigint }[] };
    expect(result.per_game_summary[0].seed).toBe(BIG_SEED);
  });

  it("leaves ordinary large-looking text untouched when it isn't a seed field", () => {
    const text = `{"error":"received: 14819371645677800001 at position 3"}`;
    const result = parsePreservingSeeds(text) as { error: string };
    expect(result.error).toBe("received: 14819371645677800001 at position 3");
  });

  it("leaves small seeds as bigint too, for a consistent type", () => {
    const text = `{"seed":42,"events":[]}`;
    const result = parsePreservingSeeds(text) as { seed: bigint };
    expect(result.seed).toBe(42n);
  });
});

describe("stringifyPreservingSeeds", () => {
  it("round-trips a bigint seed through stringify + parse exactly", () => {
    const record = { seed: BIG_SEED, players: [] };
    const text = stringifyPreservingSeeds(record);
    expect(text).toContain(`"seed":${BIG_SEED}`);
    const parsed = parsePreservingSeeds(text) as { seed: bigint };
    expect(parsed.seed).toBe(BIG_SEED);
  });

  it("produces plain JSON (no bigint literal syntax) for a normal consumer", () => {
    const text = stringifyPreservingSeeds({ seed: 5n });
    expect(() => JSON.parse(text)).not.toThrow();
    expect(JSON.parse(text)).toEqual({ seed: 5 });
  });
});
