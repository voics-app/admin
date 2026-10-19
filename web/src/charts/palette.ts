/** Categorical slots in the validated fixed order. The CSS variables swap per theme (see tokens.css). */
export const SERIES_SLOTS = 8;

export const seriesColor = (slot: number): string => `var(--series-${(slot % SERIES_SLOTS) + 1})`;

/** Entities keep their slot wherever they appear, so a filter never repaints the survivors. */
export const DELIVERY_SLOT = { standard: 0, p2p: 1, ephemeral: 2 } as const;

/**
 * Assigns slots to labels in sorted order and folds everything past the cap into "Other"
 * (a ninth hue is never generated).
 */
export function assignSlots(labels: readonly string[], cap = SERIES_SLOTS): { label: string; slot: number }[] {
  const sorted = [...labels].sort((a, b) => a.localeCompare(b));
  return sorted.slice(0, cap).map((label, slot) => ({ label, slot }));
}

export function foldOverflow<T extends { label: string }>(items: readonly T[], cap = SERIES_SLOTS): { kept: T[]; folded: number } {
  const sorted = [...items].sort((a, b) => a.label.localeCompare(b.label));
  return { kept: sorted.slice(0, cap), folded: Math.max(0, sorted.length - cap) };
}
