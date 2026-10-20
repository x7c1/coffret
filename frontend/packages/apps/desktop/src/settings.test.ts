import { describe, expect, it } from 'vitest';

import { DEFAULT_IDLE_MINUTES, DEV_IDENTIFIER, IDENTIFIER, identifierFor, idleMinutes } from './settings.js';

describe('idleMinutes', () => {
  it('is the server’s default where nothing sets it', () => {
    expect(idleMinutes(undefined)).toBe(DEFAULT_IDLE_MINUTES);
    expect(idleMinutes('')).toBe(30n);
  });

  it('takes a whole number of at least one', () => {
    expect(idleMinutes('1')).toBe(1n);
    expect(idleMinutes('90')).toBe(90n);
    expect(idleMinutes('18446744073709551615')).toBe(18446744073709551615n);
  });

  it('refuses anything else, naming the variable', () => {
    for (const refused of ['0', '-1', '1.5', 'thirty', ' 5', '5 ', '1e3', '0x10', '18446744073709551616']) {
      expect(() => idleMinutes(refused), refused).toThrow(/COFFRET_IDLE_MINUTES/);
    }
  });

  it('says why in the addon’s words', () => {
    expect(() => idleMinutes('0')).toThrow('COFFRET_IDLE_MINUTES must be at least 1, not 0');
    expect(() => idleMinutes('abc')).toThrow('COFFRET_IDLE_MINUTES is not a whole number of minutes: "abc"');
  });
});

describe('identifierFor', () => {
  it('is the installed app’s from a package', () => {
    expect(identifierFor(true, {})).toBe(IDENTIFIER);
  });

  it('is the development app’s from a checkout with a state directory', () => {
    expect(identifierFor(false, { COFFRET_STATE_DIR: '/tmp/coffret-dev' })).toBe(DEV_IDENTIFIER);
  });

  it('refuses a run from a checkout that would reach the default state directory', () => {
    expect(() => identifierFor(false, {})).toThrow(/COFFRET_STATE_DIR is not set/);
    expect(() => identifierFor(false, { COFFRET_STATE_DIR: '' })).toThrow(/COFFRET_STATE_DIR is not set/);
  });
});
