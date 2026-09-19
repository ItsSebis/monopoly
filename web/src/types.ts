// Mirrors the JSON shapes `engine` serializes (docs/data-model.md). This is
// the frontend's only source of truth for these shapes - nothing here makes
// a rules decision, it only types data the engine already produced.

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
  | { type: "PassGo" }
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
  | { type: "GameEnded"; payload: { winner: number | null; turns: number } };

export interface EventEnvelope {
  turn: number;
  player: number;
  seq: number;
  event: Event;
}
