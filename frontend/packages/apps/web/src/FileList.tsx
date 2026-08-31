import { useEffect, useRef, type CSSProperties, type ReactNode } from 'react';

import type { ListedFile, Listing } from '@coffret/api';

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
 */
export function FileList({
  listing,
  selected,
  onOpenFolder,
  onOpenFile,
  onUnsupported,
}: {
  listing: Listing;
  /** The Entry Path the reader was last opened at here, if any. */
  selected: string | null;
  onOpenFolder: (path: string) => void;
  onOpenFile: (path: string) => void;
  onUnsupported: (file: ListedFile) => void;
}) {
  const root = listing.path === '';
  const empty = listing.folders.length === 0 && listing.files.length === 0;
  // A Library root with no mapping of its own and no files sitting in it is the
  // ordinary shape of a device that mapped one top-level folder, and there is
  // nothing on this screen for a banner to explain: no row here is inert,
  // because the rows are folders and the folders say for themselves. Anywhere
  // else — and at a root that does hold files — unmapped is worth saying.
  const sayUnmapped = !listing.mapped && !(root && listing.files.length === 0);
  return (
    <div style={{ flex: 1, overflow: 'auto', minWidth: 0 }}>
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
                  <Chip color={file.state === 'present' ? COLOR.present : COLOR.remote}>
                    {file.state}
                  </Chip>
                </td>
              </Row>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

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

function Chip({ color, children }: { color: string; children: ReactNode }) {
  return (
    <span
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
