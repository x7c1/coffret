import { describe, expect, it, vi } from 'vitest';

import {
  denyEveryWindow,
  explorerLaunchUrl,
  explorerOrigin,
  explorerWindowOpener,
  guardNavigation,
  staysOn,
  type Navigable,
} from './navigation.js';

const ORIGIN = 'http://127.0.0.1:41234';

describe('staysOn', () => {
  it('allows the same origin, whatever the path, query or fragment', () => {
    expect(staysOn(ORIGIN, `${ORIGIN}/`)).toBe(true);
    expect(staysOn(ORIGIN, `${ORIGIN}/books/one?page=3#top`)).toBe(true);
  });

  it('refuses other origins', () => {
    for (const other of [
      'http://127.0.0.1:41235/',
      'http://localhost:41234/',
      'https://127.0.0.1:41234/',
      'http://127.0.0.2:41234/',
      'https://accounts.google.com/',
    ]) {
      expect(staysOn(ORIGIN, other), other).toBe(false);
    }
  });

  it('refuses file:, javascript:, data: and what does not parse', () => {
    for (const other of ['file:///etc/passwd', 'javascript:alert(1)', 'data:text/html,hi', 'about:blank', 'not a url']) {
      expect(staysOn(ORIGIN, other), other).toBe(false);
    }
  });

  it('refuses everything before the origin is known', () => {
    expect(staysOn(null, `${ORIGIN}/`)).toBe(false);
  });
});

function contents() {
  const listeners = new Map<string, (event: { preventDefault(): void }, url: string) => void>();
  const navigable: Navigable = {
    on(event, listener) {
      listeners.set(event, listener);
      return navigable;
    },
  };
  const navigate = (event: string, url: string) => {
    const preventDefault = vi.fn();
    listeners.get(event)?.({ preventDefault }, url);
    return preventDefault.mock.calls.length > 0;
  };
  return { navigable, navigate };
}

describe('guardNavigation', () => {
  it('prevents navigations and redirects away from the origin, and lets the rest through', () => {
    const { navigable, navigate } = contents();
    const warn = vi.fn();
    guardNavigation(navigable, 'explorer', () => ORIGIN, warn);

    for (const event of ['will-navigate', 'will-redirect']) {
      expect(navigate(event, `${ORIGIN}/elsewhere`)).toBe(false);
      expect(navigate(event, 'https://example.com/')).toBe(true);
      expect(navigate(event, 'file:///tmp/x.html')).toBe(true);
    }
    expect(warn).toHaveBeenCalledTimes(4);
    for (const [said] of warn.mock.calls) {
      expect(said).not.toContain('example.com');
    }
  });

  it('reads the origin when asked', () => {
    const { navigable, navigate } = contents();
    let origin: string | null = null;
    guardNavigation(navigable, 'Passphrase', () => origin, vi.fn());
    expect(navigate('will-navigate', `${ORIGIN}/`)).toBe(true);
    origin = ORIGIN;
    expect(navigate('will-navigate', `${ORIGIN}/`)).toBe(false);
  });
});

describe('window opening', () => {
  it('opens nothing from the Passphrase window', () => {
    expect(denyEveryWindow()).toEqual({ action: 'deny' });
  });

  it('hands an https: address to the system browser, and opens no window', () => {
    const openExternal = vi.fn(() => Promise.resolve());
    const open = explorerWindowOpener(openExternal, vi.fn());
    const consent = 'https://accounts.google.com/o/oauth2/v2/auth?client_id=x&state=y';
    expect(open({ url: consent })).toEqual({ action: 'deny' });
    expect(openExternal).toHaveBeenCalledWith(consent);
  });

  it('denies http:, file:, custom schemes and what does not parse', () => {
    const openExternal = vi.fn(() => Promise.resolve());
    const warn = vi.fn();
    const open = explorerWindowOpener(openExternal, warn);
    for (const url of [
      'http://example.com/',
      `${ORIGIN}/`,
      'file:///etc/passwd',
      'javascript:alert(1)',
      'mailto:someone@example.com',
      'coffret://open',
      'vscode://file/x',
      'not a url',
    ]) {
      expect(open({ url }), url).toEqual({ action: 'deny' });
    }
    expect(openExternal).not.toHaveBeenCalled();
    expect(warn).toHaveBeenCalledTimes(8);
  });

  it('says a refusal of the system browser without the address', async () => {
    const warn = vi.fn();
    const open = explorerWindowOpener(() => Promise.reject(new Error('could not open https://x/?state=s')), warn);
    open({ url: 'https://x/?state=s' });
    await vi.waitFor(() => expect(warn).toHaveBeenCalledOnce());
    expect(warn.mock.calls[0]?.[0]).not.toContain('state=s');
  });
});

describe('the explorer’s address', () => {
  it('carries the ticket in the fragment, and the origin is the address’s', () => {
    expect(explorerLaunchUrl('127.0.0.1:41234', 'T1')).toBe(`${ORIGIN}/#launch=T1`);
    expect(explorerOrigin('127.0.0.1:41234')).toBe(ORIGIN);
    expect(staysOn(explorerOrigin('127.0.0.1:41234'), explorerLaunchUrl('127.0.0.1:41234', 'T1'))).toBe(true);
  });
});
