# Data Model

These are the shared schemas every other doc references. They're serialized with `serde` (JSON on the wire for the CLI/API/web, and directly as Rust structs inside `engine`) so the same types flow through the whole system with no translation layer.

## RuleSet

The full configuration of "which Monopoly are we playing" — everything in [game-rules.md](./game-rules.md)'s configurable-toggles sections lives here.

```jsonc
{
  "starting_cash": 1500,
  "go_salary": 200,
  "jail_fine": 50,
  "luxury_tax": 75,
  "income_tax_mode": "choice",       // "flat" | "percentage" | "choice"
  "even_build_rule": true,
  "auction_on_decline": true,
  "free_parking_pot": false,
  "max_turns": null                  // optional cap, mainly for batch runs to bound worst-case game length
}
```

## PlayerConfig

One entry per player in a game.

```jsonc
{
  "name": "P1",
  "strategy": "buy_good",            // registered strategy id
  "strategy_params": { "reserve": 150 }  // strategy-specific overrides, see player-strategies.md
}
```

A game is fully specified by `RuleSet + Vec<PlayerConfig> + seed` — this triple is what gets archived for a batch game (see [Run record](#run-record) below) and what a CLI invocation takes as input (see [headless-cli.md](./headless-cli.md)).

## GameState

The engine's live internal state, exposed read-only to strategies as `GameView` and to the browser/CLI as a snapshot.

```jsonc
{
  "turn": 42,
  "current_player": 1,
  "players": [
    {
      "name": "P1", "cash": 640, "position": 24,
      "properties": [24, 25], "in_jail": false, "jail_turns": 0,
      "get_out_of_jail_cards": 0, "bankrupt": false
    }
  ],
  "board": [
    { "space": 24, "owner": 0, "houses": 2, "mortgaged": false }
    // one entry per ownable space; unowned/non-ownable spaces omitted or null
  ],
  "bank": { "houses_remaining": 26, "hotels_remaining": 12 },
  "free_parking_pot": 0
}
```

## Event log entry

Every event carries a common envelope plus a type-specific payload.

```jsonc
{ "turn": 42, "player": 1, "seq": 187, "type": "RentPaid",
  "payload": { "from": 1, "to": 0, "amount": 44, "property": 24 } }
```

`seq` is a monotonically increasing index across the whole game, used to order events for playback and to make individual events addressable (e.g. "step to event 187") for the UI's step-through control.

## Run record

The archived shape (server + `docs/api.md`). Deliberately different for single vs. batch runs, per the [determinism-as-storage-optimization](./architecture.md#determinism-as-a-storage-optimization) decision:

**Single run** (explicitly saved live game):

```jsonc
{
  "id": "run_01hz...", "kind": "single", "created_at": "2026-01-01T00:00:00Z",
  "rule_set": { /* RuleSet */ }, "players": [ /* PlayerConfig */ ], "seed": 1234,
  "events": [ /* full event log */ ],
  "final_stats": { /* see analysis-and-metrics.md */ }
}
```

**Batch run**:

```jsonc
{
  "id": "run_01hz...", "kind": "batch", "created_at": "2026-01-01T00:00:00Z",
  "rule_set": { /* RuleSet */ }, "players": [ /* PlayerConfig */ ],
  "seeds": [1, 2, 3 /* ...N */],
  "per_game_summary": [ { "seed": 1, "winner": 0, "turns": 87 } /* ...N, no full event logs */ ],
  "aggregate_stats": { /* see analysis-and-metrics.md */ }
}
```

Any individual game inside a batch run can be re-expanded to its full event log on demand by re-running the engine with `(rule_set, players, seed)` — the server does this lazily when the UI asks to inspect one specific game from a batch (see [api.md](./api.md#get-runsidgamesseed)).

**As of Phase 3** (pre-Phase-4, no server yet), `monopoly run --out`/`monopoly batch --out` write the core of this shape directly, without the archive envelope (no `id`/`kind`/`created_at` — those are added when Phase 4's server actually archives a run): `{seed, winner, turns, events, final_state, final_stats}` for a single run, `{base_seed, per_game, aggregate}` for a batch (`per_game`/`aggregate` here are what this doc calls `per_game_summary`/`aggregate_stats` — Phase 4 settles the exact archived field names when it defines the wire format for real).
