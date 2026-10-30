import { useEffect, useState, type CSSProperties, type ReactNode } from 'react';

import { peekFile, type DeleteTarget, type ListedFile, type Listing } from '@coffret/api';

import { deleteTargetOf } from './actions';
import { deleteItems } from './deleteEntries';
import {
  counted,
  extensionOf,
  fileOnDevice,
  firstImage,
  folderOnDevice,
  FOLDER,
  itemsIn,
  kindOf,
  NOT_IN_LIBRARY_YET,
  NOT_PACKED_YET,
  opensInReader,
  selectedHeading,
  summaryOf,
  type Described,
} from './details';
import type { RowFill } from './fill';
import { size, time } from './humanize';
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
 * with modifier keys (see [`selection`](./selection)). With several, it says
 * what they come to together (see [`summaryOf`](./details)) and offers
 * `Delete <n> items…`, the same deletion as the actions menu's. For one row,
 * from the top:
 *
 * - a preview: an image file's picture; for a folder holding images, its first
 *   one — for a scanned book, nearly always the cover; and otherwise a plain
 *   placeholder, the extension of a file or the item count of a folder. The
 *   picture is drawn only where its bytes are on this device already, asked
 *   for as a peek (see [`peekFile`](../../../gateway/api/src/file.ts)): moving
 *   the focus down a list is not opening anything, so it fetches nothing from
 *   Storage and sets no folder coming;
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
  listingOf,
  shownOf,
  uploadStoppedOf,
  onOpen,
  onDelete,
}: {
  /** The rows selected, top to bottom; the focused row alone where nothing else is. */
  selected: readonly Described[];
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
}) {
  return (
    <aside className="details" aria-label="Details" style={PANE}>
      {selected.length === 0 ? (
        <p style={{ margin: 0, padding: 16, color: COLOR.dim, fontSize: 13 }}>Nothing selected</p>
      ) : selected.length === 1 ? (
        <One
          row={selected[0]}
          asked={selected[0].kind === 'folder' ? listingOf(selected[0].path) : undefined}
          shownOf={shownOf}
          uploadStoppedOf={uploadStoppedOf}
          onOpen={onOpen}
        />
      ) : (
        <Several rows={selected} onDelete={onDelete} />
      )}
    </aside>
  );
}

/** One row, described. */
function One({
  row,
  asked,
  shownOf,
  uploadStoppedOf,
  onOpen,
}: {
  row: Described;
  /** The folder's own listing as far as it has been asked, for a folder row. */
  asked: Asked<Listing> | undefined;
  shownOf: (row: FileRow) => RowFill;
  uploadStoppedOf: (file: ListedFile) => boolean;
  onOpen: (row: Described) => void;
}) {
  const listing = asked?.status === 'ready' ? asked.value : null;
  // The picture to draw, if any: the file itself, or a folder's first image.
  const image = row.kind === 'file' ? (row.file.openable ? row.file : null) : listing === null ? null : firstImage(listing);
  // Only from the bytes this device has: a peek never fetches.
  const here = image !== null && image.state !== 'remote';
  const url = usePeek(here ? image.path : null);
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
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12, padding: 16 }}>
      <div style={PREVIEW}>
        {url !== null ? (
          <img src={url} alt="" style={{ maxWidth: '100%', maxHeight: '100%', objectFit: 'contain' }} />
        ) : folder !== null ? (
          <Placeholder mark={<FolderMark />} label={listing === null ? '' : counted(itemsIn(listing), 'item', 'items')} />
        ) : (
          <Placeholder mark={<SheetMark />} label={extensionOf(row.name)?.toUpperCase() ?? ''} />
        )}
      </div>
      <div>
        <p style={{ margin: 0, color: COLOR.dim, fontSize: 12, overflowWrap: 'anywhere' }}>
          {where === '' ? LIBRARY_ROOT : where}
        </p>
        <h2 style={{ margin: '2px 0 0', fontSize: 16, fontWeight: 600, overflowWrap: 'anywhere' }}>{row.name}</h2>
      </div>
      {opensInReader(row, listing) && (
        <button type="button" onClick={() => onOpen(row)} style={OPEN}>
          Open
        </button>
      )}
      {folder !== null && (folder.prospective || folder.stranded) && (
        <p style={{ margin: 0, color: COLOR.added, fontSize: 13 }}>
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
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12, padding: 16 }}>
      <div style={PREVIEW}>
        <StackedSheets count={summary.count} />
      </div>
      <h2 style={{ margin: 0, fontSize: 16, fontWeight: 600 }}>{selectedHeading(summary.count)}</h2>
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
    <dl style={{ margin: 0, display: 'grid', gridTemplateColumns: 'auto 1fr', columnGap: 12, rowGap: 6, fontSize: 13 }}>
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
 * The picture at `path`, peeked, as a URL an `<img>` draws; `null` while it is
 * out, where it was refused, and where there is no path.
 *
 * Asked for rather than pointed at, as the reader's pages are: the request
 * carries the tab's session, which an `<img src>` would not. Let go of when the
 * path changes — a focus walking down a list asks for each row it passes and
 * keeps only the last — and the URL revoked with it. A refusal draws the
 * placeholder: the pane has nothing to say about a preview that did not come
 * that the row does not already say.
 */
function usePeek(path: string | null): string | null {
  const [peeked, setPeeked] = useState<{ path: string; url: string } | null>(null);
  useEffect(() => {
    if (path === null) {
      setPeeked(null);
      return;
    }
    const aborter = new AbortController();
    let url: string | null = null;
    peekFile(path, aborter.signal).then(
      (blob) => {
        if (aborter.signal.aborted) {
          return;
        }
        url = URL.createObjectURL(blob);
        setPeeked({ path, url });
      },
      () => {
        if (!aborter.signal.aborted) {
          setPeeked(null);
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
  return peeked !== null && peeked.path === path ? peeked.url : null;
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

const PANE: CSSProperties = {
  flex: `0 0 ${WIDTH}px`,
  width: WIDTH,
  overflowY: 'auto',
  borderLeft: `1px solid ${COLOR.border}`,
  background: COLOR.panel,
};

const PREVIEW: CSSProperties = {
  height: 220,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  background: COLOR.ground,
  border: `1px solid ${COLOR.border}`,
  borderRadius: 4,
  overflow: 'hidden',
};

/** Drawn in light red, as the actions menu draws a deletion. */
const DELETE_BUTTON: CSSProperties = {
  alignSelf: 'flex-start',
  border: `1px solid ${COLOR.refused}`,
  background: 'transparent',
  color: COLOR.refused,
  font: 'inherit',
  fontSize: 13,
  padding: '4px 16px',
  borderRadius: 4,
  cursor: 'pointer',
};

const OPEN: CSSProperties = {
  alignSelf: 'flex-start',
  border: `1px solid ${COLOR.border}`,
  background: COLOR.selected,
  color: COLOR.text,
  font: 'inherit',
  fontSize: 13,
  padding: '4px 16px',
  borderRadius: 4,
  cursor: 'pointer',
};
