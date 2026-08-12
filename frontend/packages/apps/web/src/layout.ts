// Pure layout/prefetch math for the viewer, kept free of DOM so it is unit
// testable.

export interface Entry {
  id: number;
  path: string;
}

/** Number of grid columns that fit the container at the given cell width. */
export function columnCount(containerWidth: number, cellWidth: number): number {
  return Math.max(1, Math.floor(containerWidth / cellWidth));
}

/** Number of virtualized rows needed for `itemCount` items. */
export function rowCount(itemCount: number, columns: number): number {
  return Math.ceil(itemCount / columns);
}

/** Item indices belonging to one virtualized row. */
export function rowItems(row: number, columns: number, itemCount: number): number[] {
  const start = row * columns;
  const end = Math.min(start + columns, itemCount);
  const items = [];
  for (let i = start; i < end; i++) {
    items.push(i);
  }
  return items;
}

/**
 * Indices to prefetch around the current reader position, nearest first and
 * biased forward (page turns overwhelmingly go forward).
 */
export function prefetchTargets(current: number, itemCount: number, radius: number): number[] {
  const targets = [];
  for (let step = 1; step <= radius; step++) {
    const forward = current + step;
    if (forward < itemCount) {
      targets.push(forward);
    }
    const backward = current - step;
    if (backward >= 0) {
      targets.push(backward);
    }
  }
  return targets;
}
