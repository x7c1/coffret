import { describe, expect, it, vi } from 'vitest';

import {
  APPEARANCE_CHANGED,
  appearanceChangedListener,
  followAppearance,
  type NativeTheme,
  themeSourceOf,
} from './appearance.js';
import { TAKE_DROP } from './drops.js';
import { FRESH_TICKET, SESSION_ENDED } from './renewal.js';
import { groundOf, GROUND, type Appearance } from './windows.js';

describe("the windows' chrome, drawn in the explorer's appearance", () => {
  it('maps each appearance to the theme source of the same scheme', () => {
    expect(themeSourceOf('system')).toBe('system');
    expect(themeSourceOf('light')).toBe('light');
    expect(themeSourceOf('dark')).toBe('dark');
  });

  it('sets the theme source from what is read, and answers with it', () => {
    for (const appearance of ['system', 'light', 'dark'] as const) {
      const theme: NativeTheme = { themeSource: 'system' };
      const warn = vi.fn();
      expect(followAppearance(() => appearance, theme, warn)).toBe(appearance);
      expect(theme.themeSource).toBe(appearance);
      expect(warn).not.toHaveBeenCalled();
    }
  });

  it('follows the system where the read fails, and says so', () => {
    const theme: NativeTheme = { themeSource: 'dark' };
    const warn = vi.fn();
    const answered = followAppearance(
      () => {
        throw new Error('no Library is open');
      },
      theme,
      warn,
    );
    expect(answered).toBe('system');
    expect(theme.themeSource).toBe('system');
    expect(warn).toHaveBeenCalledOnce();
    expect(warn.mock.calls[0]?.[0]).toContain('no Library is open');
  });

  it("picks the ground for System by the system's own choice, not one an earlier Dark forced", () => {
    // Electron's `shouldUseDarkColors` follows the theme source where it is
    // forced, and the system where it is `system`.
    const systemDark = false;
    const theme = {
      themeSource: 'dark' as NativeTheme['themeSource'],
      get shouldUseDarkColors() {
        return this.themeSource === 'system' ? systemDark : this.themeSource === 'dark';
      },
    };
    const appearance = followAppearance(() => 'system', theme, vi.fn());
    expect(groundOf(appearance, theme.shouldUseDarkColors)).toBe(GROUND.light);
  });
});

describe('the explorer page saying the server took a new appearance', () => {
  const EXPLORER = { id: 'explorer' };
  const ORIGIN = 'http://127.0.0.1:41000';

  /** The listener as `main.ts` wires it: on the message, the appearance is read afresh. */
  function hearing(kept: () => Appearance) {
    const theme: NativeTheme = { themeSource: 'system' };
    const read = vi.fn(kept);
    const warn = vi.fn();
    const listener = appearanceChangedListener(
      () => EXPLORER,
      () => ORIGIN,
      () => {
        followAppearance(read, theme, warn);
      },
      warn,
    );
    return { listener, theme, read, warn };
  }

  it('reads the appearance afresh from the explorer page, sets the theme source from that read, and answers nothing', () => {
    let kept: Appearance = 'dark';
    const { listener, theme, read } = hearing(() => kept);
    expect(listener({ sender: EXPLORER, senderFrame: { url: `${ORIGIN}/` } })).toBeUndefined();
    expect(read).toHaveBeenCalledOnce();
    expect(theme.themeSource).toBe('dark');

    kept = 'light';
    listener({ sender: EXPLORER, senderFrame: { url: `${ORIGIN}/folder` } });
    expect(read).toHaveBeenCalledTimes(2);
    expect(theme.themeSource).toBe('light');
  });

  it('is let be from another window or another origin, and reads nothing', () => {
    const { listener, theme, read, warn } = hearing(() => 'dark');
    listener({ sender: { id: 'passphrase' }, senderFrame: { url: `${ORIGIN}/` } });
    listener({ sender: EXPLORER, senderFrame: { url: 'http://127.0.0.1:9/' } });
    listener({ sender: EXPLORER, senderFrame: null });
    expect(read).not.toHaveBeenCalled();
    expect(theme.themeSource).toBe('system');
    expect(warn).toHaveBeenCalledTimes(3);
  });

  it('is let be while there is no explorer window', () => {
    const changed = vi.fn();
    const warn = vi.fn();
    const listener = appearanceChangedListener(
      () => null,
      () => ORIGIN,
      changed,
      warn,
    );
    listener({ sender: EXPLORER, senderFrame: { url: `${ORIGIN}/` } });
    expect(changed).not.toHaveBeenCalled();
    expect(warn).toHaveBeenCalledOnce();
  });

  it('has a channel of its own', () => {
    expect(new Set([APPEARANCE_CHANGED, FRESH_TICKET, SESSION_ENDED, TAKE_DROP]).size).toBe(4);
  });
});
