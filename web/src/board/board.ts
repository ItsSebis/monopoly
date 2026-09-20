import { BOARD_LAYOUT } from "./layout";
import type { EventEnvelope, GameState } from "../types";

const JAIL_SPACE = 10;

/** Renders the 40-space board and player tokens from a `GameState` snapshot,
 * plus a light per-event animation layer for token movement (see
 * docs/frontend.md). Ownership/house/cash display only ever comes from a
 * full `renderState()` call - `applyEvent()` never invents or derives state,
 * it only moves a token to a position the event itself already names. */
export class BoardView {
  private spaceEls: HTMLElement[] = [];
  private tokenContainers: HTMLElement[] = [];

  constructor(container: HTMLElement) {
    container.innerHTML = "";
    for (const space of BOARD_LAYOUT) {
      const el = document.createElement("div");
      el.className = "space";
      el.style.gridRow = String(space.row + 1);
      el.style.gridColumn = String(space.col + 1);
      if (space.colorGroup) {
        const bar = document.createElement("div");
        bar.className = "color-bar";
        bar.style.background = colorForGroup(space.colorGroup);
        el.appendChild(bar);
      }
      const tokens = document.createElement("div");
      tokens.className = "tokens";
      el.appendChild(tokens);
      const name = document.createElement("div");
      name.className = "name";
      name.textContent = space.name;
      el.appendChild(name);
      const houses = document.createElement("div");
      houses.className = "houses";
      el.appendChild(houses);

      container.appendChild(el);
      this.spaceEls.push(el);
      this.tokenContainers.push(tokens);
    }
  }

  /** Full re-sync to the engine's authoritative state - the ground truth,
   * called once per turn. */
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
      el.querySelector(".houses")!.textContent =
        property.houses === 0 ? "" : property.houses === 5 ? "🏨" : "🏠".repeat(property.houses);
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
   * layout. Degrades gracefully to today's instant snap at very high
   * playback speeds, where a later call simply overwrites an
   * already-in-flight transition before a frame paints. */
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
  }
}

function colorForGroup(group: string): string {
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
