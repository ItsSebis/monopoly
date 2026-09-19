import { describe, expect, it } from "vitest";
import { buildIncomeTaxMode, buildPlayers, buildRuleSet } from "./configForm";

function formData(fields: Record<string, string>): FormData {
  const data = new FormData();
  for (const [key, value] of Object.entries(fields)) data.set(key, value);
  return data;
}

describe("buildIncomeTaxMode", () => {
  it("defaults to choice", () => {
    expect(buildIncomeTaxMode(formData({ income_tax_flat_amount: "200" }))).toEqual({
      mode: "choice",
      flat_amount: 200,
    });
  });

  it("builds flat", () => {
    expect(
      buildIncomeTaxMode(formData({ income_tax_mode: "flat", income_tax_flat_amount: "150" })),
    ).toEqual({ mode: "flat", amount: 150 });
  });

  it("builds percentage", () => {
    expect(
      buildIncomeTaxMode(formData({ income_tax_mode: "percentage", income_tax_rate: "0.1" })),
    ).toEqual({ mode: "percentage", rate: 0.1 });
  });
});

describe("buildRuleSet", () => {
  it("maps every field to the documented RuleSet shape (docs/data-model.md)", () => {
    const rules = buildRuleSet(
      formData({
        starting_cash: "1500",
        go_salary: "200",
        jail_fine: "50",
        luxury_tax: "75",
        income_tax_mode: "choice",
        income_tax_flat_amount: "200",
        even_build_rule: "on",
        auction_on_decline: "on",
        max_turns: "1000",
      }),
    );
    expect(rules).toEqual({
      starting_cash: 1500,
      go_salary: 200,
      jail_fine: 50,
      luxury_tax: 75,
      income_tax_mode: { mode: "choice", flat_amount: 200 },
      even_build_rule: true,
      auction_on_decline: true,
      free_parking_pot: false, // checkbox omitted from FormData when unchecked
      max_turns: 1000,
    });
  });

  it("maps an omitted max_turns to null, matching RuleSet's optional cap", () => {
    const rules = buildRuleSet(formData({ starting_cash: "1500" }));
    expect(rules.max_turns).toBeNull();
  });
});

describe("buildPlayers", () => {
  it("maps player rows to PlayerConfig", () => {
    expect(buildPlayers([{ name: "P1", strategy: "buy_good" }])).toEqual([
      { name: "P1", strategy: "buy_good" },
    ]);
  });
});
