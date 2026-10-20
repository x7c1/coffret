import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { DoorMode, ServedLibrary } from './addon.js';
import { Desktop, type Closing } from './desktop.js';

/** The windows and the addon, as the dispatcher sees them, recording what it asks. */
function setup() {
  let served: ServedLibrary | null = null;
  let visible = false;
  let tokens = 0;
  const loaded: string[] = [];
  const core = {
    openDoor: vi.fn((mode: DoorMode) => `http://127.0.0.1:9/#door=${mode}-${++tokens}`),
    served: vi.fn(() => served),
  };
  const passphrase = {
    load: vi.fn((url: string) => {
      loaded.push(url);
    }),
    bringForward: vi.fn(() => {
      visible = true;
    }),
    hide: vi.fn(() => {
      visible = false;
    }),
    isVisible: vi.fn(() => visible),
  };
  let explorerOpen = false;
  const shell = {
    openExplorer: vi.fn(() => {
      explorerOpen = true;
    }),
    focusExplorer: vi.fn(() => explorerOpen),
    reportServerStopped: vi.fn(),
    quit: vi.fn(),
    warn: vi.fn(),
  };
  const desktop = new Desktop(core, passphrase, shell);
  return {
    desktop,
    core,
    passphrase,
    shell,
    loaded,
    serve(library: ServedLibrary | null) {
      served = library;
    },
  };
}

function closing(): Closing & { prevented: () => boolean } {
  const preventDefault = vi.fn();
  return { preventDefault, prevented: () => preventDefault.mock.calls.length > 0 };
}

const LOCKED = { name: 'main', unlocked: false };
const UNLOCKED = { name: 'main', unlocked: true };

let fixture: ReturnType<typeof setup>;

beforeEach(() => {
  fixture = setup();
  fixture.desktop.open();
});

describe('as it starts', () => {
  it('opens the Passphrase window in open mode, in front', () => {
    expect(fixture.core.openDoor).toHaveBeenCalledWith('Open');
    expect(fixture.loaded).toEqual(['http://127.0.0.1:9/#door=Open-1']);
    expect(fixture.passphrase.bringForward).toHaveBeenCalledOnce();
  });
});

describe('LibraryOpened', () => {
  it('with the Passphrase window open in open mode hides it and opens the explorer', () => {
    fixture.desktop.handle('LibraryOpened');
    expect(fixture.passphrase.hide).toHaveBeenCalledOnce();
    expect(fixture.passphrase.isVisible()).toBe(false);
    expect(fixture.shell.openExplorer).toHaveBeenCalledOnce();
  });

  it('a second time opens no second explorer', () => {
    fixture.desktop.handle('LibraryOpened');
    fixture.desktop.handle('LibraryOpened');
    expect(fixture.shell.openExplorer).toHaveBeenCalledOnce();
    expect(fixture.shell.warn).toHaveBeenCalledOnce();
  });
});

describe('UnlockRequested', () => {
  beforeEach(() => {
    fixture.desktop.handle('LibraryOpened');
    fixture.serve(LOCKED);
    fixture.passphrase.bringForward.mockClear();
  });

  it('with the window hidden loads it afresh in unlock mode and brings it forward', () => {
    fixture.desktop.handle('UnlockRequested');
    expect(fixture.core.openDoor).toHaveBeenLastCalledWith('Unlock');
    expect(fixture.loaded).toEqual(['http://127.0.0.1:9/#door=Open-1', 'http://127.0.0.1:9/#door=Unlock-2']);
    expect(fixture.passphrase.bringForward).toHaveBeenCalledOnce();
    expect(fixture.passphrase.isVisible()).toBe(true);
  });

  it('with the window already showing unlock mode only brings it forward, without a reload', () => {
    fixture.desktop.handle('UnlockRequested');
    fixture.desktop.handle('UnlockRequested');
    expect(fixture.core.openDoor).toHaveBeenCalledTimes(2);
    expect(fixture.passphrase.load).toHaveBeenCalledTimes(2);
    expect(fixture.passphrase.bringForward).toHaveBeenCalledTimes(2);
  });

  it('with the window hidden again after an unlock loads it afresh', () => {
    fixture.desktop.handle('UnlockRequested');
    fixture.desktop.handle('LibraryUnlocked');
    fixture.desktop.handle('UnlockRequested');
    expect(fixture.loaded.at(-1)).toBe('http://127.0.0.1:9/#door=Unlock-3');
  });

  it('asks nothing where the Library is already unlocked', () => {
    fixture.serve(UNLOCKED);
    fixture.desktop.handle('UnlockRequested');
    expect(fixture.passphrase.load).toHaveBeenCalledOnce();
    expect(fixture.passphrase.bringForward).not.toHaveBeenCalled();
  });

  it('asks nothing where no Library is served any more', () => {
    fixture.serve(null);
    fixture.desktop.handle('UnlockRequested');
    expect(fixture.passphrase.load).toHaveBeenCalledOnce();
    expect(fixture.shell.warn).toHaveBeenCalledOnce();
  });

  it('says where the door could not be opened, and brings nothing forward', () => {
    fixture.core.openDoor.mockImplementationOnce(() => {
      throw new Error('the desktop has not started');
    });
    fixture.desktop.handle('UnlockRequested');
    expect(fixture.shell.warn).toHaveBeenCalledWith(expect.stringContaining('the desktop has not started'));
    expect(fixture.passphrase.bringForward).not.toHaveBeenCalled();
  });
});

describe('UnlockRequested before a Library is served', () => {
  it('leaves the open-mode window as it is', () => {
    fixture.serve(LOCKED);
    fixture.desktop.handle('UnlockRequested');
    expect(fixture.passphrase.load).toHaveBeenCalledOnce();
    expect(fixture.shell.warn).toHaveBeenCalledOnce();
  });
});

describe('LibraryUnlocked', () => {
  it('hides the Passphrase window and brings the explorer forward', () => {
    fixture.desktop.handle('LibraryOpened');
    fixture.serve(LOCKED);
    fixture.desktop.handle('UnlockRequested');
    fixture.desktop.handle('LibraryUnlocked');
    expect(fixture.passphrase.isVisible()).toBe(false);
    expect(fixture.shell.focusExplorer).toHaveBeenCalledOnce();
    expect(fixture.shell.quit).not.toHaveBeenCalled();
  });

  it('with the Passphrase window hidden already brings nothing forward', () => {
    fixture.desktop.handle('LibraryOpened');
    fixture.desktop.handle('LibraryUnlocked');
    expect(fixture.passphrase.isVisible()).toBe(false);
    expect(fixture.shell.focusExplorer).not.toHaveBeenCalled();
  });
});

describe('ServerStopped', () => {
  it('before a Library is served says so and quits', () => {
    fixture.desktop.handle('ServerStopped');
    expect(fixture.shell.reportServerStopped).toHaveBeenCalledOnce();
    expect(fixture.shell.quit).toHaveBeenCalledOnce();
    expect(fixture.shell.reportServerStopped.mock.invocationCallOrder[0]).toBeLessThan(
      fixture.shell.quit.mock.invocationCallOrder[0] ?? 0,
    );
  });

  it('after a Library is served says so and quits', () => {
    fixture.desktop.handle('LibraryOpened');
    fixture.desktop.handle('ServerStopped');
    expect(fixture.shell.reportServerStopped).toHaveBeenCalledOnce();
    expect(fixture.shell.quit).toHaveBeenCalledOnce();
  });
});

describe('closing the Passphrase window', () => {
  it('before a Library is served quits the app', () => {
    const event = closing();
    fixture.desktop.passphraseClosing(event);
    expect(event.prevented()).toBe(false);
    expect(fixture.shell.quit).toHaveBeenCalledOnce();
  });

  it('after a Library is served only hides it', () => {
    fixture.desktop.handle('LibraryOpened');
    fixture.serve(LOCKED);
    fixture.desktop.handle('UnlockRequested');
    const event = closing();
    fixture.desktop.passphraseClosing(event);
    expect(event.prevented()).toBe(true);
    expect(fixture.passphrase.isVisible()).toBe(false);
    expect(fixture.shell.quit).not.toHaveBeenCalled();
  });

  it('while the app quits lets it close', () => {
    fixture.desktop.handle('LibraryOpened');
    fixture.desktop.quit();
    const event = closing();
    fixture.desktop.passphraseClosing(event);
    expect(event.prevented()).toBe(false);
    expect(fixture.shell.quit).not.toHaveBeenCalled();
  });
});

describe('a second launch', () => {
  it('brings the Passphrase window forward while it is showing', () => {
    fixture.desktop.bringForward();
    expect(fixture.passphrase.bringForward).toHaveBeenCalledTimes(2);
    expect(fixture.shell.focusExplorer).not.toHaveBeenCalled();
  });

  it('brings the explorer forward once the Passphrase window is hidden', () => {
    fixture.desktop.handle('LibraryOpened');
    fixture.desktop.bringForward();
    expect(fixture.shell.focusExplorer).toHaveBeenCalledOnce();
    expect(fixture.passphrase.bringForward).toHaveBeenCalledOnce();
  });
});
