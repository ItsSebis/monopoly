// Mirrors the JSON shapes `engine` serializes (docs/data-model.md). This is
// the frontend's only source of truth for these shapes - nothing here makes
// a rules decision, it only types data the engine already produced.

import type { ColorGroup } from "./board/layout";

export type IncomeTaxMode =
  | { mode: "flat"; amount: number }
  | { mode: "percentage"; rate: number }
  | { mode: "choice"; flat_amount: number };

export interface RuleSet {
  starting_cash: number;
  go_salary: number;
  jail_fine: number;
  luxury_tax: number;
  income_tax_mode: IncomeTaxMode;
  even_build_rule: boolean;
  auction_on_decline: boolean;
  free_parking_pot: boolean;
  max_turns: number | null;
  double_go_salary: boolean;
  unlimited_houses: boolean;
  trading_enabled: boolean;
}

export interface PlayerConfig {
  name: string;
  strategy: string;
}

export interface GameConfig {
  rules: RuleSet;
  players: PlayerConfig[];
}

export interface PlayerState {
  name: string;
  cash: number;
  position: number;
  in_jail: boolean;
  jail_turns: number;
  bankrupt: boolean;
  goojf_cards: ("Chance" | "CommunityChest")[];
}

export interface PropertyState {
  owner: number | null;
  /** 0-4 = houses, 5 = hotel. */
  houses: number;
  mortgaged: boolean;
}

export interface GameState {
  turn: number;
  current_player: number;
  players: PlayerState[];
  /** Always 40 entries, indexed by space. */
  properties: PropertyState[];
  bank_houses_remaining: number;
  bank_hotels_remaining: number;
  free_parking_pot: number;
}

export type DeckKind = "Chance" | "CommunityChest";

export type CardEffect =
  | { AdvanceTo: number }
  | "AdvanceToNearestRailroad"
  | "AdvanceToNearestUtility"
  | { CollectFromBank: number }
  | { PayBank: number }
  | { CollectFromEachPlayer: number }
  | { PayEachPlayer: number }
  | { PropertyRepairAssessment: { per_house: number; per_hotel: number } }
  | "GoBackThreeSpaces"
  | "GoToJail"
  | "GetOutOfJailFree";

export type TaxKind = "Income" | "Luxury" | "Repair";
export type JailReason = "GoToJailSpace" | "ThreeDoubles" | "Card";
export type JailAction = "PayFine" | "RollForDoubles";

export type Event =
  | { type: "RollDice"; payload: { dice: [number, number] } }
  | { type: "Move"; payload: { from: number; to: number } }
  | { type: "PassGo"; payload: { amount: number } }
  | { type: "PropertyOffered"; payload: { space: number; price: number } }
  | { type: "PurchaseDecision"; payload: { space: number; bought: boolean } }
  | { type: "RentPaid"; payload: { to: number; amount: number; space: number } }
  | { type: "TaxPaid"; payload: { amount: number; kind: TaxKind } }
  | { type: "JailEntered"; payload: { reason: JailReason } }
  | { type: "JailDecision"; payload: { action: JailAction; forced: boolean } }
  | { type: "JailExited" }
  | { type: "UsedGetOutOfJailFreeCard" }
  | { type: "HouseBuilt"; payload: { space: number } }
  | { type: "HouseSold"; payload: { space: number } }
  | { type: "Mortgaged"; payload: { space: number } }
  | { type: "CardDrawn"; payload: { deck: DeckKind; effect: CardEffect } }
  | { type: "AuctionBid"; payload: { player: number; amount: number | null } }
  | { type: "AuctionWon"; payload: { player: number; space: number; amount: number } }
  | { type: "Bankrupted"; payload: { payee: number | null } }
  | {
      type: "TradeExecuted";
      payload: {
        to: number;
        offered_properties: number[];
        offered_cash: number;
        requested_properties: number[];
        requested_cash: number;
      };
    }
  | { type: "TradeDeclined"; payload: { to: number } }
  | { type: "GameEnded"; payload: { winner: number | null; turns: number } };

export interface EventEnvelope {
  turn: number;
  player: number;
  seq: number;
  event: Event;
}

// Mirrors of `docs/analysis-and-metrics.md`'s stat shapes
// (crates/engine/src/stats.rs, crates/engine/src/batch.rs) - a Run
// record's `final_stats`/`aggregate_stats`.

export interface CashFlowBreakdown {
  rent_paid: number;
  rent_received: number;
  tax_paid: number;
  card_net: number;
  go_salary_collected: number;
}

export interface PropertyAcquired {
  space: number;
  owner: number;
  turn: number;
}

export interface MonopolyCompleted {
  group: ColorGroup;
  owner: number;
  turn: number;
}

export interface BankruptcyRecord {
  player: number;
  turn: number;
  payee: number | null;
}

export interface PropertyRoi {
  space: number;
  owner: number | null;
  rent_collected: number;
  cost_basis: number;
}

export interface PerGameStats {
  winner: number | null;
  turns: number;
  net_worth_by_turn: number[][];
  cash_flow: CashFlowBreakdown[];
  property_timeline: PropertyAcquired[];
  monopolies_completed: MonopolyCompleted[];
  bankruptcies: BankruptcyRecord[];
  /** Indexed by `total - 2` for totals 2..=12. */
  dice_roll_counts: number[];
  /** One entry per board space. */
  landing_counts: number[];
  property_roi: PropertyRoi[];
}

export interface HeadToHead {
  wins: number;
  total: number;
}

export interface DistributionSummary {
  mean: number;
  median: number;
  p10: number;
  p90: number;
}

export interface AggregateStats {
  games: number;
  win_rate_by_strategy: Record<string, number>;
  head_to_head: Record<string, Record<string, HeadToHead>>;
  roi_by_strategy: Record<string, number>;
  game_length: number[];
  bankruptcy_turns: number[];
  dice_roll_counts: number[];
  landing_counts: number[];
  final_net_worth_by_strategy: Record<string, DistributionSummary>;
}

/** `seed` is a `bigint`, not `number`: it's a full-range u64 that routinely
 * exceeds `Number.MAX_SAFE_INTEGER` - see bigJson.ts, which api.ts uses to
 * parse/stringify these shapes without losing precision on it. */
export interface BatchGameSummary {
  seed: bigint;
  winner: number | null;
  turns: number;
}

// Mirrors of `docs/data-model.md#run-record` - the archived shapes. The
// server-assigned `id`/`kind`/`created_at` are added on top by `RunSummary`/
// `RunDetail` below, matching what `GET /runs`/`GET /runs/{id}` actually
// return.

export interface SingleRunRecord {
  rule_set: RuleSet;
  players: PlayerConfig[];
  seed: bigint;
  events: EventEnvelope[];
  final_stats: PerGameStats;
}

export interface BatchRunRecord {
  rule_set: RuleSet;
  players: PlayerConfig[];
  seeds: bigint[];
  per_game_summary: BatchGameSummary[];
  aggregate_stats: AggregateStats;
}

/** `GET /runs` list item (`docs/api.md#get-runs`) - a lightweight preview,
 * not a full record. */
export interface RunSummary {
  id: string;
  kind: "single" | "batch";
  created_at: string;
  rule_set: RuleSet;
  players: PlayerConfig[];
  winner?: number | null;
  games?: number;
}

export type RunDetail =
  | ({ id: string; kind: "single"; created_at: string } & SingleRunRecord)
  | ({ id: string; kind: "batch"; created_at: string } & BatchRunRecord);

// Mirrors of `docs/api.md#interactive-sessions-phase-9`'s shapes
// (crates/server/src/interactive/{routes,decision,board}.rs) - the 5
// interactive-session endpoints `interactive/sessionController.ts` talks to.

/** `GET /board` - static mechanical board data (`BoardSpaceDto`,
 * `crates/server/src/interactive/board.rs`). `kind`/`group` mirror the
 * engine's own `&'static str`/`ColorGroup` serialization exactly. */
export interface BoardSpaceDto {
  index: number;
  kind:
    | "go"
    | "street"
    | "railroad"
    | "utility"
    | "income_tax"
    | "luxury_tax"
    | "chance"
    | "community_chest"
    | "jail"
    | "free_parking"
    | "go_to_jail";
  group?: ColorGroup;
  price?: number;
  base_rent?: number;
  /** Rent with 1-4 houses (indices 0-3) and with a hotel (index 4). */
  house_rent?: [number, number, number, number, number];
  house_cost?: number;
  mortgage_value?: number;
}

export interface PurchaseOffer {
  space: number;
  price: number;
}

export type BuildAction = { Build: number } | { SellHouse: number };
export type MortgageAction = { Mortgage: number } | { SellHouse: number };

export interface TradeOffer {
  to: number;
  offered_properties: number[];
  offered_cash: number;
  requested_properties: number[];
  requested_cash: number;
}

/** `PendingDecision` (`crates/server/src/interactive/decision.rs`) - the 7
 * `Strategy` hooks, tagged by `kind`. Published by a session snapshot when
 * the human seat is the one blocked on a decision. */
export type PendingDecision =
  | { kind: "Purchase"; player: number; offer: PurchaseOffer }
  | { kind: "JailAction"; player: number }
  | { kind: "Build"; player: number }
  | { kind: "Mortgage"; player: number; shortfall: number }
  | { kind: "AuctionBid"; player: number; space: number }
  | { kind: "TradeProposal"; player: number }
  | { kind: "TradeResponse"; player: number; offer: TradeOffer };

/** `DecisionAnswer` - the client's answer to whatever `PendingDecision` is
 * currently pending, posted to `POST /sessions/{id}/decisions`. */
export type DecisionAnswer =
  | { kind: "Purchase"; buy: boolean }
  | { kind: "JailAction"; action: JailAction }
  | { kind: "Build"; actions: BuildAction[] }
  | { kind: "Mortgage"; actions: MortgageAction[] }
  | { kind: "AuctionBid"; amount: number | null }
  | { kind: "TradeProposal"; offer: TradeOffer | null }
  | { kind: "TradeResponse"; accept: boolean };

export interface GameOver {
  winner: number | null;
  turns: number;
}

export interface CreateSessionRequest {
  rules?: RuleSet;
  players: PlayerConfig[];
  human_seat: number;
  seed?: number;
}

export interface SessionSnapshot {
  id: string;
  human_seat: number;
  state: GameState;
  pending: PendingDecision | null;
  events: EventEnvelope[];
  seq: number;
  game_over: GameOver | null;
  errored: string | null;
}
