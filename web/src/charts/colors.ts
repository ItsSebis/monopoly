// Mirrors style.css's --p0..--p7 palette. Duplicated here (rather than read
// via getComputedStyle) because Canvas 2D - what Chart.js draws with - does
// not resolve CSS custom properties, unlike ordinary DOM styling.
const PALETTE = ["#e6194b", "#3cb44b", "#4363d8", "#f58231", "#911eb4", "#46f0f0", "#f032e6", "#bcf60c"];

export function colorForIndex(index: number): string {
  return PALETTE[index % PALETTE.length];
}
