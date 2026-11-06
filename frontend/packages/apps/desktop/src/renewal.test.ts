import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  FRESH_TICKET,
  RENEW_AHEAD_MS,
  Renewal,
  RENEWAL_CHECK_MS,
  RENEWAL_RETRY_MS,
  SESSION_ENDED,
  sessionEndedListener,
  TOKEN_LIFETIME_MS,
} from './renewal.js';

/** The explorer's window, as the renewal reaches it: shown, hidden, or destroyed. */
function setup() {
  let tickets = 0;
  let window: 'shown' | 'hidden' | 'destroyed' = 'shown';
  const delivered: string[] = [];
  const target = {
    issueTicket: vi.fn(() => `ticket-${++tickets}`),
    deliver: vi.fn((ticket: string) => {
      if (window === 'destroyed') {
        return false;
      }
      delivered.push(ticket);
      return true;
    }),
    warn: vi.fn(),
  };
  const renewal = new Renewal(target);
  return {
    renewal,
    target,
    delivered,
    window(now: 'shown' | 'hidden' | 'destroyed') {
      window = now;
    },
  };
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

// LA-17, LA-18: ahead of a token's expiry, the main process issues a fresh
// ticket on its own timer and hands it to the explorer's page; nothing the
// page does makes it issue one.
describe('the renewal of the explorer session', () => {
  it('hands a fresh ticket over ahead of the expiry, on its own timer, and again after each', async () => {
    const { renewal, target, delivered } = setup();
    renewal.start();
    const loadedAt = Date.now();
    renewal.loaded();

    await vi.advanceTimersByTimeAsync(TOKEN_LIFETIME_MS - RENEW_AHEAD_MS - RENEWAL_CHECK_MS);
    expect(target.issueTicket).not.toHaveBeenCalled();
    expect(delivered).toEqual([]);

    await vi.advanceTimersByTimeAsync(RENEWAL_CHECK_MS);
    expect(delivered).toEqual(['ticket-1']);
    // Ahead of the expiry: the token the load's ticket was exchanged for still
    // has the margin left.
    expect(Date.now() - loadedAt).toBe(TOKEN_LIFETIME_MS - RENEW_AHEAD_MS);

    await vi.advanceTimersByTimeAsync(TOKEN_LIFETIME_MS - RENEW_AHEAD_MS);
    expect(delivered).toEqual(['ticket-1', 'ticket-2']);
    renewal.stop();
  });

  it('hands nothing over before a window was loaded', async () => {
    const { renewal, target } = setup();
    renewal.start();
    await vi.advanceTimersByTimeAsync(2 * TOKEN_LIFETIME_MS);
    expect(target.issueTicket).not.toHaveBeenCalled();
    renewal.stop();
  });

  it('starts the count again from every load', async () => {
    const { renewal, delivered } = setup();
    renewal.start();
    renewal.loaded();
    await vi.advanceTimersByTimeAsync(TOKEN_LIFETIME_MS / 2);
    renewal.loaded();
    await vi.advanceTimersByTimeAsync(TOKEN_LIFETIME_MS / 2);
    expect(delivered).toEqual([]);
    await vi.advanceTimersByTimeAsync(TOKEN_LIFETIME_MS / 2 - RENEW_AHEAD_MS);
    expect(delivered).toEqual(['ticket-1']);
    renewal.stop();
  });

  it('hands the ticket over to a hidden window as to a shown one', async () => {
    const { renewal, delivered, window } = setup();
    renewal.start();
    renewal.loaded();
    window('hidden');
    await vi.advanceTimersByTimeAsync(TOKEN_LIFETIME_MS - RENEW_AHEAD_MS);
    expect(delivered).toEqual(['ticket-1']);
    renewal.stop();
  });

  it('hands a ticket over as soon as a device that slept through it wakes', () => {
    const { renewal, delivered } = setup();
    renewal.loaded();
    // The wall clock moved on while no timer ran.
    vi.setSystemTime(Date.now() + TOKEN_LIFETIME_MS);
    renewal.tick();
    expect(delivered).toEqual(['ticket-1']);
  });

  it('delivers nothing to a destroyed window, and waits for the next load', async () => {
    const { renewal, target, delivered, window } = setup();
    renewal.start();
    renewal.loaded();
    window('destroyed');
    await vi.advanceTimersByTimeAsync(TOKEN_LIFETIME_MS - RENEW_AHEAD_MS);
    expect(delivered).toEqual([]);
    expect(renewal.dueAt()).toBeNull();
    target.issueTicket.mockClear();
    await vi.advanceTimersByTimeAsync(TOKEN_LIFETIME_MS);
    expect(target.issueTicket).not.toHaveBeenCalled();

    window('shown');
    renewal.loaded();
    await vi.advanceTimersByTimeAsync(TOKEN_LIFETIME_MS - RENEW_AHEAD_MS);
    expect(delivered).toEqual(['ticket-2']);
    renewal.stop();
  });

  it('says where no ticket could be issued, without the ticket, and tries again later', async () => {
    const { renewal, target, delivered } = setup();
    renewal.start();
    renewal.loaded();
    target.issueTicket.mockImplementationOnce(() => {
      throw new Error('no Library is open');
    });
    await vi.advanceTimersByTimeAsync(TOKEN_LIFETIME_MS - RENEW_AHEAD_MS);
    expect(target.warn).toHaveBeenCalledWith(expect.stringContaining('no Library is open'));
    expect(delivered).toEqual([]);
    await vi.advanceTimersByTimeAsync(RENEWAL_RETRY_MS);
    expect(delivered).toEqual(['ticket-1']);
    for (const [said] of target.warn.mock.calls) {
      expect(said).not.toContain('ticket-1');
    }
    renewal.stop();
  });

  it('hands nothing over once stopped', async () => {
    const { renewal, target } = setup();
    renewal.start();
    renewal.loaded();
    renewal.stop();
    await vi.advanceTimersByTimeAsync(2 * TOKEN_LIFETIME_MS);
    expect(target.issueTicket).not.toHaveBeenCalled();
  });

  it('keeps the channels distinct from the drop', () => {
    expect(new Set([FRESH_TICKET, SESSION_ENDED, 'coffret:take-drop']).size).toBe(3);
  });
});

describe('the explorer page saying its session ended', () => {
  const EXPLORER = { id: 'explorer' };
  const ORIGIN = 'http://127.0.0.1:41000';

  function hearing() {
    const ended = vi.fn();
    const warn = vi.fn();
    const listener = sessionEndedListener(
      () => EXPLORER,
      () => ORIGIN,
      ended,
      warn,
    );
    return { listener, ended, warn };
  }

  it('is heard from the explorer page, and answers nothing', () => {
    const { listener, ended } = hearing();
    expect(listener({ sender: EXPLORER, senderFrame: { url: `${ORIGIN}/` } })).toBeUndefined();
    expect(ended).toHaveBeenCalledOnce();
  });

  it('is let be from another window or another origin', () => {
    const { listener, ended, warn } = hearing();
    listener({ sender: { id: 'passphrase' }, senderFrame: { url: `${ORIGIN}/` } });
    listener({ sender: EXPLORER, senderFrame: { url: 'http://127.0.0.1:9/' } });
    listener({ sender: EXPLORER, senderFrame: null });
    expect(ended).not.toHaveBeenCalled();
    expect(warn).toHaveBeenCalledTimes(3);
  });
});
