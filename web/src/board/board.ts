import { BOARD_LAYOUT, displayName, type BoardLang } from "./layout";
import type { BoardSpaceDto, EventEnvelope, GameState } from "../types";

const JAIL_SPACE = 10;

/** Renders the 40-space board and player tokens from a `GameState` snapshot,
 * plus a light per-event animation layer for token movement (see
 * docs/frontend.md). Ownership/house/cash display only ever comes from a
 * full `renderState()` call - `applyEvent()` never invents or derives state,
 * it only moves a token to a position the event itself already names. */
export class BoardView {
  private spaceEls: HTMLElement[] = [];
  private tokenContainers: HTMLElement[] = [];
  private nameEls: HTMLElement[] = [];
  private houseEls: HTMLElement[] = [];
  private tooltipEls: HTMLElement[] = [];
  private lastOwner: (number | null)[] = BOARD_LAYOUT.map(() => null);
  private lastHouses: number[] = BOARD_LAYOUT.map(() => 0);
  private boardData: BoardSpaceDto[] = [];

  constructor(container: HTMLElement, lang: BoardLang) {
    container.innerHTML = "";
    for (const space of BOARD_LAYOUT) {
      const el = document.createElement("div");
      el.className = "space";
      el.style.gridRow = String(space.row + 1);
      el.style.gridColumn = String(space.col + 1);

      // Everything that must stay clipped to the tile (tight board cells
      // easily overflow a long property name) lives in `.space-inner`;
      // `.space` itself stays `overflow: visible` so the hover tooltip -
      // deliberately positioned *above* the tile - isn't clipped by the
      // same rule that keeps the name/houses text contained.
      const inner = document.createElement("div");
      inner.className = "space-inner";
      el.appendChild(inner);

      if (space.colorGroup) {
        const bar = document.createElement("div");
        bar.className = "color-bar";
        bar.style.background = colorForGroup(space.colorGroup);
        inner.appendChild(bar);
      }
      const tokens = document.createElement("div");
      tokens.className = "tokens";
      inner.appendChild(tokens);
      const name = document.createElement("div");
      name.className = "name";
      name.textContent = displayName(space, lang);
      inner.appendChild(name);
      const houses = document.createElement("div");
      houses.className = "houses";
      inner.appendChild(houses);
      const tooltip = document.createElement("div");
      tooltip.className = "rent-tooltip";
      el.appendChild(tooltip);

      container.appendChild(el);
      this.spaceEls.push(el);
      this.tokenContainers.push(tokens);
      this.nameEls.push(name);
      this.houseEls.push(houses);
      this.tooltipEls.push(tooltip);
    }
  }

  /** Enriches the hover tooltip with real price/rent data from `GET /board`
   * (`interactive/sessionController.ts` fetches this once at startup) -
   * optional and purely additive, so plain offline live/batch play still
   * renders a perfectly usable board without ever calling the server. */
  setBoardData(data: BoardSpaceDto[]): void {
    this.boardData = data;
    BOARD_LAYOUT.forEach((space, index) => {
      this.tooltipEls[index].textContent = this.tooltipText(space.name, this.boardData[index]);
    });
  }

  private tooltipText(name: string, dto: BoardSpaceDto | undefined): string {
    if (!dto) return name;
    if (dto.price === undefined) return name;
    const parts = [`${name} — $${dto.price}`];
    if (dto.base_rent !== undefined) parts.push(`rent $${dto.base_rent}`);
    if (dto.house_rent) parts.push(`up to $${dto.house_rent[4]} with a hotel`);
    if (dto.mortgage_value !== undefined) parts.push(`mortgage $${dto.mortgage_value}`);
    return parts.join(", ");
  }

  /** Switches the board's own space labels between English and German - see
   * `layout.ts`'s `BoardLang`. Nothing else about the board (colors,
   * ownership, tokens) is language-dependent. */
  setLanguage(lang: BoardLang): void {
    BOARD_LAYOUT.forEach((space, index) => {
      this.nameEls[index].textContent = displayName(space, lang);
    });
  }

  /** Full re-sync to the engine's authoritative state - the ground truth,
   * called once per turn. Diffs against the previous call's ownership/house
   * count per space so only what actually changed gets a brief change-pulse
   * - the CSS `transition` on `.space`'s own background handles ownership
   * smoothly for free, this only covers the house-count text, which a plain
   * `textContent` swap can't tween. */
  renderState(state: GameState): void {
    for (let space = 0; space < this.spaceEls.length; space++) {
      const el = this.spaceEls[space];
      const property = state.properties[space];
      el.classList.remove(
        ...Array.from(el.classList).filter((c) => c.startsWith("owned-") || c === "mortgaged"),
      );
      if (property.owner !== null) {
        el.classList.add(`owned-${property.owner}`);
      }
      el.classList.toggle("mortgaged", property.mortgaged);
      this.houseEls[space].textContent =
        property.houses === 0 ? "" : property.houses === 5 ? "🏨" : "🏠".repeat(property.houses);

      if (property.houses !== this.lastHouses[space]) {
        this.pulse(this.houseEls[space], "houses-changed");
      }
      if (property.owner !== this.lastOwner[space]) {
        this.pulse(el, "ownership-changed");
      }
      this.lastOwner[space] = property.owner;
      this.lastHouses[space] = property.houses;
    }

    for (const tokens of this.tokenContainers) tokens.innerHTML = "";
    state.players.forEach((player, index) => {
      if (player.bankrupt) return;
      const token = document.createElement("div");
      token.className = "token";
      token.dataset.player = String(index);
      token.style.background = `var(--p${index})`;
      token.title = player.name;
      this.tokenContainers[player.position].appendChild(token);
    });
  }

  /** Moves a single player's token immediately, ahead of the next full
   * `renderState()` - the only per-event visual the board does. */
  applyEvent(env: EventEnvelope): void {
    const e = env.event;
    if (e.type === "Move") {
      this.moveToken(env.player, e.payload.to);
    } else if (e.type === "JailEntered") {
      // Sent-to-jail changes position without a `Move` event (see
      // docs/analysis-and-metrics.md's landing_counts caveat) - the board
      // has to know this exception too, to animate it at all.
      this.moveToken(env.player, JAIL_SPACE);
    }
  }

  /** Re-parents the token into its new space and animates the move with a
   * FLIP transform (capture the old position, move, then transition back
   * from an inverse transform to identity) rather than an instant snap -
   * plain CSS transitions don't tween a DOM reparent by themselves, since
   * the browser never treats the old and new parents as one continuous
   * layout. Degrades gracefully to an instant snap at very high playback
   * speeds, where a later call simply overwrites an already-in-flight
   * transition before a frame paints. */
  private moveToken(player: number, space: number): void {
    const token = document.querySelector<HTMLElement>(`.token[data-player="${player}"]`);
    // Not found before the first `renderState()` creates the tokens - the
    // next full sync will place it correctly, so there's nothing to do here.
    if (!token) return;

    const from = token.getBoundingClientRect();
    this.tokenContainers[space].appendChild(token);
    const to = token.getBoundingClientRect();
    const dx = from.left - to.left;
    const dy = from.top - to.top;
    if (dx === 0 && dy === 0) return;

    token.style.transition = "none";
    token.style.transform = `translate(${dx}px, ${dy}px)`;
    token.getBoundingClientRect(); // forces a reflow so the line above takes effect before the next one
    token.style.transition = "transform 250ms ease-out";
    token.style.transform = "";

    this.pulse(this.spaceEls[space], "arrived");
  }

  /** Re-triggers a one-shot CSS animation class - forces a reflow after
   * removing it so a space that changes twice in quick succession (e.g. two
   * tokens landing there back to back) restarts the animation instead of a
   * no-op class toggle, then clears it once the animation's own duration
   * has elapsed. */
  private pulse(el: HTMLElement, className: string): void {
    el.classList.remove(className);
    void el.offsetWidth;
    el.classList.add(className);
    setTimeout(() => el.classList.remove(className), 700);
  }
}

export function colorForGroup(group: string): string {
  const colors: Record<string, string> = {
    Brown: "#955436",
    LightBlue: "#aae0fa",
    Pink: "#d93a96",
    Orange: "#f7941d",
    Red: "#ed1b24",
    Yellow: "#fef200",
    Green: "#1fb25a",
    DarkBlue: "#0072bb",
  };
  return colors[group] ?? "transparent";
}
