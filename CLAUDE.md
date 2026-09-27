# Development workflow

This file documents the standing workflow used for substantial work in this
repository — each roadmap phase, and any other change large enough to
warrant real isolation and review. It's written to be portable: the
numbered workflow below has no Monopoly-specific assumptions in it, and the
"This repo's specifics" section at the end is the only part that needs
adapting when reusing this file in another project.

## The workflow

Follow this loop for any non-trivial feature or phase of work, rather than
implementing ad-hoc directly on the main branch:

1. **Plan first.** Enter plan mode (or otherwise produce a written plan)
   before writing code. Ground the plan in things actually read from the
   codebase — file paths, existing patterns, real constraints — not
   assumptions about what's probably there. If the scope or interpretation
   of the request is genuinely ambiguous in a way that changes the size or
   shape of the work, ask; otherwise research the answer instead of asking.

2. **Self-review the plan** with as much objective/automated checking as
   feasible (does it compile in principle, does it contradict an existing
   design decision, does it duplicate something that already exists) before
   presenting it for approval. Minimize back-and-forth — don't just ask "is
   this ok?" without having already checked what can be checked.

3. **Isolate the implementation** in a dedicated git worktree and branch, so
   the main branch stays untouched and reviewable until the work is ready to
   merge.

4. **Use a review hierarchy, not self-certification.** After implementing,
   run at least two independent review passes over the diff:
   - **Correctness and performance first** — an adversarial review looking
     for real bugs (logic errors, edge cases, unsafe assumptions, race
     conditions), not style. Reproduce suspected bugs before fixing them,
     and re-measure after fixing to confirm the fix actually worked.
   - **Cleanliness and simplification second** — duplicated logic that
     should be shared (or over-abstracted logic that should be inlined),
     dead code, stale or self-contradictory comments, naming and style
     consistency with the surrounding codebase. Correctness takes
     precedence when the two trade off.
   Prefer independent reviewers (a fresh subagent with no memory of *why*
   the code was written, not the same context that wrote it) — code that
   looks obviously correct to its own author is exactly the code most
   likely to hide a bug from that same author.

5. **Run full automated verification** after every round of fixes: the
   complete test suite, linter, and formatter for every language/toolchain
   touched — not just the tests for the specific thing that changed.
   Nothing gets called "done" on the basis of a partial test run.

6. **Manually verify the feature actually works**, end to end, not just
   that its tests pass: drive the real UI in a browser for user-facing
   changes, or run the real CLI/binary for headless changes. Automated
   tests check that code does what the tests assume it should; manual
   verification checks that the tests assumed the right thing.

7. **Merge back to the mainline branch**, then verify the merge landed
   cleanly (status is clean, log looks right, verification still passes on
   the mainline branch after merging — a clean merge can still combine two
   individually-fine pieces of work into something broken).

8. **Commit and push.**

## When to skip this

Ad-hoc, low-risk requests that aren't themselves a planned feature or phase
— a docs update, a small config tweak, a one-line bug fix reported by the
user, research folded into an existing doc — can be done directly on the
mainline branch without the worktree or dual-review ceremony, even when
they're bundled into the same message as a request that *does* need the
full workflow. The judgment call is blast radius: work with no real chance
of silently breaking something doesn't need the heavyweight loop just
because it happened alongside work that does.

Bugs found *during* manual verification (step 6) are fixed in the same
worktree before merging, as part of the same phase — they don't get their
own separate pass through this whole loop.

## Adapting this to a new project

The eight steps above don't change; what changes per-project is just:

- The exact commands for step 5 (test runner, linter, formatter — see this
  repo's version below for the shape to match).
- Where manual verification (step 6) happens: a browser for a web UI, a
  terminal for a CLI, a running server for an API, etc.
- The worktree/branch naming convention and merge strategy used in step 3/7.
- Whatever review-hierarchy tooling is available (dedicated review
  subagents, a linting bot, a second model) — the *shape* of "independent
  correctness review, then independent cleanliness review" matters more
  than the specific tool.

## This repo's specifics

- **Worktrees**: created as sibling directories, e.g. `../monopoly-phase7`
  on a branch named for the phase (`phase7-<short-description>`), from
  `main`.
- **Automated verification** (run from the repo root unless noted):
  - Rust: `cargo build --workspace`, `cargo test --workspace`,
    `cargo clippy --workspace --all-targets -- -D warnings`,
    `cargo fmt --check`.
  - Wasm bindings (only needed before web verification):
    `wasm-pack build crates/engine-wasm --target web --out-dir
    ../../web/src/wasm` — this output directory is gitignored and *not*
    rebuilt automatically by a running `vite`/dev-server process, so it
    goes stale after any `crates/engine` change until this is re-run (a
    recurring source of "my change isn't showing up in the browser").
  - Web (from `web/`): `npx tsc -b --noEmit`, `npx vitest run`, and
    `npm run build` as a final production-build sanity check.
- **Manual verification**: `claude-in-chrome` for anything in `web/`
  (actually click through the feature in a live `vite preview` or dev
  server, don't just trust the automated tests); the `monopoly` CLI binary
  directly for anything in `crates/cli`/`crates/engine`/`crates/server`.
  When driving the browser for correctness-sensitive checks (toggling a
  specific checkbox, confirming a specific value), prefer
  `javascript_tool` DOM manipulation (`.click()`, set `.value` + dispatch
  `change`) over coordinate-based clicks and verify the result via a
  follow-up read (e.g. `.checked`) — a coordinate click can silently miss
  the element with no error.
- **Merging**: `git merge --no-ff` from `main` into the phase branch's
  worktree checkout (or vice versa), so each phase's merge is a single,
  identifiable commit in `main`'s history; then `git worktree remove` and
  `git branch -d` to clean up.
- **Docs**: every phase ends with a doc-reconciliation pass — `docs/roadmap.md`'s
  entry for the phase, and any other doc (`docs/game-rules.md`,
  `docs/player-strategies.md`, `docs/frontend.md`, `docs/headless-cli.md`,
  `docs/analysis-and-metrics.md`, `docs/data-model.md`,
  `docs/simulation-engine.md`) that describes what the phase actually
  changed, updated to match reality rather than the original plan.
