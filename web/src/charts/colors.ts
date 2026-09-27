// Mirrors style.css's --p0..--p7 palette (its light-mode values - Chart.js
// canvases don't switch with `prefers-color-scheme` today, so a fixed set is
// the honest choice here rather than one that'd only match half the time).
// Duplicated here (rather than read via getComputedStyle) because Canvas 2D
// - what Chart.js draws with - does not resolve CSS custom properties,
// unlike ordinary DOM styling.
const PALETTE = [
  "hsl(355, 72%, 56%)",
  "hsl(23, 82%, 54%)",
  "hsl(48, 88%, 47%)",
  "hsl(142, 55%, 40%)",
  "hsl(189, 70%, 38%)",
  "hsl(213, 78%, 56%)",
  "hsl(263, 60%, 60%)",
  "hsl(328, 65%, 54%)",
];

export function colorForIndex(index: number): string {
  return PALETTE[index % PALETTE.length];
}
