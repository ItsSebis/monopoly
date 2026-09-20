import { describe, expect, it } from "vitest";
import { formatEvent } from "./eventLog";
import type { EventEnvelope } from "./types";

const names = ["Alice", "Bob"];

function envelope(player: number, event: EventEnvelope["event"]): EventEnvelope {
  return { turn: 1, player, seq: 0, event };
}

describe("formatEvent", () => {
  it("formats passing GO with the amount actually collected", () => {
    expect(formatEvent(envelope(0, { type: "PassGo", payload: { amount: 200 } }), names)).toBe(
      "Alice passed GO and collected $200",
    );
    // Under RuleSet.double_go_salary, landing exactly on GO carries double.
    expect(formatEvent(envelope(0, { type: "PassGo", payload: { amount: 400 } }), names)).toBe(
      "Alice passed GO and collected $400",
    );
  });

  it("formats rent paid with both players' names and the space", () => {
    const line = formatEvent(
      envelope(0, { type: "RentPaid", payload: { to: 1, amount: 44, space: 39 } }),
      names,
    );
    expect(line).toBe("Alice paid $44 rent to Bob for Boardwalk");
  });

  it("formats a purchase decision", () => {
    const bought = formatEvent(
      envelope(1, { type: "PurchaseDecision", payload: { space: 1, bought: true } }),
      names,
    );
    expect(bought).toBe("Bob bought Mediterranean Avenue");

    const declined = formatEvent(
      envelope(1, { type: "PurchaseDecision", payload: { space: 1, bought: false } }),
      names,
    );
    expect(declined).toBe("Bob declined Mediterranean Avenue");
  });

  it("formats bankruptcy to a player and to the bank", () => {
    expect(formatEvent(envelope(0, { type: "Bankrupted", payload: { payee: 1 } }), names)).toBe(
      "Alice went bankrupt to Bob",
    );
    expect(formatEvent(envelope(0, { type: "Bankrupted", payload: { payee: null } }), names)).toBe(
      "Alice went bankrupt to the bank",
    );
  });

  it("formats game end with and without a winner", () => {
    expect(
      formatEvent(envelope(0, { type: "GameEnded", payload: { winner: 0, turns: 87 } }), names),
    ).toBe("Alice won after 87 turns!");
    expect(
      formatEvent(envelope(0, { type: "GameEnded", payload: { winner: null, turns: 1000 } }), names),
    ).toBe("Game ended after 1000 turns with no winner");
  });

  it("formats an executed trade, including cash notes on either side", () => {
    const swap = formatEvent(
      envelope(0, {
        type: "TradeExecuted",
        payload: {
          to: 1,
          offered_properties: [1],
          offered_cash: 0,
          requested_properties: [3],
          requested_cash: 0,
        },
      }),
      names,
    );
    expect(swap).toBe("Alice traded with Bob: gave Mediterranean Avenue, received Baltic Avenue");

    const withCash = formatEvent(
      envelope(0, {
        type: "TradeExecuted",
        payload: {
          to: 1,
          offered_properties: [],
          offered_cash: 100,
          requested_properties: [3],
          requested_cash: 0,
        },
      }),
      names,
    );
    expect(withCash).toBe(
      "Alice traded with Bob: gave nothing, received Baltic Avenue (+$100 to Bob)",
    );
  });

  it("formats a declined trade", () => {
    const line = formatEvent(envelope(0, { type: "TradeDeclined", payload: { to: 1 } }), names);
    expect(line).toBe("Alice's trade offer to Bob was declined");
  });

  it("describes a card effect", () => {
    const line = formatEvent(
      envelope(0, { type: "CardDrawn", payload: { deck: "Chance", effect: { AdvanceTo: 39 } } }),
      names,
    );
    expect(line).toBe("Alice drew a Chance card: advance to Boardwalk");
  });
});
