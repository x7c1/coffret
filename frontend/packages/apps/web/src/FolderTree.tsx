import { useEffect, useState } from 'react';

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
 * And one control: a folder made here, under the one that is open. It is where
 * a book is brought in — the pages dropped into a folder made for them are
 * packed rather than synced — and it is the only thing on this screen that adds
 * a place rather than showing one. The place is this screen's until the first
 * Entry under it commits, which is what makes it a folder the server names for
 * itself; a pending one is drawn dimmed, so that "not in the Library yet" is
 * something a person can see rather than something they have to remember.
 */
export function FolderTree({
  folders,
  pending,
  current,
  onOpen,
  onNewFolder,
}: {
  folders: readonly string[];
  /** The folders made here that the Library does not hold yet. */
  pending: readonly string[];
  current: string;
  onOpen: (folder: string) => void;
  /** Make a folder under the one that is open. */
  onNewFolder: () => void;
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
      <Row
        label="Library"
        icon="◈"
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
}: {
  node: FolderNode;
  depth: number;
  current: string;
  expanded: ReadonlySet<string>;
  pending: ReadonlySet<string>;
  onToggle: (path: string) => void;
  onOpen: (folder: string) => void;
}) {
  const open = expanded.has(node.path);
  return (
    <>
      <Row
        label={node.name}
        icon={node.children.length === 0 ? '·' : open ? '▾' : '▸'}
        depth={depth}
        selected={current === node.path}
        waiting={pending.has(node.path)}
        onOpen={() => onOpen(node.path)}
        onToggleIcon={node.children.length === 0 ? undefined : () => onToggle(node.path)}
        opened={open}
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
          />
        ))}
    </>
  );
}

function Row({
  label,
  icon,
  depth,
  selected,
  waiting,
  onOpen,
  onToggleIcon,
  opened,
}: {
  label: string;
  icon: string;
  depth: number;
  selected: boolean;
  /** Whether this folder is one made here that the Library does not hold. */
  waiting?: boolean;
  onOpen: () => void;
  onToggleIcon?: () => void;
  /** Whether the branch this marker turns is open, where it turns one. */
  opened?: boolean;
}) {
  return (
    <div style={{ display: 'flex', alignItems: 'center' }}>
      {/* The marker opens and closes the branch and the name chooses the
          folder, so neither target has to mean both — except on a folder with
          nothing under it, where there is no branch and the marker may as well
          do what the row does. */}
      <button
        onClick={onToggleIcon ?? onOpen}
        // What it will do when it is pressed, which is the opposite of what it
        // last did: a marker on an open branch closes it, and reading out
        // "expand" over it would name the wrong half of the toggle.
        aria-label={
          onToggleIcon === undefined
            ? label
            : `${opened === true ? 'collapse' : 'expand'} ${label}`
        }
        style={{
          width: 18,
          marginLeft: 6 + depth * 12,
          border: 'none',
          background: 'none',
          color: COLOR.dim,
          cursor: 'pointer',
          font: 'inherit',
          padding: 0,
        }}
      >
        {icon}
      </button>
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
          color: waiting === true ? COLOR.uploading : COLOR.text,
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
    </div>
  );
}
