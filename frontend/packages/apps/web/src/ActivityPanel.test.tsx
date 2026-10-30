import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, expect, it, onTestFinished, vi } from 'vitest';

import {
  ActivityPanel,
  LOCKED,
  SHORTCUTS,
  SHOW_FAILURES,
  UNLOCK,
  type UnlockOffer,
} from './ActivityPanel';
import { ANSWER_HOLDS, type Entry, type Running } from './activity';
import type { Trouble } from './retry';

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const QUIET = { running: false, waiting: false, said: null, refused: null, ask: () => undefined };

function entry(over: Partial<Entry> = {}): Entry {
  return {
    key: 'server:1',
    at: '2026-10-30T09:00:00Z',
    outcome: 'success',
    text: 'added 3 files to the Library',
    retry: null,
    retryAs: null,
    standing: false,
    settled: false,
    ranOut: false,
    answers: false,
    ...over,
  };
}

const STOPPED_BOOK = entry({
  key: 'server:2',
  outcome: 'failure',
  text: "could not pack books/vol-1 — the Library's Storage did not answer",
  retry: { flow: 'freeze', folder: 'books/vol-1' },
  retryAs: 'pack again',
  standing: true,
});

function panel({
  running = [],
  entries = [],
  trouble = null,
  onRetry = () => undefined,
  locked = null,
}: {
  running?: Running[];
  entries?: Entry[];
  trouble?: Trouble | null;
  onRetry?: (entry: Entry) => void;
  locked?: UnlockOffer | null;
} = {}) {
  return render(
    <ActivityPanel
      kept={null}
      running={running}
      entries={entries}
      trouble={trouble}
      onRetry={onRetry}
      refresh={QUIET}
      reconnect={null}
      locked={locked}
    />,
  );
}

function toggle(): HTMLElement {
  return screen.getByRole('button', { name: 'Activity' });
}

// Collapsed by default: one row, saying what is running and how far the first
// of it has got, and no list. The Library's name is the header's, not the row's.
it('starts collapsed, as one row saying what is running', () => {
  panel({
    running: [
      { key: 'adding', text: 'sending 188 files to Vol. 06', progress: 0.6 },
      { key: 'sync', text: 'backing up what was added…', progress: null },
    ],
  });
  expect(toggle().getAttribute('aria-expanded')).toBe('false');
  expect(screen.getByText('sending 188 files to Vol. 06 · 1 more')).toBeTruthy();
  expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('60');
  expect(screen.queryByText('books')).toBeNull();
  expect(screen.queryByRole('heading', { name: 'In progress' })).toBeNull();
});

// A failure does not open the panel: the badge is the signal.
it('counts the failures on a badge without opening', () => {
  panel({ entries: [STOPPED_BOOK, entry()] });
  expect(screen.getByText('1 failed')).toBeTruthy();
  expect(toggle().getAttribute('aria-expanded')).toBe('false');
  expect(screen.queryByRole('button', { name: 'pack again' })).toBeNull();
});

// Pressing the badge opens the panel at the newest failure standing: scrolled
// to, focused and lit for a moment — past a failure a later run settled.
it('opens at the first failure standing when the badge is pressed', () => {
  vi.useFakeTimers();
  // jsdom lays nothing out and has no scrollIntoView of its own to spy on.
  const scrolled = vi.fn();
  Element.prototype.scrollIntoView = scrolled;
  onTestFinished(() => {
    delete (Element.prototype as Partial<Element>).scrollIntoView;
  });
  const settled = entry({
    key: 'server:3',
    outcome: 'failure',
    text: 'could not fetch books/vol-2/001.jpg',
    settled: true,
  });
  panel({ entries: [entry(), settled, STOPPED_BOOK] });

  fireEvent.click(screen.getByRole('button', { name: `${SHOW_FAILURES} (1 failed)` }));
  expect(toggle().getAttribute('aria-expanded')).toBe('true');
  const items = within(screen.getByRole('region', { name: 'Earlier' })).getAllByRole('listitem');
  expect(document.activeElement).toBe(items[2]);
  expect(scrolled).toHaveBeenCalledTimes(1);
  expect(scrolled.mock.contexts[0]).toBe(items[2]);
  expect(items[2].style.background).not.toBe('');
  expect(items[0].style.background).toBe('');

  act(() => {
    vi.advanceTimersByTime(2000);
  });
  expect(items[2].style.background, 'and the light goes').toBe('');
  expect(toggle().getAttribute('aria-expanded'), 'while the panel stays open').toBe('true');
});

// The answer to a gesture takes the collapsed row ahead of what is running,
// in its outcome's colour, with the bar and the badge still there — until the
// next gesture, or until its moment runs out.
it('says the answer to a gesture in place of what is running for a moment', () => {
  vi.useFakeTimers();
  const now = Date.parse('2026-10-30T09:00:00.000Z');
  vi.setSystemTime(now);
  const unsupported = entry({
    key: 'page:1',
    at: new Date(now).toISOString(),
    outcome: 'notice',
    text: 'preview of this format is not supported yet',
    answers: true,
  });
  const running = [{ key: 'sync', text: 'backing up what was added…', progress: 0.5 }];
  const entries = [unsupported, STOPPED_BOOK];
  const { unmount } = panel({ running, entries });

  const said = screen.getByText('preview of this format is not supported yet');
  expect(said.style.color).not.toBe('');
  expect(screen.queryByText('backing up what was added…')).toBeNull();
  expect(screen.getByRole('progressbar')).toBeTruthy();
  expect(screen.getByText('1 failed')).toBeTruthy();

  act(() => {
    vi.advanceTimersByTime(ANSWER_HOLDS);
  });
  expect(screen.queryByText('preview of this format is not supported yet')).toBeNull();
  expect(screen.getByText('backing up what was added…')).toBeTruthy();
  unmount();

  // Again, and this time the next gesture takes it away first.
  vi.setSystemTime(now);
  panel({ running, entries });
  expect(screen.getByText('preview of this format is not supported yet')).toBeTruthy();
  act(() => {
    vi.advanceTimersByTime(100);
  });
  fireEvent.keyDown(window, { key: 'ArrowDown' });
  expect(screen.queryByText('preview of this format is not supported yet')).toBeNull();
  expect(screen.getByText('backing up what was added…')).toBeTruthy();
});

// Expanded: the two groups, the finished entries with their outcome, and the
// Retry on the failure that offers one — and pressing it presses that entry's.
it('lists what is in progress and what is over once opened, with a Retry', () => {
  const onRetry = vi.fn();
  panel({
    running: [{ key: 'sync', text: 'backing up what was added…', progress: null }],
    entries: [STOPPED_BOOK, entry()],
    onRetry,
  });
  fireEvent.click(toggle());
  expect(toggle().getAttribute('aria-expanded')).toBe('true');

  const progress = within(screen.getByRole('region', { name: 'In progress' }));
  expect(progress.getByText('backing up what was added…')).toBeTruthy();
  expect(progress.queryByRole('button')).toBeNull();

  const over = within(screen.getByRole('region', { name: 'Earlier' }));
  const items = over.getAllByRole('listitem');
  expect(items).toHaveLength(2);
  expect(within(items[0]).getByRole('img', { name: 'Failed' })).toBeTruthy();
  expect(within(items[1]).getByRole('img', { name: 'Done' })).toBeTruthy();
  expect(within(items[1]).queryByRole('button')).toBeNull();

  fireEvent.click(over.getByRole('button', { name: 'pack again' }));
  expect(onRetry).toHaveBeenCalledWith(STOPPED_BOOK);

  // And closed again.
  fireEvent.click(toggle());
  expect(screen.queryByRole('region', { name: 'Earlier' })).toBeNull();
});

// A Retry that was refused says so beside the entry whose Retry it was.
it('says what a Retry was refused with beside its own entry', () => {
  panel({
    entries: [STOPPED_BOOK, entry()],
    trouble: { pressed: { flow: 'freeze', folder: 'books/vol-1' }, said: 'the Passphrase is required' },
  });
  fireEvent.click(toggle());
  const items = within(screen.getByRole('region', { name: 'Earlier' })).getAllByRole('listitem');
  expect(within(items[0]).getByText('the Passphrase is required')).toBeTruthy();
  expect(within(items[1]).queryByText('the Passphrase is required')).toBeNull();
});

// Nothing to clear: there is no such button.
it('offers no way to clear what is over', () => {
  panel({ entries: [entry()] });
  fireEvent.click(toggle());
  expect(screen.queryByRole('button', { name: /clear|dismiss/i })).toBeNull();
});

// While the Library is locked the row says so, with the unlock, in place of
// the summary.
it('says the Library is locked and offers the unlock in place of the summary', () => {
  const ask = vi.fn();
  panel({
    running: [{ key: 'sync', text: 'backing up what was added…', progress: null }],
    locked: { asking: false, said: null, refused: null, ask },
  });
  expect(screen.getByText(LOCKED)).toBeTruthy();
  expect(screen.queryByText('backing up what was added…')).toBeNull();
  fireEvent.click(screen.getByRole('button', { name: UNLOCK }));
  expect(ask).toHaveBeenCalled();
});

// The ? shows the tree's keyboard shortcuts while it is hovered or focused.
it('shows the keyboard shortcuts while the ? is hovered or focused', () => {
  panel();
  const help = screen.getByRole('button', { name: 'Keyboard shortcuts' });
  expect(screen.queryByRole('tooltip')).toBeNull();

  fireEvent.mouseEnter(help);
  const tip = within(screen.getByRole('tooltip'));
  for (const [keys, what] of SHORTCUTS) {
    expect(tip.getByText(keys)).toBeTruthy();
    expect(tip.getByText(what)).toBeTruthy();
  }
  fireEvent.mouseLeave(help);
  expect(screen.queryByRole('tooltip')).toBeNull();

  fireEvent.focus(help);
  expect(screen.getByRole('tooltip')).toBeTruthy();
  fireEvent.blur(help);
  expect(screen.queryByRole('tooltip')).toBeNull();
});
