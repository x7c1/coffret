import {
  useEffect,
  useId,
  useRef,
  useState,
  type MutableRefObject,
  type ReactNode,
} from 'react';

import type { KeptParcels } from '@coffret/api';

import {
  answering,
  clock,
  collapsed,
  failedBadge,
  firstStanding,
  retryKey,
  type Entry,
  type Outcome,
  type Row,
  type Running,
} from './activity';
import { size } from './humanize';
import { ReconnectOffer, type Offer } from './ReconnectOffer';
import type { Trouble } from './retry';
import { SyncIcon } from './SyncIcon';
import { COLOR, SHADOW } from './theme';

/**
 * What the server and this page are doing and have done, along the bottom of
 * the screen.
 *
 * **Collapsed**, the default, it is one row: a chevron and **Activity**, one
 * line saying what is in progress — the first thing moving, with a count of
 * the rest — a small bar for how far that has got, and a red badge counting
 * the failures that stand. A failure does not open the panel by itself: the
 * badge is the signal, and a panel that sprang open over the tree each time
 * Storage blinked would take the screen from whoever was reading. Pressing the
 * badge is what opens it, at the newest failure standing, lit for a moment —
 * a count that said there was something wrong and could not be followed to it
 * would leave a person hunting down a list for the red dot.
 *
 * The answer this page gave the gesture just made takes the row's sentence for
 * a few seconds, ahead of what is in progress (see
 * [`answering`](./activity)): filed only under **Earlier**, it would be seen
 * by nobody.
 *
 * **Expanded**, it lists **In progress** — when the page first saw each, what
 * it is, how far it has got — and **Earlier**: what the server finished, and
 * what this page answered a gesture with, newest first, each with a dot for
 * what it came to and, on a failure asking again could help, a Retry. There
 * is nothing to clear: the server forgets what is old on its own, and what a
 * person has read is still worth having to read again.
 *
 * The row also carries what there is to say about the Library itself: the
 * parcels this device keeps, and the sync icon that asks it what is new —
 * always there, unlike everything above, which
 * comes and goes with the work. While the Library is locked the row says so
 * in place of the summary, with the one button that unlocks it; every Retry
 * goes, since each would meet the same locked refusal.
 *
 * At its right end a **?** shows the tree's keyboard shortcuts while it is
 * hovered or focused.
 *
 * It is drawn below the tree and the reader's sheet and not over them, so
 * opening it shrinks them rather than covering what they show.
 */
export function ActivityPanel({
  kept,
  running,
  entries,
  trouble,
  onRetry,
  refresh,
  reconnect,
  locked,
}: {
  /**
   * The parcels this device keeps for the Library, and `null` where the
   * server has not said — before its first answer, and while it is locked.
   */
  kept: KeptParcels | null;
  /** **In progress**, top to bottom (see [`inProgress`](./activity)). */
  running: readonly Running[];
  /** **Earlier**, newest first (see [`earlier`](./activity)). */
  entries: readonly Entry[];
  /** The Retry that was refused and what refused it, if one was. */
  trouble: Trouble | null;
  /** Presses one entry's Retry. */
  onRetry: (entry: Entry) => void;
  /**
   * Asking the Library what is new, and what the last asking came to.
   *
   * One value rather than five props, because the five are one control: the
   * icon, whether it is busy, whether changes are waiting, what it found, and
   * what refused it.
   */
  refresh: {
    /** Whether a refresh is running right now. */
    running: boolean;
    /**
     * Whether the last look at the remote head said another device has
     * committed since this one caught up.
     */
    waiting: boolean;
    /** What the last one came to, and `null` before any has run. */
    said: string | null;
    /** What refused the last one, and `null` where none was. */
    refused: string | null;
    ask: () => void;
  };
  /**
   * The reconnect offered where a failure standing is the permission Storage
   * is reached through running out, and `null` on a Library whose Storage no
   * reconnect reaches.
   */
  reconnect: Offer | null;
  /** The unlock offered while the Library is locked, and `null` while it is open. */
  locked: UnlockOffer | null;
}) {
  const [open, setOpen] = useState(false);
  const listId = useId();
  const seen = useFirstSeen(running);
  const gestured = useLastGesture();
  const answer = answering(entries, Date.now(), gestured);
  useRenderAt(answer?.until ?? null);
  const row = collapsed(running, entries, answer?.entry ?? null);
  const badge = failedBadge(row.failed);
  // The failure the badge was pressed to show, lit until the moment passes.
  const [sought, setSought] = useState<string | null>(null);
  const soughtItem = useRef<HTMLLIElement | null>(null);
  useEffect(() => {
    if (sought === null) {
      return;
    }
    const item = soughtItem.current;
    // Not in jsdom, which lays nothing out.
    item?.scrollIntoView?.({ block: 'nearest' });
    item?.focus({ preventScroll: true });
    const timer = setTimeout(() => setSought(null), SOUGHT_LIT);
    return () => clearTimeout(timer);
  }, [sought]);
  const showFailures = () => {
    setOpen(true);
    setSought(firstStanding(entries)?.key ?? null);
  };
  // One Storage outage stops every run under way together, so one reconnect
  // stands for every failure the permission running out explains.
  const expired =
    reconnect !== null &&
    locked === null &&
    (trouble?.ranOut === true || entries.some((entry) => entry.standing && entry.ranOut));
  return (
    <footer aria-label="Activity" style={PANEL}>
      <div style={ROW}>
        <button
          onClick={() => setOpen((was) => !was)}
          aria-expanded={open}
          aria-controls={listId}
          // An icon rather than the word: a clock turned back, for the work
          // that ran — what the panel keeps once a run is over.
          aria-label="Activity"
          title="Activity"
          style={TOGGLE}
        >
          <svg
            aria-hidden="true"
            width={12}
            height={12}
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2.5}
            strokeLinecap="round"
            strokeLinejoin="round"
            style={{ display: 'block', transform: open ? 'rotate(90deg)' : undefined }}
          >
            <path d="m9 6 6 6-6 6" />
          </svg>
          <svg
            aria-hidden="true"
            width={16}
            height={16}
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            strokeLinecap="round"
            strokeLinejoin="round"
            style={{ display: 'block', color: COLOR.text }}
          >
            <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
            <path d="M3 3v5h5" />
            <path d="M12 7v5l4 2" />
          </svg>
        </button>
        {locked !== null ? (
          <>
            <span style={{ color: COLOR.refused }}>{LOCKED}</span>
            <button
              onClick={locked.ask}
              disabled={locked.asking}
              style={{ ...BUTTON, cursor: locked.asking ? 'default' : 'pointer' }}
            >
              {UNLOCK}
            </button>
            {locked.refused !== null ? (
              <span style={{ color: COLOR.refused }}>{locked.refused}</span>
            ) : (
              locked.said !== null && <span style={{ color: COLOR.text }}>{locked.said}</span>
            )}
          </>
        ) : (
          // Open, the groups below say all of it, and the row says it again
          // nowhere: one sentence drawn twice is one a person reads twice.
          !open && (
            <>
              <span style={{ ...SUMMARY, color: summaryColour(row, running.length > 0) }} title={row.text}>
                {row.text}
              </span>
              {row.progress !== null && <Bar progress={row.progress} width={96} />}
            </>
          )
        )}
        {badge !== null && (
          <button
            onClick={showFailures}
            aria-label={`${SHOW_FAILURES} (${badge})`}
            title={SHOW_FAILURES}
            style={BADGE}
          >
            {badge}
          </button>
        )}
        {expired && reconnect !== null && <ReconnectOffer offer={reconnect} />}
        <span style={END}>
          {/* The parcels a reader's fetches read and kept, which nothing else
              on this page would ever mention, and which grow by a parcel or
              two for every book opened. */}
          {kept !== null && kept.count > 0 && <span>{keptLine(kept)}</span>}
          {/* What the sync icon's last asking came to, beside it rather than
              in the summary: a refresh that is over has nothing to stop
              saying, and in the summary it would sit over the next run's
              progress. */}
          {refresh.refused !== null ? (
            <span style={{ color: COLOR.refused }}>{refresh.refused}</span>
          ) : (
            refresh.said !== null && <span>{refresh.said}</span>
          )}
          <SyncIcon running={refresh.running} waiting={refresh.waiting} onPress={refresh.ask} />
          <Shortcuts />
        </span>
      </div>
      {open && (
        <div id={listId} style={LIST}>
          <Group title="In progress">
            {running.length === 0 ? (
              <p style={EMPTY}>{NOTHING_IN_PROGRESS}</p>
            ) : (
              <ul style={ITEMS}>
                {running.map((item) => (
                  <li key={item.key} style={ITEM}>
                    <span style={TIME}>{clock(seen.get(item.key) ?? '')}</span>
                    <span style={TEXT}>{item.text}</span>
                    {item.progress !== null && <Bar progress={item.progress} width={120} />}
                  </li>
                ))}
              </ul>
            )}
          </Group>
          <Group title="Earlier">
            {entries.length === 0 ? (
              <p style={EMPTY}>{NOTHING_EARLIER}</p>
            ) : (
              <ul style={ITEMS}>
                {entries.map((entry) => (
                  <EarlierItem
                    key={entry.key}
                    entry={entry}
                    anchor={entry.key === sought ? soughtItem : null}
                    refused={refusedHere(trouble, entry)}
                    onRetry={onRetry}
                  />
                ))}
              </ul>
            )}
          </Group>
        </div>
      )}
    </footer>
  );
}

/** One finished entry: its time, its dot, what it says, and its Retry. */
function EarlierItem({
  entry,
  anchor,
  refused,
  onRetry,
}: {
  entry: Entry;
  /** Where the badge's press is told this is the failure it shows, and `null` where it is not. */
  anchor: MutableRefObject<HTMLLIElement | null> | null;
  /** What a press of this entry's Retry was refused with, if one was. */
  refused: string | null;
  onRetry: (entry: Entry) => void;
}) {
  return (
    <li
      ref={anchor ?? undefined}
      tabIndex={anchor === null ? undefined : -1}
      style={anchor === null ? ITEM : { ...ITEM, ...SOUGHT }}
    >
      <span style={TIME}>{clock(entry.at)}</span>
      <span
        role="img"
        aria-label={OUTCOME[entry.outcome]}
        title={OUTCOME[entry.outcome]}
        style={{ ...DOT, background: DOT_COLOUR[entry.outcome] }}
      />
      <span style={{ ...TEXT, color: entry.settled ? COLOR.dim : COLOR.text }}>
        {entry.text}
        {refused !== null && (
          <span style={{ display: 'block', color: COLOR.refused }}>{refused}</span>
        )}
      </span>
      {entry.retry !== null && entry.retryAs !== null && (
        <button
          onClick={() => onRetry(entry)}
          aria-label={entry.retryAs}
          title={entry.retryAs}
          style={ICON_BUTTON}
        >
          <span aria-hidden="true">↻</span>
        </button>
      )}
    </li>
  );
}

/**
 * The colour of the collapsed row's summary. Something in progress is said in
 * the text colour. A finished entry the row says — the answer to a gesture, or
 * the newest entry where nothing is in progress — is said, where it is a
 * failure or a notice, in its dot's colour: it is often the only answer a
 * gesture gets — a row that would not open, a book that was not packed — and a
 * sentence in the grey of the columns along the bottom of the screen is one
 * nobody notices was said. A settled entry, a success, and the row where
 * nothing is running and nothing has ended are grey.
 */
function summaryColour(row: Row, moving: boolean): string {
  const said = row.said;
  if (said === null) {
    return moving ? COLOR.text : COLOR.dim;
  }
  return said.settled || said.outcome === 'success' ? COLOR.dim : DOT_COLOUR[said.outcome];
}

/**
 * When the last gesture on this page was made — a key pressed or a pointer
 * put down anywhere on it — in milliseconds since the epoch, and 0 before any.
 *
 * Listened for at the window and in the capture phase, so the gesture is
 * stamped before whatever handles it records its answer: an answer stamped at
 * or after the last gesture is that gesture's own.
 */
function useLastGesture(): number {
  const [gestured, setGestured] = useState(0);
  useEffect(() => {
    const stamp = () => setGestured(Date.now());
    window.addEventListener('keydown', stamp, true);
    window.addEventListener('pointerdown', stamp, true);
    return () => {
      window.removeEventListener('keydown', stamp, true);
      window.removeEventListener('pointerdown', stamp, true);
    };
  }, []);
  return gestured;
}

/**
 * Draws the panel again at `at`, milliseconds since the epoch, and never where
 * it is `null`: what the row says depends on the time, and nothing else would
 * change when an answer's moment runs out.
 */
function useRenderAt(at: number | null): void {
  const [, setTick] = useState(0);
  useEffect(() => {
    if (at === null) {
      return;
    }
    const timer = setTimeout(() => setTick((tick) => tick + 1), Math.max(0, at - Date.now()));
    return () => clearTimeout(timer);
  }, [at]);
}

/** What a refused Retry said, where it was this entry's Retry that was pressed. */
function refusedHere(trouble: Trouble | null, entry: Entry): string | null {
  return trouble !== null &&
    entry.retry !== null &&
    retryKey(entry.retry) === retryKey(trouble.pressed)
    ? trouble.said
    : null;
}

/** One of the two groups, under its heading. */
function Group({ title, children }: { title: string; children: ReactNode }) {
  const id = useId();
  return (
    <section aria-labelledby={id} style={{ padding: '4px 0' }}>
      <h3 id={id} style={HEADING}>
        {title}
      </h3>
      {children}
    </section>
  );
}

/** A thin bar for how far something has got, from 0 to 1. */
function Bar({ progress, width }: { progress: number; width: number }) {
  const percent = Math.round(Math.min(1, Math.max(0, progress)) * 100);
  return (
    <span
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={percent}
      style={{
        flex: '0 0 auto',
        width,
        height: 4,
        borderRadius: 2,
        background: COLOR.border,
        overflow: 'hidden',
      }}
    >
      <span
        style={{
          display: 'block',
          width: `${percent}%`,
          height: '100%',
          background: COLOR.accent,
        }}
      />
    </span>
  );
}

/**
 * The **?** at the panel's right end, and the tree's keyboard shortcuts while
 * it is hovered or focused — the ones the tree binds, and no others.
 */
function Shortcuts() {
  const [shown, setShown] = useState(false);
  const id = useId();
  return (
    <span style={{ position: 'relative' }}>
      <button
        aria-label="Keyboard shortcuts"
        aria-describedby={shown ? id : undefined}
        onMouseEnter={() => setShown(true)}
        onMouseLeave={() => setShown(false)}
        onFocus={() => setShown(true)}
        onBlur={() => setShown(false)}
        style={{ ...ICON_BUTTON, borderRadius: '50%', border: `1px solid ${COLOR.control}` }}
      >
        ?
      </button>
      {shown && (
        <div role="tooltip" id={id} style={TOOLTIP}>
          <dl style={{ margin: 0, display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '4px 12px' }}>
            {SHORTCUTS.map(([keys, what]) => (
              <div key={keys} style={{ display: 'contents' }}>
                <dt style={{ color: COLOR.text, whiteSpace: 'nowrap' }}>{keys}</dt>
                <dd style={{ margin: 0 }}>{what}</dd>
              </div>
            ))}
          </dl>
        </div>
      )}
    </span>
  );
}

/**
 * The tree's keyboard shortcuts, as the **?** lists them: what the tree and
 * the reader bind (see `TreeTable` and `ReaderView`), written here once.
 */
export const SHORTCUTS: readonly (readonly [string, string])[] = [
  ['↑ ↓', 'move'],
  ['→', 'expand, or go in'],
  ['←', 'collapse, or go to the parent'],
  ['Enter', 'open in the reader'],
  ['Esc', 'close the reader'],
  ['Shift ↑ ↓, Shift / ⌘ (Ctrl) click', 'select several'],
  ['Right-click', 'actions'],
];

/**
 * When this page first saw each thing in progress, by its key: the time
 * column of **In progress**. The server says when a run ended, not when it
 * began, so the moment the page first heard of it is the honest time to show.
 */
function useFirstSeen(running: readonly Running[]): ReadonlyMap<string, string> {
  const seen = useRef(new Map<string, string>());
  const now = new Date().toISOString();
  const keys = new Set(running.map((item) => item.key));
  for (const key of [...seen.current.keys()]) {
    if (!keys.has(key)) {
      seen.current.delete(key);
    }
  }
  for (const key of keys) {
    if (!seen.current.has(key)) {
      seen.current.set(key, now);
    }
  }
  return seen.current;
}

/** What the row says while the Library is locked, in place of the summary. */
export const LOCKED = 'the Library is locked';

/** The one button the row offers while it is. */
export const UNLOCK = 'unlock';

/** What the badge's press does, which is what it is named by. */
export const SHOW_FAILURES = 'Show the failures';

/** How long the failure the badge was pressed to show stays lit, in milliseconds. */
const SOUGHT_LIT = 2000;

/** What **In progress** says where nothing is. */
export const NOTHING_IN_PROGRESS = 'Nothing is running.';

/** What **Earlier** says where nothing has ended. */
export const NOTHING_EARLIER = 'Nothing has finished since the server started.';

/**
 * The unlock the panel offers while the Library is locked: the button, whether
 * a press is still waiting for its answer, and what the last one came to.
 */
export interface UnlockOffer {
  /** Whether a press is waiting for the server's answer. */
  asking: boolean;
  /** What the server answered the last press with, and `null` before any. */
  said: string | null;
  /** What refused the last press, and `null` where none was. */
  refused: string | null;
  /** Presses the button. */
  ask: () => void;
}

/** What each outcome's dot is called, for a reader that cannot see its colour. */
const OUTCOME: Record<Outcome, string> = {
  success: 'Done',
  failure: 'Failed',
  notice: 'Notice',
};

/** The colour of each outcome's dot, in the colours the rest of the screen uses. */
const DOT_COLOUR: Record<Outcome, string> = {
  success: COLOR.success,
  failure: COLOR.refused,
  notice: COLOR.warn,
};

/** The parcels kept, as the row says them: `3 parcels kept, 96.0 MB`. */
function keptLine(kept: KeptParcels): string {
  const parcels = kept.count === 1 ? 'parcel' : 'parcels';
  return `${kept.count} ${parcels} kept, ${size(kept.bytes)}`;
}

const PANEL = {
  flex: '0 0 auto',
  display: 'flex',
  flexDirection: 'column',
  background: COLOR.panel,
  borderTop: `1px solid ${COLOR.border}`,
  fontSize: 13,
  color: COLOR.dim,
  minHeight: 0,
} as const;

const ROW = {
  display: 'flex',
  gap: 12,
  alignItems: 'center',
  boxSizing: 'border-box',
  minHeight: 36,
  padding: '0 8px 0 16px',
  minWidth: 0,
} as const;

const TOGGLE = {
  display: 'flex',
  gap: 6,
  alignItems: 'center',
  border: 'none',
  background: 'none',
  color: COLOR.dim,
  font: 'inherit',
  padding: 0,
  cursor: 'pointer',
} as const;

const SUMMARY = {
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
} as const;

const BADGE = {
  flex: '0 0 auto',
  padding: '2px 8px',
  borderRadius: 10,
  background: COLOR.refusedGround,
  color: COLOR.refused,
  border: 'none',
  font: 'inherit',
  fontSize: 12,
  fontWeight: 600,
  cursor: 'pointer',
} as const;

const END = {
  marginLeft: 'auto',
  display: 'flex',
  gap: 12,
  alignItems: 'center',
  flex: '0 0 auto',
} as const;

/** The expanded list: a third of the window at most, scrolling past that. */
const LIST = {
  maxHeight: '35vh',
  overflowY: 'auto',
  borderTop: `1px solid ${COLOR.border}`,
  padding: '0 8px 6px 16px',
} as const;

const HEADING = {
  margin: '8px 0 2px',
  fontSize: 11,
  fontWeight: 600,
  textTransform: 'uppercase',
  letterSpacing: '0.04em',
  color: COLOR.dim,
} as const;

const ITEMS = { listStyle: 'none', margin: 0, padding: 0 } as const;

const ITEM = {
  display: 'flex',
  gap: 10,
  alignItems: 'center',
  boxSizing: 'border-box',
  minHeight: 32,
  padding: '4px 0',
} as const;

const TIME = {
  flex: '0 0 auto',
  minWidth: 64,
  whiteSpace: 'nowrap',
  fontVariantNumeric: 'tabular-nums',
} as const;

const TEXT = { flex: 1, minWidth: 0, color: COLOR.text } as const;

/** The failure the badge was pressed to show, for the moment it is lit. */
const SOUGHT = { background: COLOR.refusedGround, outline: 'none' } as const;

const DOT = {
  flex: '0 0 auto',
  width: 6,
  height: 6,
  borderRadius: 3,
} as const;

const EMPTY = { margin: '2px 0' } as const;

/** The unlock, drawn as every framed button on this screen is. */
const BUTTON = {
  border: `1px solid ${COLOR.border}`,
  background: COLOR.panel,
  color: COLOR.text,
  font: 'inherit',
  padding: '2px 10px',
  borderRadius: 6,
  cursor: 'pointer',
} as const;

/** A Retry, and the **?**: an icon in a 28px frame. */
const ICON_BUTTON = {
  flex: '0 0 auto',
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  boxSizing: 'border-box',
  width: 28,
  height: 28,
  border: `1px solid ${COLOR.border}`,
  background: COLOR.panel,
  color: COLOR.text,
  font: 'inherit',
  padding: 0,
  borderRadius: 6,
  cursor: 'pointer',
} as const;

const TOOLTIP = {
  position: 'absolute',
  right: 0,
  bottom: 'calc(100% + 6px)',
  zIndex: 10,
  padding: '8px 10px',
  background: COLOR.panel,
  border: `1px solid ${COLOR.border}`,
  borderRadius: 8,
  boxShadow: SHADOW.menu,
  color: COLOR.dim,
  whiteSpace: 'nowrap',
} as const;
