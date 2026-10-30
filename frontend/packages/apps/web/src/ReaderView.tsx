import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';

import { getFile, isRefusal, peekFile, type Listing } from '@coffret/api';

import { drawnPages } from './drawn';
import { parentOf } from './prospective';
import { COLOR } from './theme';
import { ReconnectOffer, type Offer } from './ReconnectOffer';
import { refusedAsRanOut } from './ranOut';
import { COVER_NOT_HERE, sheetShows, sheetWanted, UNSUPPORTED, type Counter, type Sheeted, type SheetShows } from './sheet';
import { said, type Asked } from './useAsked';

/** How far ahead and behind the reader keeps pages ready. */
const PREFETCH_RADIUS = 3;

/** What the reader has to show for the picture it draws. */
type Shown =
  | { status: 'loading' }
  | { status: 'ready'; url: string }
  /** A folder's cover was peeked at and is not on this device. */
  | { status: 'absent' }
  | { status: 'failed'; message: string; ranOut: boolean };

/**
 * The reader: a dark sheet over the right of the tree, showing the row the
 * tree's focus is on.
 *
 * It follows the focus and holds no place of its own. The keys stay with the
 * tree, so `↑`/`↓` down the page rows of an open book is paging, and the
 * next row up or down — the book's folder, the next book, a file the reader
 * cannot draw — is what the sheet shows next. It binds no keys to paging:
 * `←`/`→` are the tree's, opening and closing folders, and a book bound
 * right-to-left pages the other way from one bound left-to-right, which is
 * what made arrows that turned pages confusing. What it shows (see
 * [`sheetShows`](./sheet)) is an image filling the sheet, a folder's first
 * image as a smaller cover on a stack of sheets with what the folder holds
 * under it, or a sentence for a file a browser draws nothing from.
 *
 * It leaves a strip of the tree uncovered on the left, so the rows being
 * walked stay in sight — and in reach: a click on one moves the focus, and the
 * sheet follows. It covers the tree's height and no more, which leaves the
 * Activity panel below it uncovered, where it says what is being fetched —
 * and opening the panel shortens the sheet with the tree rather than covering it.
 * `Escape` or its close button closes it; a click on it does nothing, since it
 * is where the pointer rests while somebody reads.
 *
 * A page this device does not have yet is fetched by the same request as any
 * other; what it costs is a placeholder while it is in flight, and a sentence
 * and a retry where it was refused. Neither is a screen a reader is stuck on.
 * A folder's cover is only peeked at (see [`sheetWanted`](./sheet)), so
 * walking past a shelf of books sets none of them coming; a cover not on this
 * device is said to be not here yet, on the stack where it would be drawn.
 */
export function ReaderView({
  row,
  asked,
  counter,
  discarded,
  escapable,
  onClose,
  onFetching,
  onFetched,
  reconnect,
}: {
  /** The row the tree's focus is on. */
  row: Sheeted;
  /** A folder row's own listing as far as it has been asked, which its cover and count come out of. */
  asked: Asked<Listing> | undefined;
  /** Where the row stands among the rows beside it (see [`counterOf`](./sheet)). */
  counter: Counter | null;
  discarded: number;
  /**
   * Whether `Escape` is the sheet's: not while a dialog stands over the
   * screen, whose own `Escape` it is.
   */
  escapable: boolean;
  onClose: () => void;
  onFetching: (name: string | null) => void;
  /** A file this device did not have was fetched, and the rows naming it are stale. */
  onFetched: (path: string) => void;
  /**
   * The reconnect offered where a picture was refused because the permission
   * Storage is reached through ran out, and `null` on a Library whose Storage
   * no reconnect reaches.
   */
  reconnect: Offer | null;
}) {
  // Every picture this reader has drawn, by Entry Path — see [`drawn`](./drawn),
  // which holds them and is the one place they are revoked from. Pages are
  // read and covers peeked at, out of the one cache: a cover peeked is a first
  // page already drawn, and a cover refused does not stand in the way of
  // reading that page once somebody steps into the book.
  const [drawn] = useState(() => drawnPages(getFile, peekFile));

  const [shown, setShown] = useState<Shown>({ status: 'loading' });
  const [attempt, setAttempt] = useState(0);

  const shows = sheetShows(row, asked);
  const wanted = sheetWanted(row, shows, PREFETCH_RADIUS);
  const path = wanted.shown?.path;
  const name = wanted.shown?.name;
  const remote = wanted.shown?.state === 'remote';
  const peek = wanted.peek;

  useEffect(() => {
    if (path === undefined || name === undefined) {
      return;
    }
    const held = drawn.held(path);
    if (held !== undefined) {
      setShown({ status: 'ready', url: held });
      return;
    }
    let live = true;
    setShown({ status: 'loading' });
    // A peek reaches nothing past this device's own disk, so there is no fetch
    // for the Activity panel to name.
    if (!peek) {
      onFetching(name);
    }
    void drawn.load(path, peek).then(
      (url) => {
        // Nothing to show where the answer came back to a key this device has
        // given up: it was revoked as it landed, and the picture it was is
        // already being asked for again by the attempt the discard started.
        if (!live || url === undefined) {
          return;
        }
        // The request is over either way, and the panel's line is about a
        // request in flight: cleared here rather than only when the focus moves
        // on, or the panel would go on saying "fetching" over a picture on the
        // screen.
        onFetching(null);
        setShown({ status: 'ready', url });
        if (remote && !peek) {
          // The file is on this device now, so a row still saying `remote` is
          // stale. Which rows changed is the server's to say, so the listing is
          // asked again rather than edited here.
          onFetched(path);
        }
      },
      (refused: unknown) => {
        if (live) {
          onFetching(null);
          // The one answer a peek is expected to get: the cover is not on this
          // device. Not a failure to retry — nothing was asked of Storage, and
          // asking again would get the same — and not a cue to read it either,
          // which would set the folder coming.
          if (peek && isRefusal(refused) && refused.kind === 'no_such_entry') {
            setShown({ status: 'absent' });
            return;
          }
          setShown({
            status: 'failed',
            message: said(refused),
            ranOut: refusedAsRanOut(refused),
          });
        }
      },
    );
    return () => {
      live = false;
      onFetching(null);
    };
  }, [path, name, remote, peek, attempt, drawn, onFetching, onFetched]);

  // What is kept ready follows the focus: the picture drawn and the pages
  // around it, and nothing else — a long folder walked from end to end does
  // not leave the tab holding every page of it.
  //
  // Keyed by the list's contents, spelled as one string, since the list itself
  // is a new array on every render and the effect is about which pages.
  const aheadKey = wanted.ahead === null ? null : JSON.stringify(wanted.ahead);
  useEffect(() => {
    if (aheadKey === null) {
      return;
    }
    const ahead = JSON.parse(aheadKey) as string[];
    const keep = new Set<string>(ahead);
    if (path !== undefined) {
      keep.add(path);
    }
    for (const page of ahead) {
      // A page that would not prefetch is not this row's problem. It is asked
      // for again — and answered for, on the screen — when the focus lands on
      // it.
      void drawn.load(page).catch(() => undefined);
    }
    drawn.keepOnly(keep);
  }, [aheadKey, path, drawn]);

  // What a lock takes back. The pictures this reader is holding are plaintext
  // the Master Key made, and a lock ends this server's hold on that key — so
  // they go with it: the one on the screen and every one prefetched around it.
  const gone = useRef(discarded);
  useEffect(() => {
    if (gone.current === discarded) {
      return;
    }
    gone.current = discarded;
    drawn.discard();
    // The picture on the screen was drawn from a URL that has just been
    // revoked, so it comes off in the same breath rather than one render later
    // — and what replaces it is the asking, which is what this screen shows
    // while a picture is on its way. The answer to it is the refusal, while the
    // server is locked, and the picture itself if it is not.
    setShown({ status: 'loading' });
    setAttempt((made) => made + 1);
  }, [discarded, drawn]);

  // `Escape` closes the sheet, and nothing else here is a key: the rest go to
  // the tree, whose focus the sheet follows. One something else took first —
  // the actions menu closing over the sheet — is not the sheet's.
  useEffect(() => {
    if (!escapable) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || event.defaultPrevented) {
        return;
      }
      event.preventDefault();
      onClose();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [escapable, onClose]);

  // Closing the reader is this tab dropping every picture it held.
  useEffect(() => {
    // React mounts an effect twice in development, and what holds the pictures
    // outlives both mounts, so the first one's teardown must not leave the
    // second refusing to keep what it fetches. Both run in one synchronous
    // batch, well before any request of the first can have answered.
    drawn.reopened();
    return () => drawn.closed();
  }, [drawn]);

  const where = parentOf(row.path);
  const retry = () => setAttempt((made) => made + 1);

  return (
    <section aria-label="Reader" className="sheet" style={SHEET}>
      <header style={HEADER}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <p style={{ margin: 0, fontSize: 12, color: COLOR.dim, overflowWrap: 'anywhere' }}>
            {where === '' ? 'Library' : where}
          </p>
          <h2 style={TITLE}>
            {row.kind === 'folder' ? <FolderMark /> : <FileMark />}
            <span style={{ minWidth: 0, overflowWrap: 'anywhere' }}>{row.name}</span>
          </h2>
        </div>
        {counter !== null && (
          <span aria-label="position" style={{ fontSize: 13, color: COLOR.dim, whiteSpace: 'nowrap' }}>
            {counter.at} / {counter.of}
          </span>
        )}
        <button type="button" aria-label="Close" title="Close (Esc)" onClick={onClose} style={CLOSE}>
          <svg aria-hidden width={14} height={14} viewBox="0 0 14 14">
            <path d="M2 2l10 10M12 2L2 12" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" />
          </svg>
        </button>
      </header>
      <div style={BODY}>
        <Body
          row={row}
          shows={shows}
          shown={shown}
          onRetry={retry}
          reconnect={reconnect}
        />
      </div>
    </section>
  );
}

/** The sheet under its header, for what it shows. */
function Body({
  row,
  shows,
  shown,
  onRetry,
  reconnect,
}: {
  row: Sheeted;
  shows: SheetShows;
  shown: Shown;
  onRetry: () => void;
  reconnect: Offer | null;
}) {
  switch (shows.kind) {
    case 'image':
      return (
        <Picture
          shown={shown}
          name={shows.file.name}
          alt={shows.file.name}
          remote={shows.file.state === 'remote'}
          onRetry={onRetry}
          reconnect={reconnect}
          style={{ flex: 1, minHeight: 0, maxWidth: '100%', objectFit: 'contain' }}
        />
      );
    case 'folder':
      return (
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 16, maxWidth: '100%' }}>
          <Stack>
            {shows.cover !== null ? (
              <Picture
                shown={shown}
                name={shows.cover.name}
                alt={`cover of ${row.name}`}
                // A peek brings nothing over, so its wait has nothing to explain.
                remote={false}
                onRetry={onRetry}
                reconnect={reconnect}
                style={{ maxWidth: '100%', maxHeight: '100%', objectFit: 'contain', display: 'block' }}
              />
            ) : (
              <span style={{ color: COLOR.dim, fontSize: 13, textAlign: 'center', padding: 12 }}>
                {shows.refused ?? shows.note}
              </span>
            )}
          </Stack>
          <div style={{ textAlign: 'center' }}>
            <p style={{ margin: 0, fontSize: 15, fontWeight: 600, overflowWrap: 'anywhere' }}>{row.name}</p>
            <p style={{ margin: '4px 0 0', fontSize: 13, color: COLOR.dim }}>{shows.caption}</p>
          </div>
        </div>
      );
    case 'unsupported':
      return (
        <div style={{ textAlign: 'center', padding: 16 }}>
          <p style={{ margin: 0, fontSize: 15, fontWeight: 600, overflowWrap: 'anywhere' }}>{shows.name}</p>
          <p style={{ margin: '6px 0 0', fontSize: 13, color: COLOR.dim }}>{UNSUPPORTED}</p>
        </div>
      );
  }
}

/**
 * The cover's place: two sheets fanned behind a third, which holds the cover
 * — or the line saying there is none. Smaller than a page is drawn, so a
 * folder is never taken for the first page in it.
 */
function Stack({ children }: { children: ReactNode }) {
  return (
    <div style={{ position: 'relative', width: STACK_WIDTH, height: STACK_HEIGHT, maxWidth: '100%' }}>
      {[6, -4].map((turn) => (
        <div
          key={turn}
          aria-hidden
          style={{ ...STACK_SHEET, transform: `rotate(${turn}deg)` }}
        />
      ))}
      <div
        style={{
          ...STACK_SHEET,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          overflow: 'hidden',
          background: COLOR.ground,
        }}
      >
        {children}
      </div>
    </div>
  );
}

/** One picture, through the states its request goes through. */
function Picture({
  shown,
  name,
  alt,
  remote,
  onRetry,
  reconnect,
  style,
}: {
  shown: Shown;
  /** The file's name, as the wait names it. */
  name: string;
  alt: string;
  remote: boolean;
  onRetry: () => void;
  reconnect: Offer | null;
  style: CSSProperties;
}) {
  switch (shown.status) {
    case 'loading':
      // The wait is the same request either way, and only one of the two waits
      // has anything to explain: a picture this device already has is read off
      // its own disk, and saying it was not there would be saying the opposite
      // of what the row it was opened from says (spec: EP-10).
      return (
        <p style={{ color: COLOR.dim, textAlign: 'center', margin: 0, padding: 12 }}>
          fetching {name}…
          {remote && (
            <>
              <br />
              <span style={{ fontSize: 12 }}>
                this device does not have it yet, so it is being brought over
              </span>
            </>
          )}
        </p>
      );
    case 'failed':
      return (
        <div style={{ textAlign: 'center', maxWidth: 520, padding: 16 }}>
          <p style={{ color: COLOR.refused }}>{shown.message}</p>
          {/* A picture refused because the permission ran out is one no retry
              brings back: the reconnect is offered above it, and the retry
              stays for once the permission is renewed. */}
          {shown.ranOut && reconnect !== null && (
            <p style={{ color: COLOR.text, fontSize: 13 }}>
              <ReconnectOffer offer={reconnect} />
            </p>
          )}
          <button
            onClick={onRetry}
            style={{
              border: `1px solid ${COLOR.border}`,
              background: COLOR.panel,
              color: COLOR.text,
              font: 'inherit',
              padding: '5px 14px',
              borderRadius: 4,
              cursor: 'pointer',
            }}
          >
            try again
          </button>
        </div>
      );
    case 'absent':
      return (
        <span style={{ color: COLOR.dim, fontSize: 13, textAlign: 'center', padding: 12 }}>{COVER_NOT_HERE}</span>
      );
    case 'ready':
      return <img src={shown.url} alt={alt} style={style} />;
  }
}

function FolderMark() {
  return (
    <svg aria-hidden width={16} height={16} viewBox="0 0 16 16" style={{ flex: 'none' }}>
      <path
        d="M1.5 3.5h4.2l1.5 1.5h7.3v7.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.1}
        strokeLinejoin="round"
      />
    </svg>
  );
}

function FileMark() {
  return (
    <svg aria-hidden width={16} height={16} viewBox="0 0 24 24" style={{ flex: 'none' }}>
      <path d="M6 2.5h8l4 4v15H6z" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinejoin="round" />
      <path d="M14 2.5v4h4" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinejoin="round" />
    </svg>
  );
}

/**
 * The strip of the tree the sheet leaves uncovered on its left: about 380px,
 * given back to the sheet on a narrow window so that it keeps 480px where it
 * can, and never under 64px, so a sliver of the rows being walked always
 * shows.
 */
export const SHEET_LEFT = 'max(64px, min(380px, calc(100% - 480px)))';

/** The cover's stack: about half a page's height on a laptop's screen. */
const STACK_WIDTH = 240;
const STACK_HEIGHT = 320;

const SHEET: CSSProperties = {
  position: 'absolute',
  top: 0,
  bottom: 0,
  right: 0,
  left: SHEET_LEFT,
  display: 'flex',
  flexDirection: 'column',
  background: '#0a0a0a',
  borderLeft: `1px solid ${COLOR.border}`,
  boxShadow: '-12px 0 24px rgba(0, 0, 0, 0.5)',
  zIndex: 10,
};

const HEADER: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: 12,
  padding: '10px 12px 10px 16px',
  borderBottom: `1px solid ${COLOR.border}`,
};

const TITLE: CSSProperties = {
  margin: '2px 0 0',
  display: 'flex',
  alignItems: 'center',
  gap: 6,
  fontSize: 15,
  fontWeight: 600,
};

const CLOSE: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: 28,
  height: 28,
  border: `1px solid ${COLOR.border}`,
  background: COLOR.panel,
  color: COLOR.text,
  borderRadius: 4,
  cursor: 'pointer',
};

const BODY: CSSProperties = {
  flex: 1,
  minHeight: 0,
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'center',
  padding: 12,
};

const STACK_SHEET: CSSProperties = {
  position: 'absolute',
  inset: 0,
  borderRadius: 3,
  border: `1px solid ${COLOR.border}`,
  background: COLOR.panel,
};
