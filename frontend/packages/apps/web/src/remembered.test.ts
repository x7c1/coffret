import { afterEach, expect, it, vi } from 'vitest';

import { recall, remember, type Storing } from './remembered';

afterEach(() => {
  window.localStorage.clear();
  vi.restoreAllMocks();
});

it('reads back what was remembered', () => {
  expect(recall('coffret.test')).toBe(false);
  remember('coffret.test', true);
  expect(recall('coffret.test')).toBe(true);
  remember('coffret.test', false);
  expect(recall('coffret.test')).toBe(false);
});

// A private window or a browser blocking site data throws on every access, and
// the drop must go on as if nothing was ever remembered.
it('falls back to the default when storage throws', () => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
  const blocked: Storing = () => {
    throw new DOMException('blocked', 'SecurityError');
  };
  const refusing: Storing = () => ({
    getItem: () => {
      throw new DOMException('blocked', 'SecurityError');
    },
    setItem: () => {
      throw new DOMException('full', 'QuotaExceededError');
    },
  });

  expect(recall('coffret.test', blocked)).toBe(false);
  expect(recall('coffret.test', refusing)).toBe(false);
  expect(() => remember('coffret.test', true, blocked)).not.toThrow();
  expect(() => remember('coffret.test', true, refusing)).not.toThrow();
  expect(console.warn).toHaveBeenCalled();
});
