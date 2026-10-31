// What the app does with each thing the addon tells it, with its windows
// being closed, and with being asked for the explorer again: the whole of its
// behaviour between the windows, kept apart from Electron so that it is tested
// without it.

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
  /**
   * Opens a new explorer's window onto the Library served, loaded from a fresh
   * launch ticket.
   */
  openExplorer(): void;
  /**
   * Brings the explorer's window forward — shown, restored and focused —
   * saying whether there was one to bring: none before a Library is opened,
   * nor once the window was destroyed.
   */
  focusExplorer(): boolean;
  /** Hides the explorer's window, keeping it and its page as they are. */
  hideExplorer(): void;
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

/** What closing the explorer's window comes to. */
export type ExplorerClose = 'close' | 'hide' | 'quit';

/**
 * What closing the explorer's window comes to on `platform`, with a tray icon
 * shown or not, while the app is quitting or not.
 *
 * - While the app quits, the window closes like any other.
 * - With a tray icon, the window is only hidden: the app stays, serving the
 *   Library, and the tray, a second launch or the Dock brings it back.
 * - Without one, it is hidden on macOS too, since the Dock icon is still
 *   there to bring it back; elsewhere nothing would be left to reach the app
 *   by, so it quits.
 */
export function explorerCloses(platform: NodeJS.Platform, tray: boolean, quitting: boolean): ExplorerClose {
  if (quitting) {
    return 'close';
  }
  if (tray || platform === 'darwin') {
    return 'hide';
  }
  return 'quit';
}

/** Where the app runs, as far as its windows closing goes. */
export interface Residency {
  platform: NodeJS.Platform;
  /** Whether a tray icon is shown. */
  tray: boolean;
}

/**
 * The app's standing between its windows.
 *
 * - Before a Library is served, the Passphrase window is the app: it is
 *   opened in *open* mode, and closing it quits.
 * - Once one is served, the explorer has a window of its own and the
 *   Passphrase window is hidden, not closed. It comes back in *unlock* mode
 *   when the explorer asks for the Library to be unlocked — a press of
 *   *Unlock*, or the page seeing the idle lock — or the tray's *Unlock…* is
 *   chosen, and goes again once the Library is unlocked. Closing it only
 *   hides it. Closing the explorer's window hides it too, where the app stays
 *   resident ({@link explorerCloses}).
 * - A Passphrase window whose token the door ended after its last refused
 *   try is spent: it is loaded afresh the next time it is brought forward,
 *   rather than shown again as it is.
 * - When the server stops, nothing is served any more, and the app says so
 *   and quits.
 */
export class Desktop {
  private served = false;
  /** The mode the Passphrase window's page was last loaded in. */
  private mode: DoorMode | null = null;
  private quitting = false;
  /** Whether the door ended the Passphrase window's token unused. */
  private spent = false;

  constructor(
    private readonly core: Core,
    private readonly passphrase: PassphraseWindow,
    private readonly shell: Shell,
    private readonly residency: Residency,
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
        this.askForPassphrase();
        return;
      case 'PassphraseWindowSpent':
        this.spent = true;
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

  /** What closing the explorer's window comes to: see {@link explorerCloses}. */
  explorerClosing(event: Closing): void {
    switch (explorerCloses(this.residency.platform, this.residency.tray, this.quitting)) {
      case 'close':
        return;
      case 'hide':
        event.preventDefault();
        this.shell.hideExplorer();
        return;
      case 'quit':
        this.shell.quit();
        return;
    }
  }

  /** The app has begun quitting: no window is kept from closing any more. */
  quit(): void {
    this.quitting = true;
  }

  /** Whether a Library is served and waits, locked, for its Passphrase. */
  locked(): boolean {
    if (!this.served) {
      return false;
    }
    return this.core.served()?.unlocked === false;
  }

  /**
   * Brings the explorer back: what a second launch, a click on the Dock icon,
   * and the tray's *Open the explorer* do.
   *
   * Before a Library is served, that is the Passphrase window, where somebody
   * is in the middle of opening one. Once one is, it is the explorer's window
   * — shown again where it was hidden, and opened anew, from a fresh launch
   * ticket, where it was destroyed — with the Passphrase window in front of
   * it in *unlock* mode where the Library has locked, since the explorer has
   * nothing to show until it is unlocked.
   */
  bringForward(): void {
    if (!this.served) {
      this.passphraseForward();
      return;
    }
    if (!this.shell.focusExplorer()) {
      this.shell.openExplorer();
    }
    if (this.locked()) {
      this.askForPassphrase();
    }
  }

  /**
   * Puts the Passphrase window in front in *unlock* mode: what the explorer's
   * asking for an unlock and the tray's *Unlock…* do. A window already showing
   * that mode is only brought forward, so that a second asking does not wipe
   * what somebody is in the middle of typing — unless it is spent, when it
   * has nothing left to take. Otherwise it is loaded afresh, from a token of
   * its own, which also leaves its field empty.
   *
   * Nothing is asked where no Library is served, or where the one served is
   * already unlocked: an asking that arrives after an unlock landed has
   * nothing left to ask for.
   */
  askForPassphrase(): void {
    const served = this.served ? this.core.served() : null;
    if (served === null) {
      this.shell.warn('the Passphrase was asked for again while no Library was served');
      return;
    }
    if (served.unlocked) {
      return;
    }
    if (!(this.mode === 'Unlock' && this.passphrase.isVisible() && !this.spent)) {
      try {
        this.load('Unlock');
      } catch (error) {
        this.shell.warn(`the Passphrase window could not be opened to unlock: ${message(error)}`);
        return;
      }
    }
    this.passphrase.bringForward();
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
   * Brings the Passphrase window forward as it is, or loaded afresh in the
   * mode it was in where it is spent.
   */
  private passphraseForward(): void {
    if (this.spent && this.mode !== null) {
      try {
        this.load(this.mode);
      } catch (error) {
        this.shell.warn(`the Passphrase window could not be opened again: ${message(error)}`);
      }
    }
    this.passphrase.bringForward();
  }

  /** Loads the Passphrase window afresh in `mode`, from a token of its own. */
  private load(mode: DoorMode): void {
    this.passphrase.load(this.core.openDoor(mode));
    this.mode = mode;
    this.spent = false;
  }
}

/** The sentence an exception carries, which is what the addon refuses with. */
export function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
