import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';

import {
  peekFile,
  readAhead,
  type DeleteTarget,
  type Library,
  type LibraryTotals,
  type ListedFile,
  type Listing,
} from '@coffret/api';

import { deleteTargetOf } from './actions';
import { deleteItems } from './deleteEntries';
import {
  counted,
  extensionOf,
  fileOnDevice,
  filesOnDevice,
  firstImage,
  folderOnDevice,
  FOLDER,
  itemsIn,
  kindOf,
  LIBRARY,
  nameless,
  NOT_IN_LIBRARY_YET,
  NOT_PACKED_YET,
  opensInReader,
  selectedHeading,
  storedOn,
  summaryOf,
  type Described,
} from './details';
import type { RowFill } from './fill';
import { grouped, size, time } from './humanize';
import { LIBRARY_ROOT } from './location';
import { parentOf } from './prospective';
import { COLOR } from './theme';
import type { FileRow } from './tree';
import type { Asked } from './useAsked';

/**
 * The details pane, on the right of the tree: what the selected row is, said
 * in full where the tree has a line for it.
 *
 * It follows the selection — the focused row, or the rows selected together
 * with modifier keys (see [`selection`](./selection)). With none, it describes
 * the Library itself, laid out as a folder is (see [`TheLibrary`](#TheLibrary)).
 * With several, it says what they come to together (see
 * [`summaryOf`](./details)) and offers `Delete <n> items…`, the same deletion
 * as the actions menu's. For one row, from the top:
 *
 * - a preview: an image file's picture; for a folder holding images, its first
 *   one — for a scanned book, nearly always the cover; and otherwise a plain
 *   placeholder, the extension of a file or the item count of a folder. A
 *   file's picture is drawn only where its bytes are on this device already,
 *   asked for as a peek (see [`peekFile`](../../../gateway/api/src/file.ts)):
 *   moving the focus down a list is not opening anything, so it fetches
 *   nothing from Storage and sets no folder coming. A folder's cover this
 *   device does not have is read ahead (see
 *   [`readAhead`](../../../gateway/api/src/file.ts), spec: LA-33): that one
 *   image is fetched and none of the folder is set coming, so walking past a
 *   shelf of folders shows each cover and fetches nothing more — and while it
 *   is on its way the preview says it is loading rather than drawing the
 *   folder as having no cover;
 * - where the row stands, its name, and Open where the reader can show the row
 *   ([`opensInReader`](./details)) — what `Enter` does there: the reader's
 *   sheet on the row;
 * - the facts, in words that fit any kind of file: Kind, Items (a folder's),
 *   Size (a folder's being everything below it), Modified (a folder's being
 *   the newest file's anywhere below it), and whether it is on this device.
 *
 * A fixed column on a wide window. Below a narrow one it is hidden by
 * `index.html` and the tree keeps the whole width.
 */
export function DetailsPane({
  selected,
  library,
  listingOf,
  shownOf,
  uploadStoppedOf,
  onOpen,
  onDelete,
  onFetched,
}: {
  /** The rows selected, top to bottom; the focused row alone where nothing else is. */
  selected: readonly Described[];
  /** What the pane says of the Library where no row is selected. */
  library: LibraryShown;
  /** A folder's own listing as far as it has been asked; `undefined` where it has not. */
  listingOf: (folder: string) => Asked<Listing> | undefined;
  /** What a file row shows, the listing's state and the fill's together. */
  shownOf: (row: FileRow) => RowFill;
  /** Whether the run that would carry a file into the Library stopped (see [`uploadStopped`](./details)). */
  uploadStoppedOf: (file: ListedFile) => boolean;
  /** Open: what `Enter` does on the row. */
  onOpen: (row: Described) => void;
  /** `Delete <n> items…` over several rows: the deletion the actions menu asks for. */
  onDelete: (target: DeleteTarget) => void;
  /**
   * A folder's cover read ahead has landed: the file is on this device now, so
   * the rows still saying `remote` are stale and asked again, as the reader's
   * sheet has them asked once it fetches.
   */
  onFetched: (path: string) => void;
}) {
  return (
    <aside className="details" aria-label="Details" style={PANE}>
      {selected.length === 0 ? (
        <TheLibrary shown={library} />
      ) : selected.length === 1 ? (
        <One
          row={selected[0]}
          asked={selected[0].kind === 'folder' ? listingOf(selected[0].path) : undefined}
          shownOf={shownOf}
          uploadStoppedOf={uploadStoppedOf}
          onOpen={onOpen}
          onFetched={onFetched}
        />
      ) : (
        <Several rows={selected} onDelete={onDelete} />
      )}
    </aside>
  );
}

/** What the pane is told of the Library, each as far as it has been asked. */
export interface LibraryShown {
  /** Which Library this is: its name and its provider. */
  library: Asked<Library>;
  /** What the whole of it comes to (spec: LA-39). */
  totals: Asked<LibraryTotals>;
  /** The root's listing, whose rows are the Library's items. */
  root: Asked<Listing>;
}

/**
 * The Library itself, where no row is focused: drawn as a folder's details
 * are, with a box in place of a folder's mark, its name with no location
 * above it — it is the top, and stands nowhere — and the facts a folder has,
 * over the whole Library, with three of its own: which Storage it is kept on,
 * how many objects it keeps there, and how much of this device's disk it
 * takes: the files here and the parcels held for reading again.
 *
 * Each fact is `…` while what it is read from is out, and `—` where that was
 * refused, as a folder's Items is: a count that will not come is not drawn as
 * one still coming.
 */
function TheLibrary({ shown }: { shown: LibraryShown }) {
  const library = shown.library.status === 'ready' ? shown.library.value : null;
  const totals = shown.totals.status === 'ready' ? shown.totals.value : null;
  const root = shown.root.status === 'ready' ? shown.root.value : null;
  const awaited = (asked: Asked<unknown>) => (asked.status === 'failed' ? '—' : '…');
  const fromLibrary = (say: (library: Library) => string) =>
    library !== null ? say(library) : awaited(shown.library);
  const fromTotals = (say: (totals: LibraryTotals) => string) =>
    totals !== null ? say(totals) : awaited(shown.totals);

  const facts: [string, ReactNode][] = [
    ['Kind', LIBRARY],
    ['Stored on', fromLibrary((one) => storedOn(one.provider))],
    ['Objects on Storage', fromTotals((all) => grouped(all.objects))],
    ['Items', root !== null ? grouped(itemsIn(root)) : awaited(shown.root)],
    ['Files', fromTotals((all) => grouped(all.files_total))],
    ['Size', fromTotals((all) => size(all.size))],
    ['Modified', fromTotals((all) => time(all.mtime))],
    ['On this device', fromTotals((all) => filesOnDevice(all.files_present, all.files_total))],
    ['Size on this device', fromTotals((all) => size(all.size_on_device))],
  ];

  return (
    <div style={BLOCKS}>
      <div style={PREVIEW}>
        <Placeholder mark={<BoxMark />} label={root === null ? '' : counted(itemsIn(root), 'item', 'items')} />
      </div>
      {library === null ? (
        <h2 style={{ ...TITLE, color: COLOR.dim }}>{shown.library.status === 'failed' ? shown.library.message : '…'}</h2>
      ) : library.name === null ? (
        <h2 style={{ ...TITLE, color: COLOR.dim }}>{nameless(library)}</h2>
      ) : (
        <h2 style={TITLE}>{library.name}</h2>
      )}
      <Facts facts={facts} />
    </div>
  );
}

/** One row, described. */
function One({
  row,
  asked,
  shownOf,
  uploadStoppedOf,
  onOpen,
  onFetched,
}: {
  row: Described;
  /** The folder's own listing as far as it has been asked, for a folder row. */
  asked: Asked<Listing> | undefined;
  shownOf: (row: FileRow) => RowFill;
  uploadStoppedOf: (file: ListedFile) => boolean;
  onOpen: (row: Described) => void;
  onFetched: (path: string) => void;
}) {
  const listing = asked?.status === 'ready' ? asked.value : null;
  // The picture to draw, if any: the file itself, or a folder's first image.
  const image = row.kind === 'file' ? (row.file.openable ? row.file : null) : listing === null ? null : firstImage(listing);
  // A file's from the bytes this device has alone, since a peek never
  // fetches; a folder's cover read ahead where this device does not have it.
  const remote = image !== null && image.state === 'remote';
  const ahead = remote && row.kind === 'folder';
  const picture = usePicture(image !== null && (!remote || ahead) ? image.path : null, ahead, onFetched);
  const url = picture.status === 'ready' ? picture.url : null;
  const folder = row.kind === 'folder' ? row : null;
  const where = parentOf(row.path);

  const facts: [string, ReactNode][] = [];
  if (row.kind === 'file') {
    facts.push(['Kind', kindOf(row.name)]);
    facts.push(['Size', size(row.file.size)]);
    facts.push(['Modified', time(row.file.mtime)]);
    facts.push(['On this device', fileOnDevice(shownOf(row), uploadStoppedOf(row.file))]);
  } else {
    facts.push(['Kind', FOLDER]);
    // `…` while the listing is out, and `—` where it was refused: a count that
    // will not come is not drawn as one still coming.
    facts.push(['Items', listing !== null ? String(itemsIn(listing)) : asked?.status === 'failed' ? '—' : '…']);
    // A prepared folder and one waiting to be packed have nothing in the
    // Library to measure; the sentence above the facts says so instead.
    if (!row.prospective && !row.stranded) {
      facts.push(['Size', size(row.folder.size)]);
      facts.push(['Modified', time(row.folder.mtime)]);
      facts.push(['On this device', folderOnDevice(row.folder)]);
    }
  }

  return (
    <div style={BLOCKS}>
      <div style={PREVIEW}>
        {url !== null ? (
          <img src={url} alt="" style={{ maxWidth: '100%', maxHeight: '100%', objectFit: 'contain' }} />
        ) : folder !== null ? (
          <Placeholder
            mark={<FolderMark />}
            label={
              picture.status === 'loading' && ahead
                ? COVER_LOADING
                : listing === null
                  ? ''
                  : counted(itemsIn(listing), 'item', 'items')
            }
          />
        ) : (
          <Placeholder mark={<SheetMark />} label={extensionOf(row.name)?.toUpperCase() ?? ''} />
        )}
      </div>
      <div>
        <p style={{ margin: 0, color: COLOR.dim, fontSize: 12, overflowWrap: 'anywhere' }}>
          {where === '' ? LIBRARY_ROOT : where}
        </p>
        <h2 style={{ ...TITLE, margin: '2px 0 0' }}>{row.name}</h2>
      </div>
      {opensInReader(row, listing) && (
        <button type="button" onClick={() => onOpen(row)} style={OPEN}>
          Open
        </button>
      )}
      {folder !== null && (folder.prospective || folder.stranded) && (
        <p style={{ margin: 0, color: COLOR.accentDeep, fontSize: 13 }}>
          {folder.prospective ? NOT_IN_LIBRARY_YET : NOT_PACKED_YET}
        </p>
      )}
      <Facts facts={facts} />
    </div>
  );
}

/**
 * Several rows, as what they come to together, and the one deletion of them
 * all — of the ones the Library holds, which is where the count on the button
 * can be smaller than the heading's (see [`deleteTargetOf`](./actions)).
 */
function Several({ rows, onDelete }: { rows: readonly Described[]; onDelete: (target: DeleteTarget) => void }) {
  const summary = summaryOf(rows);
  const target = deleteTargetOf(rows);
  return (
    <div style={BLOCKS}>
      <div style={PREVIEW}>
        <StackedSheets count={summary.count} />
      </div>
      <h2 style={TITLE}>{selectedHeading(summary.count)}</h2>
      <Facts
        facts={[
          ['Folders', String(summary.folders)],
          ['Files', String(summary.files)],
          ['Size', size(summary.bytes)],
        ]}
      />
      {target !== null && (
        <button type="button" onClick={() => onDelete(target)} style={DELETE_BUTTON}>
          {deleteItems(target)}
        </button>
      )}
    </div>
  );
}

/** The two-column list of facts: a name, and what it is. */
function Facts({ facts }: { facts: readonly [string, ReactNode][] }) {
  return (
    <dl style={{ margin: 0, display: 'grid', gridTemplateColumns: 'auto 1fr', columnGap: 12, rowGap: 10, fontSize: 13 }}>
      {facts.map(([name, value]) => (
        <div key={name} style={{ display: 'contents' }}>
          <dt style={{ color: COLOR.dim }}>{name}</dt>
          <dd style={{ margin: 0, overflowWrap: 'anywhere' }}>{value}</dd>
        </div>
      ))}
    </dl>
  );
}

/**
 * What the preview says over a folder whose cover is on its way from Storage,
 * in the word the reader's sheet and the Activity panel use for it.
 */
export const COVER_LOADING = 'fetching cover…';

/** Where the preview's picture stands. */
type Picture =
  /** No picture is asked for, or the one asked for was refused. */
  | { status: 'none' }
  /** It is on its way. */
  | { status: 'loading' }
  /** It is here, as a URL an `<img>` draws. */
  | { status: 'ready'; url: string };

/**
 * The picture at `path` as a URL an `<img>` draws: peeked, or read ahead where
 * `ahead` says — a folder's cover this device does not have — and `none`
 * where there is no path or it was refused.
 *
 * Asked for rather than pointed at, as the reader's pages are: the request
 * carries the tab's session, which an `<img src>` would not. Let go of when the
 * path changes — a focus walking down a list asks for each row it passes and
 * keeps only the last — and the URL revoked with it. A refusal draws the
 * placeholder: the pane has nothing to say about a preview that did not come
 * that the row does not already say.
 *
 * Asked again only when the path changes, and not when `ahead` does: a cover
 * read ahead that lands is handed to `fetched`, which has the listing asked
 * again, and that listing says the cover is on this device now — the picture
 * already drawn is that file.
 */
function usePicture(path: string | null, ahead: boolean, fetched: (path: string) => void): Picture {
  const [picture, setPicture] = useState<{ path: string; url: string | null } | null>(null);
  const asksAhead = useRef(ahead);
  asksAhead.current = ahead;
  const landed = useRef(fetched);
  landed.current = fetched;
  useEffect(() => {
    if (path === null) {
      setPicture(null);
      return;
    }
    const aborter = new AbortController();
    let url: string | null = null;
    const fetches = asksAhead.current;
    (fetches ? readAhead : peekFile)(path, aborter.signal).then(
      (blob) => {
        if (aborter.signal.aborted) {
          return;
        }
        url = URL.createObjectURL(blob);
        setPicture({ path, url });
        if (fetches) {
          landed.current(path);
        }
      },
      () => {
        if (!aborter.signal.aborted) {
          setPicture({ path, url: null });
        }
      },
    );
    return () => {
      aborter.abort();
      if (url !== null) {
        URL.revokeObjectURL(url);
      }
    };
  }, [path]);
  if (path === null) {
    return { status: 'none' };
  }
  if (picture === null || picture.path !== path) {
    return { status: 'loading' };
  }
  return picture.url === null ? { status: 'none' } : { status: 'ready', url: picture.url };
}

/** A large mark and a word under it, where there is no picture to draw. */
function Placeholder({ mark, label }: { mark: ReactNode; label: string }) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 8, color: COLOR.dim }}>
      {mark}
      {label !== '' && <span style={{ fontSize: 13, fontWeight: 600, letterSpacing: 0.5 }}>{label}</span>}
    </div>
  );
}

function SheetMark() {
  return (
    <svg aria-hidden width={56} height={56} viewBox="0 0 24 24">
      <path d="M6 2.5h8l4 4v15H6z" fill="none" stroke="currentColor" strokeWidth={1.2} strokeLinejoin="round" />
      <path d="M14 2.5v4h4" fill="none" stroke="currentColor" strokeWidth={1.2} strokeLinejoin="round" />
    </svg>
  );
}

/** A box with its lid on: the Library, which holds everything the tree lists. */
function BoxMark() {
  return (
    <svg aria-hidden width={64} height={64} viewBox="0 0 16 16">
      <path
        d="M1.5 4.5h13v2h-13zM2.5 6.5v6.5a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1v-6.5M6.5 9h3"
        fill="none"
        stroke="currentColor"
        strokeWidth={0.8}
        strokeLinejoin="round"
        strokeLinecap="round"
      />
    </svg>
  );
}

function FolderMark() {
  return (
    <svg aria-hidden width={64} height={64} viewBox="0 0 16 16">
      <path
        d="M1.5 3.5h4.2l1.5 1.5h7.3v7.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z"
        fill="none"
        stroke="currentColor"
        strokeWidth={0.8}
        strokeLinejoin="round"
      />
    </svg>
  );
}

/** Three sheets fanned behind one another, and how many rows are selected on the front one. */
function StackedSheets({ count }: { count: number }) {
  return (
    <svg aria-hidden width={96} height={96} viewBox="0 0 48 48">
      {[
        { x: 14, y: 6, turn: 8 },
        { x: 12, y: 8, turn: -6 },
      ].map(({ x, y, turn }) => (
        <rect
          key={turn}
          x={x}
          y={y}
          width={22}
          height={30}
          rx={2}
          transform={`rotate(${turn} 24 24)`}
          fill={COLOR.panel}
          stroke={COLOR.border}
          strokeWidth={1}
        />
      ))}
      <rect x={13} y={10} width={22} height={30} rx={2} fill={COLOR.ground} stroke={COLOR.dim} strokeWidth={1} />
      <text x={24} y={29} textAnchor="middle" fontSize={11} fontWeight={600} fill={COLOR.text}>
        {count}
      </text>
    </svg>
  );
}

/** About 320px: room for a cover at a readable size, and the tree keeps the rest. */
const WIDTH = 320;

/** The room around the pane's blocks. */
const PAD = 20;

/** On the ground, as the tree beside it is: the panels are what stands over the two. */
const PANE: CSSProperties = {
  flex: `0 0 ${WIDTH}px`,
  width: WIDTH,
  boxSizing: 'border-box',
  overflowY: 'auto',
  borderLeft: `1px solid ${COLOR.border}`,
  background: COLOR.ground,
};

/** The pane's blocks, top to bottom: the preview, the name, Open, the facts. */
const BLOCKS: CSSProperties = { display: 'flex', flexDirection: 'column', gap: 16, padding: PAD };

const TITLE: CSSProperties = { margin: 0, fontSize: 18, fontWeight: 700, overflowWrap: 'anywhere' };

const PREVIEW: CSSProperties = {
  height: 220,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  background: COLOR.panel,
  border: `1px solid ${COLOR.border}`,
  borderRadius: 8,
  overflow: 'hidden',
};

/** A button of the pane: as high as the header's, so a finger finds it as easily. */
const PANE_BUTTON: CSSProperties = {
  alignSelf: 'flex-start',
  minHeight: 32,
  font: 'inherit',
  fontSize: 14,
  fontWeight: 600,
  padding: '0 16px',
  borderRadius: 8,
  cursor: 'pointer',
};

/** Drawn in red on its tint, as the actions menu draws a deletion. */
const DELETE_BUTTON: CSSProperties = {
  ...PANE_BUTTON,
  border: `1px solid ${COLOR.refused}`,
  background: COLOR.refusedGround,
  color: COLOR.refused,
};

/** Filled with the accent: the one thing the pane offers to do. */
const OPEN: CSSProperties = {
  ...PANE_BUTTON,
  border: 'none',
  background: COLOR.accent,
  color: COLOR.onAccent,
};
