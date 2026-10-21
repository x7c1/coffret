import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { DoorMode, ServedLibrary } from './addon.js';
import { Desktop, explorerCloses, type Closing, type Residency } from './desktop.js';

/** The windows and the addon, as the dispatcher sees them, recording what it asks. */
function setup(residency: Residency = { platform: 'linux', tray: true }) {
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
  // The explorer's window: none, or one shown or hidden.
  let explorer: 'none' | 'shown' | 'hidden' = 'none';
  const shell = {
    openExplorer: vi.fn(() => {
      explorer = 'shown';
    }),
    focusExplorer: vi.fn(() => {
      if (explorer === 'none') {
        return false;
      }
      explorer = 'shown';
      return true;
    }),
    hideExplorer: vi.fn(() => {
      explorer = 'hidden';
    }),
    reportServerStopped: vi.fn(),
    quit: vi.fn(),
    warn: vi.fn(),
  };
  const desktop = new Desktop(core, passphrase, shell, residency);
  return {
    desktop,
    core,
    passphrase,
    shell,
    loaded,
    serve(library: ServedLibrary | null) {
      served = library;
    },
    explorer: () => explorer,
    /** The explorer's window destroyed, as a crash or the platform may leave it. */
    destroyExplorer() {
      explorer = 'none';
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

describe('bringing the explorer back', () => {
  it('before a Library is served brings the Passphrase window forward as it is', () => {
    fixture.desktop.bringForward();
    expect(fixture.passphrase.bringForward).toHaveBeenCalledTimes(2);
    expect(fixture.passphrase.load).toHaveBeenCalledOnce();
    expect(fixture.shell.focusExplorer).not.toHaveBeenCalled();
    expect(fixture.shell.openExplorer).not.toHaveBeenCalled();
  });

  describe('once a Library is served and unlocked', () => {
    beforeEach(() => {
      fixture.desktop.handle('LibraryOpened');
      fixture.serve(UNLOCKED);
      fixture.shell.openExplorer.mockClear();
      fixture.passphrase.bringForward.mockClear();
    });

    it('with the window present and showing brings it forward, and opens no other', () => {
      fixture.desktop.bringForward();
      expect(fixture.shell.focusExplorer).toHaveBeenCalledOnce();
      expect(fixture.shell.openExplorer).not.toHaveBeenCalled();
      expect(fixture.explorer()).toBe('shown');
      expect(fixture.passphrase.bringForward).not.toHaveBeenCalled();
    });

    it('with the window hidden shows it again, and opens no other', () => {
      fixture.desktop.explorerClosing(closing());
      expect(fixture.explorer()).toBe('hidden');
      fixture.desktop.bringForward();
      expect(fixture.explorer()).toBe('shown');
      expect(fixture.shell.openExplorer).not.toHaveBeenCalled();
    });

    it('with the window destroyed opens a new one, from a fresh ticket', () => {
      fixture.destroyExplorer();
      fixture.desktop.bringForward();
      expect(fixture.shell.openExplorer).toHaveBeenCalledOnce();
      expect(fixture.explorer()).toBe('shown');
    });
  });

  describe('once the Library served has locked', () => {
    beforeEach(() => {
      fixture.desktop.handle('LibraryOpened');
      fixture.serve(LOCKED);
      fixture.passphrase.bringForward.mockClear();
    });

    it('brings the explorer and, in front of it, the Passphrase window in unlock mode', () => {
      fixture.desktop.explorerClosing(closing());
      fixture.desktop.bringForward();
      expect(fixture.explorer()).toBe('shown');
      expect(fixture.core.openDoor).toHaveBeenLastCalledWith('Unlock');
      expect(fixture.passphrase.bringForward).toHaveBeenCalledOnce();
      expect(fixture.shell.focusExplorer.mock.invocationCallOrder[0]).toBeLessThan(
        fixture.passphrase.bringForward.mock.invocationCallOrder[0] ?? 0,
      );
    });

    it('with the window destroyed opens a new one before asking for the Passphrase', () => {
      fixture.destroyExplorer();
      fixture.desktop.bringForward();
      expect(fixture.shell.openExplorer).toHaveBeenCalledTimes(2);
      expect(fixture.core.openDoor).toHaveBeenLastCalledWith('Unlock');
      expect(fixture.passphrase.isVisible()).toBe(true);
    });

    it('leaves an unlock already showing as it is', () => {
      fixture.desktop.handle('UnlockRequested');
      fixture.desktop.bringForward();
      expect(fixture.passphrase.load).toHaveBeenCalledTimes(2);
      expect(fixture.passphrase.bringForward).toHaveBeenCalledTimes(2);
    });
  });
});

describe('closing the explorer window', () => {
  const PLATFORMS = ['darwin', 'linux', 'win32'] as const;
  const cases = PLATFORMS.flatMap((platform) =>
    [true, false].flatMap((tray) => [true, false].map((quitting) => ({ platform, tray, quitting }))),
  );

  it.each(cases)('on $platform with a tray $tray, quitting $quitting', ({ platform, tray, quitting }) => {
    const expected = quitting ? 'close' : tray || platform === 'darwin' ? 'hide' : 'quit';
    expect(explorerCloses(platform, tray, quitting)).toBe(expected);

    const resident = setup({ platform, tray });
    resident.desktop.open();
    resident.desktop.handle('LibraryOpened');
    if (quitting) {
      resident.desktop.quit();
    }
    const event = closing();
    resident.desktop.explorerClosing(event);
    expect(event.prevented()).toBe(expected === 'hide');
    expect(resident.shell.hideExplorer).toHaveBeenCalledTimes(expected === 'hide' ? 1 : 0);
    expect(resident.shell.quit).toHaveBeenCalledTimes(expected === 'quit' ? 1 : 0);
  });

  it('hides it on Linux with a tray, leaving the app running', () => {
    fixture.desktop.handle('LibraryOpened');
    fixture.desktop.explorerClosing(closing());
    expect(fixture.explorer()).toBe('hidden');
    expect(fixture.shell.quit).not.toHaveBeenCalled();
  });
});

describe('the Library being locked, as the tray sees it', () => {
  it('is not before a Library is served', () => {
    fixture.serve(LOCKED);
    expect(fixture.desktop.locked()).toBe(false);
  });

  it('follows the Library once one is served', () => {
    fixture.desktop.handle('LibraryOpened');
    fixture.serve(UNLOCKED);
    expect(fixture.desktop.locked()).toBe(false);
    fixture.serve(LOCKED);
    expect(fixture.desktop.locked()).toBe(true);
    fixture.serve(null);
    expect(fixture.desktop.locked()).toBe(false);
  });

  it('is what the tray’s Unlock… asks the Passphrase for', () => {
    fixture.desktop.handle('LibraryOpened');
    fixture.serve(LOCKED);
    fixture.desktop.askForPassphrase();
    expect(fixture.core.openDoor).toHaveBeenLastCalledWith('Unlock');
    expect(fixture.passphrase.isVisible()).toBe(true);
  });
});

describe('a spent Passphrase window', () => {
  it('is loaded afresh in unlock mode when it is asked for again', () => {
    fixture.desktop.handle('LibraryOpened');
    fixture.serve(LOCKED);
    fixture.desktop.handle('UnlockRequested');
    fixture.desktop.handle('PassphraseWindowSpent');
    fixture.desktop.handle('UnlockRequested');
    expect(fixture.loaded).toEqual([
      'http://127.0.0.1:9/#door=Open-1',
      'http://127.0.0.1:9/#door=Unlock-2',
      'http://127.0.0.1:9/#door=Unlock-3',
    ]);
    expect(fixture.passphrase.isVisible()).toBe(true);
  });

  it('is loaded once, after which the fresh window is live and kept as it is', () => {
    fixture.desktop.handle('LibraryOpened');
    fixture.serve(LOCKED);
    fixture.desktop.handle('UnlockRequested');
    fixture.desktop.handle('PassphraseWindowSpent');
    fixture.desktop.handle('UnlockRequested');
    fixture.desktop.handle('UnlockRequested');
    expect(fixture.passphrase.load).toHaveBeenCalledTimes(3);
  });

  it('before a Library is served is loaded afresh in open mode when brought forward', () => {
    fixture.desktop.handle('PassphraseWindowSpent');
    fixture.desktop.bringForward();
    expect(fixture.loaded).toEqual(['http://127.0.0.1:9/#door=Open-1', 'http://127.0.0.1:9/#door=Open-2']);
    fixture.desktop.bringForward();
    expect(fixture.passphrase.load).toHaveBeenCalledTimes(2);
  });

  it('says where it could not be opened again, and still brings it forward', () => {
    fixture.desktop.handle('PassphraseWindowSpent');
    fixture.core.openDoor.mockImplementationOnce(() => {
      throw new Error('the desktop has not started');
    });
    fixture.desktop.bringForward();
    expect(fixture.shell.warn).toHaveBeenCalledWith(expect.stringContaining('the desktop has not started'));
    expect(fixture.passphrase.bringForward).toHaveBeenCalledTimes(2);
  });
});
