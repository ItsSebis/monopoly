/** Buckets a flat array of numbers into `bucketCount` equal-width ranges for
 * a histogram (game length, bankruptcy turn, ...). Pure so it's unit-testable
 * without Chart.js/a DOM - `docs/analysis-and-metrics.md`'s distributions are
 * exactly this shape once Chart.js has no native histogram type. */
export function bucket(values: number[], bucketCount = 10): { labels: string[]; counts: number[] } {
  if (values.length === 0) return { labels: [], counts: [] };

  const min = Math.min(...values);
  const max = Math.max(...values);
  if (min === max) {
    return { labels: [String(min)], counts: [values.length] };
  }

  const width = (max - min) / bucketCount;
  const counts = new Array(bucketCount).fill(0);
  for (const value of values) {
    const index = Math.min(bucketCount - 1, Math.floor((value - min) / width));
    counts[index]++;
  }
  const labels = counts.map((_, i) => {
    const from = Math.round(min + i * width);
    const to = Math.round(min + (i + 1) * width);
    return `${from}-${to}`;
  });
  return { labels, counts };
}
