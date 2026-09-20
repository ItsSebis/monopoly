// `SingleRunRecord.seed`/`BatchRunRecord.seeds`/`BatchGameSummary.seed`
// (docs/data-model.md#run-record) are u64s that routinely exceed
// `Number.MAX_SAFE_INTEGER` (2^53-1) - virtually every server-generated seed
// does, since seeds are drawn from the full 64-bit range. Plain
// `JSON.parse`/`response.json()` silently rounds such a value to the
// nearest representable double, which would make "Replay" or a re-"Save"
// silently act on a *different* seed than the one actually archived - a
// wrong-game bug, not just a display glitch. These wrap/unwrap exactly the
// `"seed":`/`"seeds":[...]` shapes (never a generic "any large number"
// scan, which could misfire on a large number embedded in an unrelated
// string field, e.g. an error message) so they round-trip as `bigint`.
// A Private Use Area character, not a control character like `\u0000`:
// `JSON.stringify` only escapes `"`, `\`, and control characters (U+0000-
// U+001F), so this survives round-tripping through it unescaped, unlike an
// earlier version of this sentinel that used `\u0000` and silently never
// matched on the way back out (JSON.stringify re-encodes it as the 6-byte
// text `\u0000`, not a raw NUL).
const SENTINEL = "bigint:";
const SEED_SCALAR = /"seed":\s*(-?\d+)/g;
const SEEDS_ARRAY = /"seeds":\s*\[([-\d,\s]*)\]/g;

export function parsePreservingSeeds(text: string): unknown {
  const marked = text
    .replace(SEED_SCALAR, (_m, digits: string) => `"seed":"${SENTINEL}${digits}"`)
    .replace(SEEDS_ARRAY, (_m, inner: string) => {
      const wrapped = inner
        .split(",")
        .map((n) => n.trim())
        .filter((n) => n.length > 0)
        .map((n) => `"${SENTINEL}${n}"`)
        .join(",");
      return `"seeds":[${wrapped}]`;
    });
  return JSON.parse(marked, (_key, value) =>
    typeof value === "string" && value.startsWith(SENTINEL) ? BigInt(value.slice(SENTINEL.length)) : value,
  );
}

/** Inverse of `parsePreservingSeeds` - `JSON.stringify` throws on a `bigint`
 * value, so every `bigint` is marked during stringification and the marker
 * is then unquoted back into a bare numeral in the resulting text. Safe to
 * apply unconditionally (not just to `seed`/`seeds`): nothing in this app's
 * request bodies is ever a `bigint` except a seed. */
export function stringifyPreservingSeeds(value: unknown): string {
  const json = JSON.stringify(value, (_key, v) => (typeof v === "bigint" ? `${SENTINEL}${v}` : v));
  return json.replace(new RegExp(`"${SENTINEL}(-?\\d+)"`, "g"), "$1");
}
