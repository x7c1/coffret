import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';

import type { Added, Fill, ListedFile, Listing } from '@coffret/api';

import { droppedFiles } from './drop';
import { rowFill, type RowState } from './fill';
import { size, time } from './humanize';
import { COLOR } from './theme';

/**
 * What the current folder holds, on the right.
 *
 * Sub-folders first, then files, both in the order the server answered in — the
 * byte order of the canonical paths, which is the one order every device agrees
 * on. Nothing here re-sorts and nothing case-folds.
 *
 * Names only: no thumbnail, and every stored file appears whatever its format.
 * A row a browser draws nothing from is a row like any other, and one the
 * explorer will not offer to open.
 *
 * Each row's state is the listing's answer, with what the server is doing about
 * it over the top: `present`, `remote` and `uploading` are the listing's to say
 * and nothing here overrides them, while `fetching`, `failed` and `declined` are
 * the fill's — work in flight, which the listing has no word for.
 *
 * The list is also where files are added. Dropping them on it adds them to the
 * folder it is showing, which is the whole of the gesture: there is no upload
 * button and no dialog, because what a person means by dragging files onto a
 * folder is not in doubt. A folder no mapping of this device reaches takes no
 * drop — its rows are already inert and the banner already says why — and it
 * says so while the drag is still in the air rather than only by not reacting
 * to it: the outline a drag brings up is the refused colour there, and letting
 * go says in words that nothing was added.
 */
export function FileList({
  listing,
  fill,
  selected,
  onOpenFolder,
  onOpenFile,
  onUnsupported,
  onAdd,
  onUnmapped,
}: {
  listing: Listing;
  /** What the server is bringing over, wherever it is bringing it. */
  fill: Fill | null;
  /** The Entry Path the reader was last opened at here, if any. */
  selected: string | null;
  onOpenFolder: (path: string) => void;
  onOpenFile: (path: string) => void;
  onUnsupported: (file: ListedFile) => void;
  /** Files dropped on this folder, each with its path relative to it. */
  onAdd: (files: Added[]) => void;
  /** A drop onto a folder with nowhere on this device to put it. */
  onUnmapped: () => void;
}) {
  // Whether something is being dragged over the list right now. A `dragenter` and
  // a `dragleave` fire for every element the pointer crosses inside it, so this
  // is counted rather than set: a `dragleave` off a row onto the row below it
  // would otherwise take the outline away in the middle of the drag.
  const [over, setOver] = useState(0);
  const dragged = over > 0;
  const root = listing.path === '';
  const empty = listing.folders.length === 0 && listing.files.length === 0;
  // A Library root with no mapping of its own and no files sitting in it is the
  // ordinary shape of a device that mapped one top-level folder, and there is
  // nothing on this screen for a banner to explain: no row here is inert,
  // because the rows are folders and the folders say for themselves. Anywhere
  // else — and at a root that does hold files — unmapped is worth saying.
  const sayUnmapped = !listing.mapped && !(root && listing.files.length === 0);
  return (
    <div
      style={{
        flex: 1,
        overflow: 'auto',
        minWidth: 0,
        // Inside the outline rather than around it, so that nothing on the
        // screen moves when a drag arrives: an outline that took up room would
        // shift every row under the pointer at the moment of dropping.
        //
        // A folder this device has no folder for is outlined too, and in the
        // refused colour. The answer to "will this take my files" is worth
        // having while the files are still in the air, and a list that simply
        // did not light up would leave a person to find out by letting go.
        outline: dragged
          ? `2px dashed ${listing.mapped ? COLOR.uploading : COLOR.refused}`
          : undefined,
        outlineOffset: -2,
      }}
      // The three that have to be answered for a drop to happen at all. The
      // browser's own default for a dropped file is to navigate to it, which
      // would replace the explorer with the picture — so both of the first two
      // are prevented, and `dragover` on every pass rather than once.
      onDragEnter={(event) => {
        event.preventDefault();
        setOver((crossed) => crossed + 1);
      }}
      onDragOver={(event) => event.preventDefault()}
      onDragLeave={() => setOver((crossed) => Math.max(0, crossed - 1))}
      onDrop={(event) => {
        event.preventDefault();
        setOver(0);
        if (!listing.mapped) {
          // Said and not merely not done. A drop is a gesture with an outcome,
          // and the outcome here is that none of those files were added — which
          // a screen that goes on looking exactly as it did does not tell
          // anybody. The banner over the rows is the standing reason; this is
          // the answer to the thing that was just tried.
          onUnmapped();
          return;
        }
        // The walk is asynchronous and the event is not: what it carries is
        // gone by the first await, so the traversal is started here and the
        // answer is handed over whole.
        void droppedFiles(event.dataTransfer).then(onAdd);
      }}
    >
      {sayUnmapped && <Unmapped root={root} top={listing.path.split('/')[0]} />}
      {empty ? (
        <p style={{ padding: 16, color: COLOR.dim }}>
          {root ? 'this Library is empty' : 'this folder is empty'}
        </p>
      ) : (
        <table style={{ width: '100%', borderCollapse: 'collapse', tableLayout: 'fixed' }}>
          <thead>
            <tr style={{ color: COLOR.dim, textAlign: 'left', fontSize: 12 }}>
              <th style={{ ...HEAD, width: 28 }} />
              <th style={HEAD}>name</th>
              <th style={{ ...HEAD, width: 100, textAlign: 'right' }}>size</th>
              <th style={{ ...HEAD, width: 190 }}>modified</th>
              <th style={{ ...HEAD, width: 90 }}>state</th>
            </tr>
          </thead>
          <tbody>
            {listing.folders.map((folder) => (
              <Row
                key={folder.path}
                icon="▸"
                name={folder.name}
                onActivate={() => onOpenFolder(folder.path)}
              >
                <td style={CELL} />
                <td style={CELL} />
                <td style={CELL}>
                  {!folder.mapped && <Chip color={COLOR.warn}>not here</Chip>}
                </td>
              </Row>
            ))}
            {listing.files.map((file) => (
              <Row
                key={file.path}
                icon={file.openable ? '▣' : '▤'}
                name={file.name}
                dim={!file.openable}
                selected={file.path === selected}
                // A folder no mapping reaches has nowhere on this device to put
                // a file, so every fetch under it would be declined: its rows
                // are shown and not offered, rather than letting a reader walk
                // into the refusal.
                onActivate={
                  !listing.mapped
                    ? undefined
                    : file.openable
                      ? () => onOpenFile(file.path)
                      : () => onUnsupported(file)
                }
              >
                <td style={{ ...CELL, textAlign: 'right', color: COLOR.dim }}>
                  {size(file.size)}
                </td>
                <td style={{ ...CELL, color: COLOR.dim }}>{time(file.mtime)}</td>
                <td style={CELL}>
                  <StateChip file={file} folder={listing.path} fill={fill} />
                </td>
              </Row>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

/** What each row state is drawn in. */
const CHIP: Record<RowState, string> = {
  present: COLOR.present,
  remote: COLOR.remote,
  // In the folder and not in the Library yet, which is a state to notice rather
  // than one to worry about: the sync behind it is what ends it.
  uploading: COLOR.uploading,
  fetching: COLOR.fetching,
  // A refusal, which is what stopped the fill before it reached this row.
  failed: COLOR.refused,
  // Not a refusal of anything the reader asked for: the fill found something
  // about this one file and left it alone, which is worth noticing and is not
  // worth alarm.
  declined: COLOR.warn,
};

const HEAD: CSSProperties = {
  padding: '6px 10px',
  borderBottom: `1px solid ${COLOR.border}`,
  fontWeight: 'normal',
};

const CELL: CSSProperties = {
  padding: '5px 10px',
  borderBottom: '1px solid #202020',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
};

function Row({
  icon,
  name,
  dim,
  selected,
  onActivate,
  children,
}: {
  icon: string;
  name: string;
  dim?: boolean;
  selected?: boolean;
  onActivate?: () => void;
  children: ReactNode;
}) {
  // The row the reader was last opened at is brought back into view when the
  // reader closes over it — a folder is not scrolled by turning pages, but one
  // restored from a URL was never scrolled at all, and the marked row would be
  // somewhere below the fold with nothing to say where.
  const here = useRef<HTMLTableRowElement>(null);
  useEffect(() => {
    if (selected === true) {
      here.current?.scrollIntoView({ block: 'nearest' });
    }
  }, [selected]);

  return (
    <tr
      ref={here}
      className={onActivate === undefined ? 'row' : 'row activatable'}
      onClick={onActivate}
      title={name}
      style={selected === true ? { background: COLOR.selected } : undefined}
    >
      <td style={{ ...CELL, color: COLOR.dim, textAlign: 'center' }}>{icon}</td>
      <td style={{ ...CELL, color: dim === true ? COLOR.dim : COLOR.text }}>{name}</td>
      {children}
    </tr>
  );
}

/**
 * What one row's state is, which is the listing's answer and the fill's
 * together.
 *
 * The sentence rides on the chip rather than on the row: a row's own `title` is
 * its name, and a file whose fetch was declined has something to say that a
 * name is not.
 */
function StateChip({
  file,
  folder,
  fill,
}: {
  file: ListedFile;
  folder: string;
  fill: Fill | null;
}) {
  const shown = rowFill(file, folder, fill);
  return (
    <Chip color={CHIP[shown.state]} title={shown.message}>
      {shown.state}
    </Chip>
  );
}

function Chip({
  color,
  title,
  children,
}: {
  color: string;
  title?: string | null;
  children: ReactNode;
}) {
  return (
    <span
      title={title ?? undefined}
      style={{
        display: 'inline-block',
        padding: '1px 7px',
        borderRadius: 9,
        border: `1px solid ${color}`,
        color,
        fontSize: 11,
      }}
    >
      {children}
    </span>
  );
}

/**
 * Said over the rows, because it is true of every one of them.
 *
 * It stays at the top of the list as the list scrolls: it is the only answer
 * the rows below it have for why clicking one does nothing, and a reason that
 * scrolled away would leave a screenful of names that are simply inert.
 *
 * The Library root is not the same sentence. A device with a mapping for one
 * top-level folder and no root mapping has an unmapped root and a perfectly
 * ordinary Library under it (spec: EP-9), so the root says what is actually
 * true of it — files directly in it have nowhere to go — rather than telling a
 * reader on their first screen that their Library is not here.
 *
 * What it tells a reader to map is the top-level folder and not this one. A
 * mapping is keyed by one top-level component of the Library (spec: EP-9), so
 * `coffret map` on `books/vol-1` is refused as a subtree no mapping can stand
 * for; it is `books` that has to be given a folder on this device.
 */
function Unmapped({ root, top }: { root: boolean; top: string }) {
  return (
    <p
      style={{
        position: 'sticky',
        top: 0,
        zIndex: 1,
        margin: 0,
        padding: '8px 12px',
        background: '#2a2413',
        borderBottom: `1px solid ${COLOR.warn}`,
        color: COLOR.warn,
        fontSize: 13,
      }}
    >
      {root ? (
        <>
          the Library root is not mapped on this device — files sitting directly in it
          cannot be fetched, though a folder below can be mapped on its own
        </>
      ) : (
        <>
          this folder is not on this device — map <code>{top}</code> with{' '}
          <code>coffret map</code> to fetch its files
        </>
      )}
    </p>
  );
}
