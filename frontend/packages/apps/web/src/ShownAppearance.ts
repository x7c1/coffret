// The appearance this page is drawn in, held for the page as a whole: what the
// header's switch shows and changes, what the system's choice is followed
// through, and what is painted onto the page's root.

import type { Appearance } from '@coffret/api';

import { paintScheme, schemeOf, SYSTEM_DARK, type Scheme } from './appearance';

/** The part of a `MediaQueryList` read here: whether it matches, and hearing it change. */
type SystemQuery = Pick<MediaQueryList, 'matches' | 'addEventListener' | 'removeEventListener'>;

/**
 * The appearance this page is drawn in, and the page drawn in it.
 *
 * It starts at what the host served the page in ([`servedIn`](#servedIn)),
 * so the explorer's first paint keeps the set the browser's first frame was
 * drawn in; that is `system` where nothing was kept or what was kept cannot be
 * read: how the explorer looks is never a reason for it not to open. What the
 * server kept for this device arrives later and is taken up then
 * ([`adopt`](#adopt)) — unless the person chose on this page meanwhile, which
 * is the newer word.
 */
export class ShownAppearance {
  private shown: Appearance;
  private chosen = false;
  private readonly listeners = new Set<() => void>();

  constructor(
    /** The system's own choice, or `null` where the browser cannot say, which is read as light. */
    private readonly system: SystemQuery | null,
    /** The element the colours are painted onto: the page's root. */
    private readonly root: HTMLElement,
  ) {
    this.shown = servedIn(root);
  }

  /**
   * Paints the page in what is shown, and follows the system's choice from
   * here on, as it changes. Answers with what stops following it.
   */
  start(): () => void {
    this.paint();
    const changed = () => this.paint();
    this.system?.addEventListener('change', changed);
    return () => this.system?.removeEventListener('change', changed);
  }

  /** What the page is shown in, as chosen: `system`, `light` or `dark`. */
  get appearance(): Appearance {
    return this.shown;
  }

  /** The set that comes to, the system's choice read where it is `system`. */
  get scheme(): Scheme {
    return schemeOf(this.shown, this.system?.matches === true);
  }

  /** Hears every change of what is shown; answers with what stops hearing it. */
  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  /**
   * Whether what is shown is the person's choice on this page rather than
   * what was kept: then it is this page's to keep, and what was kept is not
   * taken up over it.
   */
  get chosenHere(): boolean {
    return this.chosen;
  }

  /** The person chose `appearance` on this page. */
  choose(appearance: Appearance): void {
    this.chosen = true;
    this.show(appearance);
  }

  /**
   * What was kept for this device arrived: shown, unless the person has
   * chosen on this page since it was asked for.
   */
  adopt(kept: Appearance): void {
    if (!this.chosen) {
      this.show(kept);
    }
  }

  private show(appearance: Appearance): void {
    if (appearance === this.shown) {
      return;
    }
    this.shown = appearance;
    this.paint();
    for (const listener of this.listeners) {
      listener();
    }
  }

  private paint(): void {
    paintScheme(this.scheme, this.root);
  }
}

/**
 * The appearance the host served the page in: the `data-appearance` it wrote
 * onto the root for a set this device chose, and `system` where it wrote none
 * or one this page does not know.
 */
export function servedIn(root: HTMLElement): Appearance {
  const served = root.dataset.appearance;
  return served === 'light' || served === 'dark' ? served : 'system';
}

/**
 * This page's appearance: the one the page is painted in and the header's
 * switch changes. The system's choice is read where the browser offers it.
 */
export const SHOWN_HERE = new ShownAppearance(
  typeof window.matchMedia === 'function' ? window.matchMedia(SYSTEM_DARK) : null,
  document.documentElement,
);
