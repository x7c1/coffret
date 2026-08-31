// The reader's prefetch policy, kept free of DOM so it is unit testable.

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
