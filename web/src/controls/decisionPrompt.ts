// One reusable shell for all 7 `PendingDecision` kinds (docs/simulation-engine.md
// #interactive-sessions-the-human-strategy), docked at the top of the side
// panel rather than a center modal - the board stays visible while a human
// decision is pending, matching how CPU turns already play out on it. Each
// kind gets its own body renderer below; the shell only owns the
// title/temperature/entrance-animation chrome and submit wiring.
import { colorForGroup } from "../board/board";
import { BOARD_LAYOUT, spaceName } from "../board/layout";
import type {
  BoardSpaceDto,
  BuildAction,
  DecisionAnswer,
  GameState,
  MortgageAction,
  PendingDecision,
  RuleSet,
  TradeOffer,
} from "../types";

export interface DecisionContext {
  pending: PendingDecision;
  state: GameState;
  humanSeat: number;
  boardData: BoardSpaceDto[];
  playerNames: string[];
  ruleSet: RuleSet | null;
}

type Submit = (answer: DecisionAnswer) => void;

const TITLES: Record<PendingDecision["kind"], string> = {
  Purchase: "Buy this property?",
  JailAction: "You're in jail",
  Build: "Build houses",
  Mortgage: "Raise cash",
  AuctionBid: "Auction",
  TradeProposal: "Propose a trade",
  TradeResponse: "Incoming trade offer",
};

/** Affordance blue for a routine/optional choice, warning amber for a
 * raise-cash decision - the one kind where the player is being asked to
 * cover a shortfall, not just opting into something. */
const TEMPERATURE: Record<PendingDecision["kind"], "info" | "warning"> = {
  Purchase: "info",
  JailAction: "info",
  Build: "info",
  Mortgage: "warning",
  AuctionBid: "info",
  TradeProposal: "info",
  TradeResponse: "info",
};

function el<K extends keyof HTMLElementTagNameMap>(tag: K, className?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  return node;
}

function button(label: string, variant: "primary" | "secondary" | "stepper", onClick: () => void): HTMLButtonElement {
  const b = el("button", `decision-button decision-button--${variant}`);
  b.type = "button";
  b.textContent = label;
  b.addEventListener("click", onClick);
  return b;
}

function labeled(text: string, control: HTMLElement): HTMLLabelElement {
  const label = el("label", "decision-field");
  label.append(`${text} `, control);
  return label;
}

function groupSwatch(group: string | null): HTMLElement | null {
  if (!group) return null;
  const swatch = el("span", "decision-group-swatch");
  swatch.style.background = colorForGroup(group);
  return swatch;
}

function numberInput(min: number, max: number | undefined): HTMLInputElement {
  const input = el("input", "decision-number");
  input.type = "number";
  input.min = String(min);
  if (max !== undefined) input.max = String(max);
  input.value = "0";
  return input;
}

function renderPurchase(body: HTMLElement, ctx: DecisionContext, submit: Submit): void {
  if (ctx.pending.kind !== "Purchase") return;
  const { offer } = ctx.pending;
  const space = BOARD_LAYOUT[offer.space];
  const dto = ctx.boardData[offer.space];

  const header = el("div", "decision-purchase-header");
  const swatch = groupSwatch(space?.colorGroup ?? null);
  if (swatch) header.appendChild(swatch);
  const title = el("strong");
  title.textContent = spaceName(offer.space);
  header.appendChild(title);
  body.appendChild(header);

  const price = el("p", "decision-money");
  price.textContent = `Price: $${offer.price}`;
  body.appendChild(price);

  if (dto?.base_rent !== undefined && dto.house_rent) {
    const table = el("table", "decision-rent-table");
    const rows: [string, number][] = [
      ["No houses", dto.base_rent],
      ["1 house", dto.house_rent[0]],
      ["2 houses", dto.house_rent[1]],
      ["3 houses", dto.house_rent[2]],
      ["4 houses", dto.house_rent[3]],
      ["Hotel", dto.house_rent[4]],
    ];
    for (const [label, rent] of rows) {
      const tr = el("tr");
      const th = el("td");
      th.textContent = label;
      const td = el("td");
      td.className = "decision-money";
      td.textContent = `$${rent}`;
      tr.append(th, td);
      table.appendChild(tr);
    }
    body.appendChild(table);
  }

  const actions = el("div", "decision-actions");
  actions.append(
    button("Buy", "primary", () => submit({ kind: "Purchase", buy: true })),
    button("Pass", "secondary", () => submit({ kind: "Purchase", buy: false })),
  );
  body.appendChild(actions);
}

function renderJailAction(body: HTMLElement, ctx: DecisionContext, submit: Submit): void {
  if (ctx.pending.kind !== "JailAction") return;
  const player = ctx.state.players[ctx.humanSeat];
  const info = el("p");
  const fineNote = ctx.ruleSet ? ` The fine is $${ctx.ruleSet.jail_fine}.` : "";
  info.textContent = `Turn ${player.jail_turns + 1} in jail. Your cash: $${player.cash}.${fineNote}`;
  body.appendChild(info);

  const actions = el("div", "decision-actions");
  actions.append(
    button("Pay fine", "primary", () => submit({ kind: "JailAction", action: "PayFine" })),
    button("Roll for doubles", "secondary", () => submit({ kind: "JailAction", action: "RollForDoubles" })),
  );
  body.appendChild(actions);
}

function renderBuild(body: HTMLElement, ctx: DecisionContext, submit: Submit): void {
  if (ctx.pending.kind !== "Build") return;
  const own = ctx.state.properties
    .map((p, index) => ({ ...p, index }))
    .filter((p) => p.owner === ctx.humanSeat && !p.mortgaged);

  const staged = new Map<number, number>();
  const grid = el("div", "decision-build-grid");
  if (own.length === 0) {
    const none = el("p");
    none.textContent = "You have no eligible properties.";
    body.appendChild(none);
  }
  for (const prop of own) {
    const space = BOARD_LAYOUT[prop.index];
    const row = el("div", "decision-build-row");
    const swatch = groupSwatch(space?.colorGroup ?? null);
    if (swatch) row.appendChild(swatch);
    const label = el("span", "decision-build-label");
    label.textContent = `${spaceName(prop.index)} (${prop.houses === 5 ? "hotel" : `${prop.houses} houses`})`;
    row.appendChild(label);

    const counter = el("span", "decision-stepper-count");
    counter.textContent = "0";
    const stage = (delta: number) => {
      const next = (staged.get(prop.index) ?? 0) + delta;
      staged.set(prop.index, next);
      counter.textContent = next > 0 ? `+${next}` : String(next);
    };
    row.append(button("−", "stepper", () => stage(-1)), counter, button("+", "stepper", () => stage(1)));
    grid.appendChild(row);
  }
  body.appendChild(grid);

  const actions = el("div", "decision-actions");
  actions.append(
    button("Confirm", "primary", () => {
      const buildActions: BuildAction[] = [];
      for (const [space, net] of staged) {
        const count = Math.abs(net);
        for (let i = 0; i < count; i++) {
          buildActions.push(net > 0 ? { Build: space } : { SellHouse: space });
        }
      }
      submit({ kind: "Build", actions: buildActions });
    }),
    button("Skip", "secondary", () => submit({ kind: "Build", actions: [] })),
  );
  body.appendChild(actions);
}

function renderMortgage(body: HTMLElement, ctx: DecisionContext, submit: Submit): void {
  if (ctx.pending.kind !== "Mortgage") return;
  const { shortfall } = ctx.pending;
  const own = ctx.state.properties.map((p, index) => ({ ...p, index })).filter((p) => p.owner === ctx.humanSeat);

  const header = el("p", "decision-shortfall");
  header.textContent = `You need to raise $${shortfall}.`;
  body.appendChild(header);

  const staged: MortgageAction[] = [];
  let raised = 0;
  const totalEl = el("p", "decision-running-total");
  const updateTotal = () => {
    const met = raised >= shortfall;
    totalEl.textContent = `Estimated raised: $${raised} of $${shortfall}${met ? " — target met" : ""}`;
    totalEl.classList.toggle("decision-running-total--met", met);
  };
  updateTotal();

  const list = el("div", "decision-mortgage-list");
  for (const prop of own) {
    const dto = ctx.boardData[prop.index];
    const row = el("div", "decision-mortgage-row");
    const label = el("span");
    label.textContent = spaceName(prop.index);
    row.appendChild(label);

    if (prop.houses > 0) {
      const perHouse = Math.floor((dto?.house_cost ?? 0) / 2);
      let remaining = prop.houses;
      const sellBtn = button(`Sell house (+$${perHouse})`, "stepper", () => {
        remaining -= 1;
        raised += perHouse;
        staged.push({ SellHouse: prop.index });
        updateTotal();
        if (remaining <= 0) sellBtn.disabled = true;
      });
      row.appendChild(sellBtn);
    }
    if (!prop.mortgaged) {
      const value = dto?.mortgage_value ?? 0;
      const mortgageBtn = button(`Mortgage (+$${value})`, "stepper", () => {
        mortgageBtn.disabled = true;
        raised += value;
        staged.push({ Mortgage: prop.index });
        updateTotal();
      });
      row.appendChild(mortgageBtn);
    }
    list.appendChild(row);
  }
  body.appendChild(list);
  body.appendChild(totalEl);

  const actions = el("div", "decision-actions");
  actions.append(button("Confirm", "primary", () => submit({ kind: "Mortgage", actions: staged })));
  body.appendChild(actions);
}

function renderAuctionBid(body: HTMLElement, ctx: DecisionContext, submit: Submit): void {
  if (ctx.pending.kind !== "AuctionBid") return;
  const { space } = ctx.pending;
  const cash = ctx.state.players[ctx.humanSeat].cash;

  const info = el("p");
  info.textContent = `${spaceName(space)} is up for auction. Your cash: $${cash}.`;
  body.appendChild(info);

  const input = numberInput(0, Math.max(0, cash));
  body.appendChild(labeled("Your bid", input));

  const actions = el("div", "decision-actions");
  actions.append(
    button("Submit bid", "primary", () => {
      const amount = Math.max(0, Math.min(cash, Math.floor(Number(input.value) || 0)));
      submit({ kind: "AuctionBid", amount });
    }),
    button("Abstain", "secondary", () => submit({ kind: "AuctionBid", amount: null })),
  );
  body.appendChild(actions);
}

/** Only unmortgaged, house-free properties are ever tradable
 * (docs/game-rules.md#trading) - pre-filtered here so every checkbox in the
 * picker is a legal candidate, not a UI-side re-implementation of that rule
 * (the engine still validates the submitted offer regardless). */
function tradableProperties(state: GameState, owner: number): { index: number }[] {
  return state.properties
    .map((p, index) => ({ ...p, index }))
    .filter((p) => p.owner === owner && !p.mortgaged && p.houses === 0);
}

function renderTradeProposal(body: HTMLElement, ctx: DecisionContext, submit: Submit): void {
  if (ctx.pending.kind !== "TradeProposal") return;
  const others = ctx.state.players
    .map((p, index) => ({ ...p, index }))
    .filter((p) => p.index !== ctx.humanSeat && !p.bankrupt);

  const actions = el("div", "decision-actions");
  if (others.length === 0) {
    const none = el("p");
    none.textContent = "No other active players to trade with.";
    body.appendChild(none);
    actions.append(button("Skip", "secondary", () => submit({ kind: "TradeProposal", offer: null })));
    body.appendChild(actions);
    return;
  }

  let counterparty = others[0].index;
  const offeredProps = new Set<number>();
  const requestedProps = new Set<number>();

  const picker = el("select", "decision-trade-picker");
  for (const other of others) {
    const option = el("option");
    option.value = String(other.index);
    option.textContent = ctx.playerNames[other.index] ?? `Player ${other.index}`;
    picker.appendChild(option);
  }
  body.appendChild(labeled("Trade with", picker));

  const columns = el("div", "decision-trade-columns");
  body.appendChild(columns);

  function buildColumn(title: string, owner: number, selected: Set<number>): HTMLElement {
    const col = el("div", "decision-trade-column");
    const heading = el("h4");
    heading.textContent = title;
    col.appendChild(heading);
    const props = tradableProperties(ctx.state, owner);
    if (props.length === 0) {
      const p = el("p");
      p.textContent = "Nothing tradable.";
      col.appendChild(p);
    }
    for (const prop of props) {
      const row = el("label", "decision-trade-item");
      const checkbox = el("input");
      checkbox.type = "checkbox";
      checkbox.addEventListener("change", () => {
        if (checkbox.checked) selected.add(prop.index);
        else selected.delete(prop.index);
      });
      row.append(checkbox, ` ${spaceName(prop.index)}`);
      col.appendChild(row);
    }
    return col;
  }

  function renderColumns(): void {
    columns.innerHTML = "";
    offeredProps.clear();
    requestedProps.clear();
    columns.append(
      buildColumn("You give", ctx.humanSeat, offeredProps),
      buildColumn("You get", counterparty, requestedProps),
    );
  }
  picker.addEventListener("change", () => {
    counterparty = Number(picker.value);
    renderColumns();
  });
  renderColumns();

  const offeredCashInput = numberInput(0, Math.max(0, ctx.state.players[ctx.humanSeat].cash));
  const requestedCashInput = numberInput(0, undefined);
  const cashRow = el("div", "decision-trade-cash");
  cashRow.append(labeled("Cash you give", offeredCashInput), labeled("Cash you want", requestedCashInput));
  body.appendChild(cashRow);

  actions.append(
    button("Propose", "primary", () => {
      const offer: TradeOffer = {
        to: counterparty,
        offered_properties: [...offeredProps],
        offered_cash: Math.max(0, Math.floor(Number(offeredCashInput.value) || 0)),
        requested_properties: [...requestedProps],
        requested_cash: Math.max(0, Math.floor(Number(requestedCashInput.value) || 0)),
      };
      submit({ kind: "TradeProposal", offer });
    }),
    button("Skip", "secondary", () => submit({ kind: "TradeProposal", offer: null })),
  );
  body.appendChild(actions);
}

function readonlyTradeColumn(title: string, properties: number[], cash: number): HTMLElement {
  const col = el("div", "decision-trade-column");
  const heading = el("h4");
  heading.textContent = title;
  col.appendChild(heading);
  const list = el("ul", "decision-trade-list");
  if (properties.length === 0) {
    const li = el("li");
    li.textContent = "Nothing";
    list.appendChild(li);
  }
  for (const space of properties) {
    const li = el("li");
    li.textContent = spaceName(space);
    list.appendChild(li);
  }
  col.appendChild(list);
  if (cash > 0) {
    const p = el("p", "decision-money");
    p.textContent = `+ $${cash}`;
    col.appendChild(p);
  }
  return col;
}

function renderTradeResponse(body: HTMLElement, ctx: DecisionContext, submit: Submit): void {
  if (ctx.pending.kind !== "TradeResponse") return;
  const { offer } = ctx.pending;
  // `TradeOffer` carries no "from" field - only the current turn's own
  // player ever proposes a trade (`Strategy::decide_trade`'s doc comment),
  // so `state.current_player` is always the proposer here.
  const proposer = ctx.playerNames[ctx.state.current_player] ?? `Player ${ctx.state.current_player}`;

  const header = el("p");
  header.textContent = `${proposer} proposes:`;
  body.appendChild(header);

  const columns = el("div", "decision-trade-columns");
  columns.append(
    readonlyTradeColumn("They give you", offer.offered_properties, offer.offered_cash),
    readonlyTradeColumn("They want", offer.requested_properties, offer.requested_cash),
  );
  body.appendChild(columns);

  const actions = el("div", "decision-actions");
  actions.append(
    button("Accept", "primary", () => submit({ kind: "TradeResponse", accept: true })),
    button("Decline", "secondary", () => submit({ kind: "TradeResponse", accept: false })),
  );
  body.appendChild(actions);
}

const RENDERERS: Record<PendingDecision["kind"], (body: HTMLElement, ctx: DecisionContext, submit: Submit) => void> =
  {
    Purchase: renderPurchase,
    JailAction: renderJailAction,
    Build: renderBuild,
    Mortgage: renderMortgage,
    AuctionBid: renderAuctionBid,
    TradeProposal: renderTradeProposal,
    TradeResponse: renderTradeResponse,
  };

export class DecisionPrompt {
  constructor(
    private readonly container: HTMLElement,
    private readonly onAnswer: (answer: DecisionAnswer) => void,
  ) {}

  show(ctx: DecisionContext): void {
    this.container.innerHTML = "";
    this.container.hidden = false;
    this.container.className = `decision-prompt decision-prompt--${TEMPERATURE[ctx.pending.kind]}`;
    // Forces a reflow so re-triggering the entrance animation (a second
    // decision arriving before the CSS animation class was ever removed)
    // actually restarts it instead of a no-op class toggle.
    void this.container.offsetWidth;
    this.container.classList.add("decision-prompt--enter");

    const heading = el("h3", "decision-heading");
    heading.textContent = TITLES[ctx.pending.kind];
    this.container.appendChild(heading);

    const body = el("div", "decision-body");
    this.container.appendChild(body);

    const submit: Submit = (answer) => {
      this.hide();
      this.onAnswer(answer);
    };
    RENDERERS[ctx.pending.kind](body, ctx, submit);
  }

  hide(): void {
    this.container.hidden = true;
    this.container.innerHTML = "";
  }
}
