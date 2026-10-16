import { useEffect, useRef, useState } from 'react';

import type { DeleteTarget } from '@coffret/api';

import { DELETE } from './deleteEntries';
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
 * And one control: a folder made here, under the one that is open. It makes a
 * place and nothing more — how what is dropped into it is added follows the
 * drop, as it does anywhere — and it is the only thing on this screen that adds
 * a place rather than showing one. The place is this screen's until the first
 * Entry under it commits, which is what makes it a folder the server names for
 * itself; a pending one is drawn dimmed, so that "not in the Library yet" is
 * something a person can see rather than something they have to remember.
 *
 * A book — a folder with no folders under it, which is where pages are — can
 * be deleted from its row's "⋮", as it can from its row in the file list: the
 * same target handed over, so the same question asked. Not a folder made here
 * and not in the Library yet, which the Library has nothing of to take out.
 */
export function FolderTree({
  folders,
  pending,
  current,
  onOpen,
  onNewFolder,
  onDelete,
}: {
  folders: readonly string[];
  /** The folders made here that the Library does not hold yet. */
  pending: readonly string[];
  current: string;
  onOpen: (folder: string) => void;
  /** Make a folder under the one that is open. */
  onNewFolder: () => void;
  /** "Delete…" was chosen from a book's menu: the same as the file list's folder menu. */
  onDelete: (target: DeleteTarget) => void;
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
  const waiting = new Set(pending);
  return (
    <nav aria-label="folders" style={{ padding: '8px 0' }}>
      {/* No mark: the root is the place the tree hangs from, and what it is
          called is in its name. */}
      <Row
        label="Library"
        depth={0}
        selected={current === ''}
        onOpen={() => onOpen('')}
      />
      {nest(folders).map((node) => (
        <Branch
          key={node.path}
          node={node}
          depth={1}
          current={current}
          expanded={expanded}
          pending={waiting}
          onToggle={toggle}
          onOpen={onOpen}
          onDelete={onDelete}
        />
      ))}
      {/* Under the tree rather than beside the folder it acts on. What it makes
          is a folder inside whichever one is open, which the label says in
          words: a plus beside every row would be five hundred controls for one
          gesture nobody makes twice in a row. */}
      <button
        onClick={onNewFolder}
        style={{
          display: 'block',
          margin: '10px 6px 0',
          border: `1px dashed ${COLOR.border}`,
          background: 'none',
          color: COLOR.dim,
          font: 'inherit',
          padding: '3px 8px',
          borderRadius: 3,
          cursor: 'pointer',
        }}
      >
        + new folder in {current === '' ? 'the Library' : current.split('/').at(-1)}
      </button>
    </nav>
  );
}

function Branch({
  node,
  depth,
  current,
  expanded,
  pending,
  onToggle,
  onOpen,
  onDelete,
}: {
  node: FolderNode;
  depth: number;
  current: string;
  expanded: ReadonlySet<string>;
  pending: ReadonlySet<string>;
  onToggle: (path: string) => void;
  onOpen: (folder: string) => void;
  onDelete: (target: DeleteTarget) => void;
}) {
  const open = expanded.has(node.path);
  const book = node.children.length === 0;
  const waiting = pending.has(node.path);
  return (
    <>
      <Row
        label={node.name}
        depth={depth}
        selected={current === node.path}
        waiting={waiting}
        onOpen={() => onOpen(node.path)}
        onToggle={book ? undefined : () => onToggle(node.path)}
        opened={open}
        menu={
          book && !waiting
            ? [{ label: DELETE, onPress: () => onDelete({ folder: node.path, paths: [] }) }]
            : []
        }
      />
      {open &&
        node.children.map((child) => (
          <Branch
            key={child.path}
            node={child}
            depth={depth + 1}
            current={current}
            expanded={expanded}
            pending={pending}
            onToggle={onToggle}
            onOpen={onOpen}
            onDelete={onDelete}
          />
        ))}
    </>
  );
}

function Row({
  label,
  depth,
  selected,
  waiting,
  onOpen,
  onToggle,
  opened,
  menu = [],
}: {
  label: string;
  depth: number;
  selected: boolean;
  /** Whether this folder is one made here that the Library does not hold. */
  waiting?: boolean;
  onOpen: () => void;
  /** Opens and closes the branch, on a folder that has one. */
  onToggle?: () => void;
  /** Whether the branch this marker turns is open, where it turns one. */
  opened?: boolean;
  /** What the row's "⋮" offers, and nothing drawn where it offers nothing. */
  menu?: readonly MenuItem[];
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

  return (
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
        // The name and nothing else, pending or not: a row's `title` is what it
        // is called, and it is how a name too long for the column is still
        // readable. What "not in the Library yet" looks like is the colour.
        title={label}
        style={{
          flex: 1,
          minWidth: 0,
          textAlign: 'left',
          border: 'none',
          background: selected ? COLOR.selected : 'none',
          // Dimmed while the Library does not hold it: the folder is on this
          // screen and nowhere else, and nothing else on the row would say so.
          color: waiting === true ? COLOR.added : COLOR.text,
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
      <RowMenu name={label} items={menu} />
    </div>
  );
}

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
