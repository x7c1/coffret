import { expect, it } from 'vitest';

import { ShownAppearance } from './ShownAppearance';

/** A system whose choice a case sets, and says it changed as a browser does. */
function fakeSystem(dark: boolean) {
  const listeners = new Set<() => void>();
  return {
    matches: dark,
    addEventListener: (_type: string, listener: () => void) => listeners.add(listener),
    removeEventListener: (_type: string, listener: () => void) => listeners.delete(listener),
    turn(toDark: boolean) {
      this.matches = toDark;
      for (const listener of listeners) {
        listener();
      }
    },
  } as unknown as MediaQueryList & { turn: (toDark: boolean) => void };
}

// Started on System, the page follows the system as it switches, live; once
// a set is chosen over it, the system's switching no longer moves it.
it('follows the system live while on System, and not once a set is chosen', () => {
  const system = fakeSystem(false);
  const root = document.createElement('div');
  const shown = new ShownAppearance(system, root);
  const stop = shown.start();
  expect(root.dataset.scheme).toBe('light');

  system.turn(true);
  expect(root.dataset.scheme).toBe('dark');

  shown.choose('light');
  expect(root.dataset.scheme).toBe('light');
  system.turn(false);
  system.turn(true);
  expect(root.dataset.scheme).toBe('light');

  shown.choose('system');
  expect(root.dataset.scheme).toBe('dark');
  stop();
  system.turn(false);
  expect(root.dataset.scheme).toBe('dark');
});

// What the server kept arrives after the page is drawn: taken up — unless the
// person chose on this page since, which is the newer word.
it('takes up what was kept, unless the person chose on this page first', () => {
  const kept = new ShownAppearance(fakeSystem(false), document.createElement('div'));
  kept.adopt('dark');
  expect(kept.appearance).toBe('dark');

  const chosen = new ShownAppearance(fakeSystem(false), document.createElement('div'));
  chosen.choose('light');
  chosen.adopt('dark');
  expect(chosen.appearance).toBe('light');
});

// A browser that cannot say what the system chose is read as light.
it('reads a system it cannot ask as light', () => {
  const root = document.createElement('div');
  new ShownAppearance(null, root).start();
  expect(root.dataset.scheme).toBe('light');
});

// LA-38: the host writes the set this device chose onto the page it serves, so
// the explorer starts in it and its first paint keeps what the browser's first
// frame was drawn in, whatever the system says; a page served for System, or
// with a word this page does not know, starts on System.
it('starts in the set the host served the page in', () => {
  const served = (appearance: string | null) => {
    const root = document.createElement('div');
    if (appearance !== null) {
      root.dataset.appearance = appearance;
    }
    const shown = new ShownAppearance(fakeSystem(false), root);
    shown.start();
    return { appearance: shown.appearance, scheme: root.dataset.scheme };
  };
  expect(served('dark')).toEqual({ appearance: 'dark', scheme: 'dark' });
  expect(served('light')).toEqual({ appearance: 'light', scheme: 'light' });
  expect(served(null)).toEqual({ appearance: 'system', scheme: 'light' });
  expect(served('sepia')).toEqual({ appearance: 'system', scheme: 'light' });

  // What was kept, arriving later, is still taken up over it.
  const root = document.createElement('div');
  root.dataset.appearance = 'dark';
  const shown = new ShownAppearance(fakeSystem(false), root);
  shown.start();
  shown.adopt('system');
  expect(root.dataset.scheme).toBe('light');
});
