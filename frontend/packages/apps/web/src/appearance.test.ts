import { expect, it } from 'vitest';

import { paintScheme, schemeOf } from './appearance';
import { colorProperty, DARK, LIGHT } from './theme';

// `System` is the system's choice, either way; `Light` and `Dark` are what
// they say whatever the system's is.
it('resolves the choice to a set, the system read only for System', () => {
  expect(schemeOf('system', false)).toBe('light');
  expect(schemeOf('system', true)).toBe('dark');
  for (const systemDark of [false, true]) {
    expect(schemeOf('light', systemDark)).toBe('light');
    expect(schemeOf('dark', systemDark)).toBe('dark');
  }
});

// Every colour the components draw in, painted onto the root in the set's
// value — and the browser told which set it is.
it('paints every colour of the set onto the root', () => {
  const root = document.createElement('div');
  for (const [scheme, set] of [
    ['dark', DARK],
    ['light', LIGHT],
  ] as const) {
    paintScheme(scheme, root);
    for (const name of Object.keys(LIGHT) as (keyof typeof LIGHT)[]) {
      expect(root.style.getPropertyValue(colorProperty(name)), name).toBe(set[name]);
    }
    expect(root.style.colorScheme).toBe(scheme);
    expect(root.dataset.scheme).toBe(scheme);
  }
});
