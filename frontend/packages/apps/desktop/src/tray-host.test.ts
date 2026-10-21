import { describe, expect, it, vi } from 'vitest';

import { ASK, STATUS_NOTIFIER_WATCHER, trayHostFrom, trayHostLine, trayHostOn } from './tray-host.js';

describe('the bus’s answer', () => {
  it('(true,) is a tray host', () => {
    expect(trayHostFrom({ kind: 'answered', stdout: '(true,)\n' })).toEqual({ present: true, reason: 'owned' });
  });

  it('(false,) is none', () => {
    expect(trayHostFrom({ kind: 'answered', stdout: '(false,)\n' })).toEqual({ present: false, reason: 'not-owned' });
  });

  it('a timeout is none', () => {
    expect(trayHostFrom({ kind: 'timed-out' })).toEqual({ present: false, reason: 'timed-out' });
  });

  it('a missing gdbus is none', () => {
    expect(trayHostFrom({ kind: 'missing' })).toEqual({ present: false, reason: 'no-gdbus' });
  });

  it('an error is none', () => {
    expect(trayHostFrom({ kind: 'failed' })).toEqual({ present: false, reason: 'failed' });
  });

  it('anything else printed is none', () => {
    for (const stdout of ['', 'true', '(true)', '(true, false)', '(1,)', 'Error: no bus', '(TRUE,)']) {
      expect(trayHostFrom({ kind: 'answered', stdout }), stdout).toEqual({ present: false, reason: 'unreadable' });
    }
  });
});

describe('asking', () => {
  it('names the watcher, through the session bus’s own NameHasOwner', () => {
    expect(ASK.command).toBe('gdbus');
    expect(ASK.args).toContain('--session');
    expect(ASK.args).toContain('org.freedesktop.DBus.NameHasOwner');
    expect(ASK.args.at(-1)).toBe(STATUS_NOTIFIER_WATCHER);
  });

  it('is not done on macOS or Windows, which always show a tray', async () => {
    const ask = vi.fn();
    for (const platform of ['darwin', 'win32'] as const) {
      expect((await trayHostOn(platform, ask)).present).toBe(true);
    }
    expect(ask).not.toHaveBeenCalled();
  });

  it('on Linux takes the bus’s answer', async () => {
    expect(await trayHostOn('linux', async () => ({ kind: 'answered', stdout: '(true,)' }))).toEqual({
      present: true,
      reason: 'owned',
    });
    expect((await trayHostOn('linux', async () => ({ kind: 'missing' }))).present).toBe(false);
  });

  it('on Linux takes an asking that throws as none', async () => {
    const host = await trayHostOn('linux', () => Promise.reject(new Error('spawn failed at /some/path')));
    expect(host).toEqual({ present: false, reason: 'failed' });
  });
});

describe('the log line', () => {
  it('says which way it went, in fixed words', () => {
    expect(trayHostLine({ present: true, reason: 'owned' })).toContain('hides');
    expect(trayHostLine({ present: false, reason: 'no-gdbus' })).toBe(
      "no tray host on the session bus (no-gdbus); closing the explorer's window quits",
    );
  });
});
