// What the app does with each thing the addon tells it, and with the
// Passphrase window being closed: the whole of its behaviour between the
// windows, kept apart from Electron so that it is tested without it.

import type { DesktopEvent, DoorMode, ServedLibrary } from './addon.js';

/** What the app asks of the addon here. */
export interface Core {
  openDoor(mode: DoorMode): string;
  served(): ServedLibrary | null;
}

/** The Passphrase window, as the app drives it. */
export interface PassphraseWindow {
  /** Loads the door's page from `url`, afresh. */
  load(url: string): void;
  /** Shows the window, restored and in front, and focuses it. */
  bringForward(): void;
  hide(): void;
  isVisible(): boolean;
}

/** What the app does around the Passphrase window. */
export interface Shell {
  /** Opens the explorer's window onto the Library just opened. */
  openExplorer(): void;
  /** Brings the explorer's window forward, saying whether there was one. */
  focusExplorer(): boolean;
  /** Says that the Library's server stopped, and waits until that is dismissed. */
  reportServerStopped(): void;
  /** Ends the app. */
  quit(): void;
  /** Says something went other than expected, for whoever reads the app's output. */
  warn(message: string): void;
}

/** The part of a `close` event the app answers. */
export interface Closing {
  preventDefault(): void;
}

/**
 * The app's standing between its windows.
 *
 * - Before a Library is served, the Passphrase window is the app: it is
 *   opened in *open* mode, and closing it quits.
 * - Once one is served, the explorer has a window of its own and the
 *   Passphrase window is hidden, not closed. It comes back in *unlock* mode
 *   when the explorer asks for the Library to be unlocked — a press of
 *   *unlock*, or the page seeing the idle lock — and goes again once the
 *   Library is unlocked. Closing it only hides it.
 * - When the server stops, nothing is served any more, and the app says so
 *   and quits.
 */
export class Desktop {
  private served = false;
  /** The mode the Passphrase window's page was last loaded in. */
  private mode: DoorMode | null = null;
  private quitting = false;

  constructor(
    private readonly core: Core,
    private readonly passphrase: PassphraseWindow,
    private readonly shell: Shell,
  ) {}

  /** Opens the Passphrase window on the Libraries this device has. */
  open(): void {
    this.load('Open');
    this.passphrase.bringForward();
  }

  /** Acts on one event from the addon. */
  handle(event: DesktopEvent): void {
    switch (event) {
      case 'LibraryOpened':
        this.libraryOpened();
        return;
      case 'UnlockRequested':
        this.unlockRequested();
        return;
      case 'LibraryUnlocked':
        this.libraryUnlocked();
        return;
      case 'ServerStopped':
        this.served = false;
        this.shell.reportServerStopped();
        this.shell.quit();
        return;
    }
  }

  /**
   * What closing the Passphrase window comes to: before a Library is served
   * there is nothing else to the app, so it quits; afterwards the window is
   * only hidden, to come back when the Library locks. While the app is
   * quitting the window closes like any other.
   */
  passphraseClosing(event: Closing): void {
    if (this.quitting) {
      return;
    }
    if (!this.served) {
      this.shell.quit();
      return;
    }
    event.preventDefault();
    this.passphrase.hide();
  }

  /** The app has begun quitting: no window is kept from closing any more. */
  quit(): void {
    this.quitting = true;
  }

  /**
   * What a second launch of the app does in this one: the Passphrase window
   * where it is showing, since that is where somebody is in the middle of
   * opening or unlocking the Library, and the explorer otherwise.
   */
  bringForward(): void {
    if (this.passphrase.isVisible() || !this.shell.focusExplorer()) {
      this.passphrase.bringForward();
    }
  }

  private libraryOpened(): void {
    if (this.served) {
      this.shell.warn('a Library was said to be opened while one was already served');
      return;
    }
    this.served = true;
    this.passphrase.hide();
    this.shell.openExplorer();
  }

  /**
   * Hides the Passphrase window, and puts the explorer back in front where the
   * window was showing, so that whoever typed the Passphrase lands back where
   * they asked for the unlock rather than wherever the platform moves the focus.
   */
  private libraryUnlocked(): void {
    const showing = this.passphrase.isVisible();
    this.passphrase.hide();
    if (showing) {
      this.shell.focusExplorer();
    }
  }

  /**
   * Puts the Passphrase window in front in *unlock* mode. A window already
   * showing that mode is only brought forward, so that a second asking does not
   * wipe what somebody is in the middle of typing; otherwise it is loaded
   * afresh, from a token of its own, which also leaves its field empty.
   *
   * Nothing is asked where no Library is served, or where the one served is
   * already unlocked: an asking that arrives after an unlock landed has
   * nothing left to ask for.
   */
  private unlockRequested(): void {
    const served = this.served ? this.core.served() : null;
    if (served === null) {
      this.shell.warn('the Passphrase was asked for again while no Library was served');
      return;
    }
    if (served.unlocked) {
      return;
    }
    if (!(this.mode === 'Unlock' && this.passphrase.isVisible())) {
      try {
        this.load('Unlock');
      } catch (error) {
        this.shell.warn(`the Passphrase window could not be opened to unlock: ${message(error)}`);
        return;
      }
    }
    this.passphrase.bringForward();
  }

  private load(mode: DoorMode): void {
    this.passphrase.load(this.core.openDoor(mode));
    this.mode = mode;
  }
}

/** The sentence an exception carries, which is what the addon refuses with. */
export function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
