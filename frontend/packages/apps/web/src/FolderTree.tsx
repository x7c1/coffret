import { useEffect, useRef, useState, type DragEvent } from 'react';

import type { DeleteTarget } from '@coffret/api';

import { AWAITING, NEW_FOLDER, parentOf } from './awaiting';
import { DropBanner, dropOutline } from './Banner';
import { DELETE } from './deleteEntries';
import { useDragOver } from './dragOver';
import { droppedFiles, type Drop } from './drop';
import { dropOutcome } from './dropTarget';
import { RowMenu, type MenuItem } from './RowMenu';
import { COLOR } from './theme';
import { ancestry, nest, type FolderNode } from './tree';

/**
 * The Library's folders, down the left.
 *
 * Always there and folders only: a Library has folders exactly where a current
 * Entry stands under one, so a Library holding nothing shows an empty tree
 * rather than an error. The root is a row of its own, because the Library root
 * is a place to stand and not a folder anything named.
 *
 * The folder of a book whose freeze has not committed is drawn too, dimmed:
 * its pages are on the disk and the Library does not name it yet (see
 * [`strandedFolders`](./stranded)), so "not in the Library yet" is something a
 * person can see rather than something they have to remember.
 *
 * Every row is a drop target for its own folder, as the file list is for the
 * one it shows: a drag over a row brings up the same outline and the same line
 * saying what letting go will do — the refusal included, where no mapping of
 * this device reaches the folder — and a drop on it is confirmed by the screen
 * before anything is sent, as every drop is. A drop is also how a folder comes
 * to be: a folder dropped onto a row lands under its own name inside that
 * row's folder, and the confirmation says where.
 *
 * So are the folders the Library does not hold yet (see
 * [`awaiting`](./awaiting)) — a path prepared with "New folder…", and a
 * top-level folder this device maps with nothing under it — marked as such,
 * so that a place to drop the first files into is somewhere a person can see
 * and walk into.
 *
 * A book — a folder with no folders under it, which is where pages are — can
 * be deleted from its row's "⋮", as it can from its row in the file list: the
 * same target handed over, so the same question asked. Not a stranded folder,
 * nor one the Library does not hold yet, which the Library has nothing of to
 * take out.
 *
 * Every row a mapping of this device reaches offers "New folder…" from its
 * "⋮", the root's "Library" among them — in a Library with nothing in it, that
 * is the one row there is, and the way to prepare a path right under the root.
 */
export function FolderTree({
  folders,
  stranded,
  awaiting,
  current,
  onOpen,
  onDelete,
  onNewFolder,
  drops,
}: {
  folders: readonly string[];
  /** The folders of books whose freeze has not committed, which the Library does not hold yet. */
  stranded: readonly string[];
  /**
   * The folders drawn that the Library does not hold yet and that are places
   * to drop into: prepared here, or mapped on this device with nothing under
   * them. Each is among `folders` too.
   */
  awaiting: readonly string[];
  current: string;
  onOpen: (folder: string) => void;
  /** "Delete…" was chosen from a book's menu: the same as the file list's folder menu. */
  onDelete: (target: DeleteTarget) => void;
  /** "New folder…" was chosen from a row's menu, for a folder inside that row's. */
  onNewFolder: (parent: string) => void;
  /** What a drop onto a row reaches out to. */
  drops: TreeDrops;
}) {
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(
    () => new Set(ancestry(current)),
  );

  // Whatever the screen was restored to has to be visible in the tree, so every
  // folder on the way down to it is opened — including on a reload, where the
  // URL named a folder several components deep and nothing has been clicked.
  useEffect(() => {
    setExpanded((open) => {
      const path = ancestry(current);
      if (path.every((folder) => open.has(folder))) {
        return open;
      }
      return new Set([...open, ...path]);
    });
  }, [current]);

  // And a folder just prepared is brought into view too, wherever it was
  // prepared from: "New folder…" on a row of the list leaves the tree's branch
  // above it as it was, and a row that appeared nowhere would be a folder
  // somebody has to go looking for. Only the ones not drawn before, so that a
  // branch somebody closed stays closed.
  const seen = useRef<ReadonlySet<string>>(new Set(awaiting));
  useEffect(() => {
    const fresh = awaiting.filter((path) => !seen.current.has(path));
    seen.current = new Set(awaiting);
    if (fresh.length === 0) {
      return;
    }
    setExpanded((open) => {
      const above = fresh.flatMap((path) => ancestry(parentOf(path)));
      if (above.every((folder) => open.has(folder))) {
        return open;
      }
      return new Set([...open, ...above]);
    });
  }, [awaiting]);

  const toggle = (path: string) =>
    setExpanded((open) => {
      const next = new Set(open);
      if (!next.delete(path)) {
        next.add(path);
      }
      return next;
    });

  // The column this stands in is the screen's and not the tree's: it is there
  // while the folders are still being asked for, so that nothing moves across
  // the screen when they arrive.
  const waiting = new Set(stranded);
  const unheld = new Set(awaiting);
  return (
    <nav aria-label="folders" style={{ padding: '8px 0' }}>
      {/* No mark: the root is the place the tree hangs from, and what it is
          called is in its name. */}
      <Row
        label="Library"
        path=""
        depth={0}
        selected={current === ''}
        onOpen={() => onOpen('')}
        menu={newFolderOf('', drops, onNewFolder)}
        drops={drops}
      />
      {nest(folders).map((node) => (
        <Branch
          key={node.path}
          node={node}
          depth={1}
          current={current}
          expanded={expanded}
          stranded={waiting}
          awaiting={unheld}
          onToggle={toggle}
          onOpen={onOpen}
          onDelete={onDelete}
          onNewFolder={onNewFolder}
          drops={drops}
        />
      ))}
    </nav>
  );
}

/** Everything a drop onto a row of the tree reaches out to. */
export interface TreeDrops {
  /**
   * Whether a mapping of this device reaches a folder, or `null` where that is
   * not known yet (see [`mappedAt`](./dropTarget)). Unknown is not refused: the
   * drop asks the folder's own listing before anything is sent.
   */
  mapped: (folder: string) => boolean | null;
  /** Whether a book is being packed right now, wherever it is. */
  freezing: boolean;
  /**
   * A drop on a row: its files, each with its path relative to that row's
   * folder, and what the desktop app did not take of it.
   */
  onAdd: (folder: string, drop: Drop) => void;
  /** A drop was taken and its files are being read out of it, as the list's is. */
  onCollecting: () => void;
  /**
   * The drop came to no files: the walk could not be finished, or the desktop
   * app took nothing of it ([`NothingTaken`](./drop)), and this is why.
   */
  onUnreadable: (cause: unknown) => void;
  /** A drop onto a folder no mapping of this device reaches. */
  onUnmapped: (folder: string) => void;
}

/**
 * "New folder…" for the row of `folder`, where a mapping of this device is
 * known to reach it — and nothing where it is not, or not known yet: a path
 * prepared there would be one no drop could land under.
 */
function newFolderOf(
  folder: string,
  drops: TreeDrops,
  onNewFolder: (parent: string) => void,
): MenuItem[] {
  return drops.mapped(folder) === true
    ? [{ label: NEW_FOLDER, onPress: () => onNewFolder(folder) }]
    : [];
}

function Branch({
  node,
  depth,
  current,
  expanded,
  stranded,
  awaiting,
  onToggle,
  onOpen,
  onDelete,
  onNewFolder,
  drops,
}: {
  node: FolderNode;
  depth: number;
  current: string;
  expanded: ReadonlySet<string>;
  stranded: ReadonlySet<string>;
  awaiting: ReadonlySet<string>;
  onToggle: (path: string) => void;
  onOpen: (folder: string) => void;
  onDelete: (target: DeleteTarget) => void;
  onNewFolder: (parent: string) => void;
  drops: TreeDrops;
}) {
  const open = expanded.has(node.path);
  const book = node.children.length === 0;
  const waiting = stranded.has(node.path);
  const unheld = awaiting.has(node.path);
  return (
    <>
      <Row
        label={node.name}
        path={node.path}
        depth={depth}
        selected={current === node.path}
        waiting={waiting}
        unheld={unheld}
        onOpen={() => onOpen(node.path)}
        onToggle={book ? undefined : () => onToggle(node.path)}
        opened={open}
        menu={[
          ...newFolderOf(node.path, drops, onNewFolder),
          ...(book && !waiting && !unheld
            ? [{ label: DELETE, onPress: () => onDelete({ folder: node.path, paths: [] }) }]
            : []),
        ]}
        drops={drops}
      />
      {open &&
        node.children.map((child) => (
          <Branch
            key={child.path}
            node={child}
            depth={depth + 1}
            current={current}
            expanded={expanded}
            stranded={stranded}
            awaiting={awaiting}
            onToggle={onToggle}
            onOpen={onOpen}
            onDelete={onDelete}
            onNewFolder={onNewFolder}
            drops={drops}
          />
        ))}
    </>
  );
}

function Row({
  label,
  path,
  depth,
  selected,
  waiting,
  unheld,
  onOpen,
  onToggle,
  opened,
  menu = [],
  drops,
}: {
  label: string;
  /** The folder this row stands for; the empty string is the Library root. */
  path: string;
  depth: number;
  selected: boolean;
  /** Whether this folder is a stranded one, which the Library does not hold. */
  waiting?: boolean;
  /** Whether this folder is one the Library does not hold yet, to drop into. */
  unheld?: boolean;
  onOpen: () => void;
  /** Opens and closes the branch, on a folder that has one. */
  onToggle?: () => void;
  /** Whether the branch this marker turns is open, where it turns one. */
  opened?: boolean;
  /** What the row's "⋮" offers, and nothing drawn where it offers nothing. */
  menu?: readonly MenuItem[];
  drops: TreeDrops;
}) {
  // The current folder is brought into view, as the file list does for its
  // marked row: a folder restored from a URL several components deep has every
  // branch above it opened, and the row that says where the screen is would
  // otherwise sit below the fold with nothing to say so.
  const here = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (selected) {
      here.current?.scrollIntoView({ block: 'nearest' });
    }
  }, [selected]);

  // The drop this row takes, answered the way the file list answers one: an
  // outline in the colour of what letting go comes to and the line saying so,
  // read off the same fact the drop is turned away by. Unknown is taken, since
  // the drop asks the folder's own listing before anything is sent.
  const drag = useDragOver();
  const mapped = drag.dragged ? drops.mapped(path) : null;
  const outcome = dropOutcome({ mapped: mapped !== false, freezing: drops.freezing });
  const drop = (event: DragEvent) => {
    event.preventDefault();
    drag.ended();
    if (drops.mapped(path) === false) {
      // Said and not merely not done, as the list says it.
      drops.onUnmapped(path);
      return;
    }
    // The walk is asynchronous and the event is not, as on the list.
    drops.onCollecting();
    void droppedFiles(event.dataTransfer).then(
      (drop) => drops.onAdd(path, drop),
      drops.onUnreadable,
    );
  };

  return (
    <div
      onDragEnter={drag.onDragEnter}
      onDragOver={drag.onDragOver}
      onDragLeave={drag.onDragLeave}
      onDrop={drop}
      style={{ outline: dropOutline(drag.dragged, outcome), outlineOffset: -2 }}
    >
      <div ref={here} style={{ display: 'flex', alignItems: 'center' }}>
        {/* The marker opens and closes the branch and the name chooses the
            folder, so neither target has to mean both. A folder with nothing
            under it has no branch and no marker, and keeps the marker's room, so
            that every name at one depth starts at one place. */}
        {onToggle === undefined ? (
          <span aria-hidden style={{ ...MARKER, cursor: undefined, marginLeft: indent(depth) }} />
        ) : (
          <button
            onClick={onToggle}
            // What it will do when it is pressed, which is the opposite of what
            // it last did: a marker on an open branch closes it, and reading out
            // "expand" over it would name the wrong half of the toggle.
            aria-label={`${opened === true ? 'collapse' : 'expand'} ${label}`}
            style={{ ...MARKER, marginLeft: indent(depth) }}
          >
            {opened === true ? '▾' : '▸'}
          </button>
        )}
        <button
          className="tree-name"
          onClick={onOpen}
          // The name and nothing else, stranded or not: a row's `title` is what
          // it is called, and it is how a name too long for the column is still
          // readable. What "not in the Library yet" looks like is the colour.
          title={label}
          style={{
            flex: 1,
            minWidth: 0,
            textAlign: 'left',
            border: 'none',
            background: selected ? COLOR.selected : 'none',
            // Dimmed while the Library does not hold it: a stranded folder's
            // pages are on the disk and out of the Library, and nothing else
            // on its row would say so; a folder not held yet has its mark too.
            color: waiting === true || unheld === true ? COLOR.added : COLOR.text,
            font: 'inherit',
            padding: '3px 8px 3px 2px',
            borderRadius: 3,
            cursor: 'pointer',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {label}
        </button>
        {unheld === true && (
          <span title={AWAITING} style={MARK}>
            not in Library
          </span>
        )}
        <RowMenu name={label} items={menu} />
      </div>
      {/* Under the row rather than over the rows below it, and inside the
          outline, so that the pointer still over the line is still over the
          folder it is about. Every folder the tree draws is one there is
          something at, so the refusal's reason is the mapping's. */}
      {drag.dragged && drag.carriesFiles && <DropBanner outcome={outcome} held />}
    </div>
  );
}

/** The mark after the name of a folder the Library does not hold yet; the sentence is its title. */
const MARK = {
  flex: '0 0 auto',
  marginRight: 4,
  color: COLOR.added,
  fontSize: 11,
  whiteSpace: 'nowrap',
} as const;

/** How far in a row at `depth` starts. */
function indent(depth: number): number {
  return 6 + depth * 12;
}

/** The marker that opens and closes a branch, and the room it keeps on a row without one. */
const MARKER = {
  flex: '0 0 auto',
  width: 18,
  border: 'none',
  background: 'none',
  color: COLOR.dim,
  cursor: 'pointer',
  font: 'inherit',
  padding: 0,
} as const;
