// Shared by every `catch (err) { ... }` in this app that just needs a
// displayable string - `err` from a `fetch`/Worker/session round trip is
// typed `unknown`, but is virtually always an `Error` (or `ApiError`, a
// subclass) in practice; anything else falls back to `String(err)`.
export function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
