import type { MenuItemConstructorOptions } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import { OPEN_THE_EXPLORER, QUIT_COFFRET, trayMenuTemplate, UNLOCK } from './tray.js';

function actions() {
  return { openExplorer: vi.fn(), unlock: vi.fn(), quit: vi.fn() };
}

function item(template: MenuItemConstructorOptions[], label: string): MenuItemConstructorOptions {
  const found = template.find((each) => each.label === label);
  if (found === undefined) {
    throw new Error(`no ${label}`);
  }
  return found;
}

function click(entry: MenuItemConstructorOptions): void {
  (entry.click as () => void)();
}

describe.each([
  ['unlocked', false],
  ['locked', true],
] as const)('the tray menu while the Library is %s', (_state, locked) => {
  it('is Open the explorer, Unlock…, a separator and Quit Coffret, in that order', () => {
    const template = trayMenuTemplate(locked, actions());
    expect(template.map((each) => each.type ?? each.label)).toEqual([
      OPEN_THE_EXPLORER,
      UNLOCK,
      'separator',
      QUIT_COFFRET,
    ]);
  });

  it(`offers Unlock… ${locked ? 'enabled' : 'disabled'}`, () => {
    const done = actions();
    const unlock = item(trayMenuTemplate(locked, done), UNLOCK);
    expect(unlock.enabled).toBe(locked);
    click(unlock);
    expect(done.unlock).toHaveBeenCalledOnce();
  });

  it('opens the explorer from Open the explorer', () => {
    const done = actions();
    click(item(trayMenuTemplate(locked, done), OPEN_THE_EXPLORER));
    expect(done.openExplorer).toHaveBeenCalledOnce();
    expect(done.quit).not.toHaveBeenCalled();
  });

  it('quits from Quit Coffret', () => {
    const done = actions();
    click(item(trayMenuTemplate(locked, done), QUIT_COFFRET));
    expect(done.quit).toHaveBeenCalledOnce();
    expect(done.openExplorer).not.toHaveBeenCalled();
  });
});
