import type { MenuItemConstructorOptions } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import { menuTemplate, RELOAD_THE_EXPLORER } from './menu.js';

function items(template: MenuItemConstructorOptions[]): MenuItemConstructorOptions[] {
  return template.flatMap((item) => [item, ...(Array.isArray(item.submenu) ? items(item.submenu) : [])]);
}

describe.each(['darwin', 'linux'] as const)('the menu on %s', (platform) => {
  it('reloads the explorer from a fresh ticket on Cmd/Ctrl+R', () => {
    const reload = vi.fn();
    const item = items(menuTemplate(platform, reload)).find((each) => each.label === RELOAD_THE_EXPLORER);
    expect(item?.accelerator).toBe('CmdOrCtrl+R');
    (item?.click as () => void)();
    expect(reload).toHaveBeenCalledOnce();
  });

  it('offers no plain reload and no developer tools', () => {
    const roles = items(menuTemplate(platform, vi.fn())).map((each) => each.role);
    for (const role of ['reload', 'forceReload', 'toggleDevTools', 'viewMenu']) {
      expect(roles).not.toContain(role);
    }
  });
});

describe('Quit', () => {
  it('is the app menu’s on macOS, which carries Cmd+Q', () => {
    expect(menuTemplate('darwin', vi.fn()).map((each) => each.role)).toContain('appMenu');
  });

  it.each(['linux', 'win32'] as const)('is a File menu’s on %s, on Ctrl+Q', (platform) => {
    const quit = items(menuTemplate(platform, vi.fn())).find((each) => each.role === 'quit');
    expect(quit?.accelerator).toBe('Ctrl+Q');
  });
});
