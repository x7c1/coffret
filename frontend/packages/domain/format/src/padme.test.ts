import { describe, expect, it } from 'vitest';

import { paddedLength } from './padme.js';

describe('Padmé', () => {
  // FM-4: Padmé rounds an unpadded length L up to the next multiple of 2^(E-S),
  // with E = floor(log2 L) and S = floor(log2 E) + 1. These are the cases the
  // Rust implementation pins, so both implementations bucket alike.
  it('pads to the expected bucket', () => {
    const cases: [bigint, bigint][] = [
      [8n, 8n], // E=3, S=2, bucket 2, already aligned
      [9n, 10n], // E=3, S=2, bucket 2
      [100n, 104n], // E=6, S=3, bucket 8
      [1_000n, 1_024n], // E=9, S=4, bucket 32
      [1_048_576n, 1_048_576n], // E=20, S=5, bucket 32768, already aligned
      [1_048_577n, 1_081_344n], // E=20, S=5, bucket 32768
    ];
    for (const [unpadded, expected] of cases) {
      expect(paddedLength(unpadded), `L = ${unpadded}`).toBe(expected);
    }
  });

  // FM-4: a stream short enough that E - S <= 0 is stored unpadded.
  it('leaves short streams unpadded', () => {
    for (let unpadded = 0n; unpadded <= 7n; unpadded += 1n) {
      expect(paddedLength(unpadded), `L = ${unpadded}`).toBe(unpadded);
    }
  });

  // FM-4: overhead is bounded at about 12%.
  it('keeps overhead under twelve percent', () => {
    for (let unpadded = 1n; unpadded <= 100_000n; unpadded += 1n) {
      const padded = paddedLength(unpadded);
      expect(padded >= unpadded, `L = ${unpadded}`).toBe(true);
      expect(padded * 100n <= unpadded * 112n, `L = ${unpadded} padded to ${padded}`).toBe(true);
    }
  });

  it('is idempotent', () => {
    for (let unpadded = 0n; unpadded <= 10_000n; unpadded += 1n) {
      const padded = paddedLength(unpadded);
      expect(paddedLength(padded), `L = ${unpadded}`).toBe(padded);
    }
  });

  // A length whose bucket boundary is not representable in a u64 is returned
  // unpadded, as the reference implementation does: the two must agree over the
  // whole domain of the field, not only over lengths a real stream reaches.
  it('leaves lengths without a representable bucket unpadded', () => {
    const nearMax = (1n << 64n) - 1n;
    expect(paddedLength(nearMax)).toBe(nearMax);
  });
});
