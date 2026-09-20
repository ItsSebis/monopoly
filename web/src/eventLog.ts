// Turns an already-decided event into a human-readable line. This only ever
// reads fields the engine already computed (amounts, indices) - it makes no
// decisions, so it isn't a second implementation of any rule.
import { spaceName } from "./board/layout";
import type { CardEffect, EventEnvelope } from "./types";

function describeCardEffect(effect: CardEffect): string {
  if (typeof effect === "string") {
    switch (effect) {
      case "AdvanceToNearestRailroad":
        return "advance to the nearest railroad";
      case "AdvanceToNearestUtility":
        return "advance to the nearest utility";
      case "GoBackThreeSpaces":
        return "go back three spaces";
      case "GoToJail":
        return "go to jail";
      case "GetOutOfJailFree":
        return "get out of jail free";
    }
  }
  if ("AdvanceTo" in effect) return `advance to ${spaceName(effect.AdvanceTo)}`;
  if ("CollectFromBank" in effect) return `collect $${effect.CollectFromBank} from the bank`;
  if ("PayBank" in effect) return `pay the bank $${effect.PayBank}`;
  if ("CollectFromEachPlayer" in effect) return `collect $${effect.CollectFromEachPlayer} from each player`;
  if ("PayEachPlayer" in effect) return `pay each player $${effect.PayEachPlayer}`;
  return `pay $${effect.PropertyRepairAssessment.per_house}/house, $${effect.PropertyRepairAssessment.per_hotel}/hotel`;
}

function playerName(playerNames: string[], i: number): string {
  return playerNames[i] ?? `Player ${i}`;
}

/** Formats a `GameEnded` payload on its own, so callers that only have the
 * winner/turns (not a full `EventEnvelope`) - e.g. main.ts's winner banner,
 * shown once playback has drained rather than in response to a real event -
 * can reuse the same wording `formatEvent` uses for an actual `GameEnded`. */
export function formatGameEnded(
  payload: { winner: number | null; turns: number },
  playerNames: string[],
): string {
  return payload.winner === null
    ? `Game ended after ${payload.turns} turns with no winner`
    : `${playerName(playerNames, payload.winner)} won after ${payload.turns} turns!`;
}

export function formatEvent(env: EventEnvelope, playerNames: string[]): string {
  const name = (i: number) => playerName(playerNames, i);
  const actor = name(env.player);
  const e = env.event;

  switch (e.type) {
    case "RollDice":
      return `${actor} rolled ${e.payload.dice[0]} + ${e.payload.dice[1]}`;
    case "Move":
      return `${actor} moved to ${spaceName(e.payload.to)}`;
    case "PassGo":
      return `${actor} passed GO and collected $${e.payload.amount}`;
    case "PropertyOffered":
      return `${spaceName(e.payload.space)} ($${e.payload.price}) offered to ${actor}`;
    case "PurchaseDecision":
      return e.payload.bought
        ? `${actor} bought ${spaceName(e.payload.space)}`
        : `${actor} declined ${spaceName(e.payload.space)}`;
    case "RentPaid":
      return `${actor} paid $${e.payload.amount} rent to ${name(e.payload.to)} for ${spaceName(e.payload.space)}`;
    case "TaxPaid":
      return `${actor} paid $${e.payload.amount} ${e.payload.kind.toLowerCase()} tax`;
    case "JailEntered":
      return `${actor} was sent to jail`;
    case "JailDecision":
      if (e.payload.action === "PayFine") {
        return e.payload.forced
          ? `${actor} was forced to pay the fine and leave jail`
          : `${actor} paid the fine to leave jail`;
      }
      return `${actor} rolled for doubles to try to leave jail`;
    case "JailExited":
      return `${actor} left jail`;
    case "UsedGetOutOfJailFreeCard":
      return `${actor} used a Get Out of Jail Free card`;
    case "HouseBuilt":
      return `${actor} built on ${spaceName(e.payload.space)}`;
    case "HouseSold":
      return `${actor} sold a house on ${spaceName(e.payload.space)}`;
    case "Mortgaged":
      return `${actor} mortgaged ${spaceName(e.payload.space)}`;
    case "CardDrawn":
      return `${actor} drew a ${e.payload.deck === "Chance" ? "Chance" : "Community Chest"} card: ${describeCardEffect(e.payload.effect)}`;
    case "AuctionBid":
      return e.payload.amount === null
        ? `${actor} passed on the auction`
        : `${actor} bid $${e.payload.amount}`;
    case "AuctionWon":
      return `${actor} won the auction for ${spaceName(e.payload.space)} at $${e.payload.amount}`;
    case "Bankrupted":
      return e.payload.payee === null
        ? `${actor} went bankrupt to the bank`
        : `${actor} went bankrupt to ${name(e.payload.payee)}`;
    case "TradeExecuted": {
      const gave = e.payload.offered_properties.map(spaceName).join(", ") || "nothing";
      const got = e.payload.requested_properties.map(spaceName).join(", ") || "nothing";
      const cashNote = [
        e.payload.offered_cash > 0 ? `+$${e.payload.offered_cash} to ${name(e.payload.to)}` : null,
        e.payload.requested_cash > 0 ? `+$${e.payload.requested_cash} from ${name(e.payload.to)}` : null,
      ]
        .filter(Boolean)
        .join(", ");
      return `${actor} traded with ${name(e.payload.to)}: gave ${gave}, received ${got}${cashNote ? ` (${cashNote})` : ""}`;
    }
    case "TradeDeclined":
      return `${actor}'s trade offer to ${name(e.payload.to)} was declined`;
    case "GameEnded":
      return formatGameEnded(e.payload, playerNames);
  }
}
