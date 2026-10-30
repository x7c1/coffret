import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react';

import type { Described } from './details';
import { BUTTON, Dialog, LINE, PRIMARY } from './Dialog';
import { moveTargets, openedFor, type TargetRow } from './moving';
import { FOLDER_ICON_PATH } from './rowIcon';
import { COLOR } from './theme';

/**
 * The question "Move to…" asks first: where to. A tree of the Library's
 * folders alone, with a `Library` row at the top so that the root can be
 * chosen (see [`moveTargets`](./moving)), opened down to where the rows stand
 * now. A folder the move would do nothing in — the one they stand in, one of
 * them, anything under one — is drawn faint and cannot be chosen.
 *
 * Choosing a folder, and then Move (or a double-click on it), goes on to the
 * confirmation a drag's release opens, built from the server's preview.
 * Escape, a click outside it and Cancel close it having moved nothing.
 *
 * The tree is one stop of Tab: the row the keys last stood on, else the
 * chosen folder's, else the `Library` row. From there the keys are the Library tree's (see
 * [`arrowed`](./tree)): `↑`/`↓` go to the row above or below, `→` opens a
 * closed folder or goes to the first folder in an open one, `←` closes an open
 * folder or goes to the one it stands in; Enter or Space chooses the row, as a
 * click does.
 */
export function MoveToDialog({
  title,
  folders,
  moved,
  onChoose,
  onClose,
}: {
  /** `Move “Vol. 03” to…`, or `Move 3 items to…`. */
  title: string;
  /** Every folder to offer — the Library's and the ones prepared on this page — or `null` while they are not known. */
  folders: readonly string[] | null;
  /** The rows being moved. */
  moved: readonly Described[];
  /** The folder chosen; the empty string is the Library root. */
  onChoose: (landing: string) => void;
  onClose: () => void;
}) {
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => openedFor(moved));
  const [chosen, setChosen] = useState<string | null>(null);
  // The row the keys stand on, once a key or a press has put them on one.
  const [focus, setFocus] = useState<string | null>(null);
  const rows = useMemo(
    () => (folders === null ? [] : moveTargets(folders, expanded, moved)),
    [folders, expanded, moved],
  );
  const shown = (path: string | null): path is string => path !== null && rows.some((row) => row.path === path);
  // A row under a folder closed since is no longer drawn, and gives way.
  const current = shown(focus) ? focus : shown(chosen) ? chosen : '';
  const drawn = useRef(new Map<string, HTMLDivElement>());
  // Whether a key moved `current`, so that the browser's focus follows it; a
  // press moves it itself, and opening the dialog must leave it where it is.
  const byKey = useRef(false);
  useEffect(() => {
    if (byKey.current) {
      byKey.current = false;
      drawn.current.get(current)?.focus();
    }
  }, [current]);
  const toggle = (path: string) =>
    setExpanded((held) => {
      const next = new Set(held);
      if (!next.delete(path)) {
        next.add(path);
      }
      return next;
    });

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = keyed(rows, current, event.key);
    if (step === null) {
      return;
    }
    event.preventDefault();
    if ('choose' in step) {
      setChosen(step.choose);
    } else if ('toggle' in step) {
      toggle(step.toggle);
    } else {
      byKey.current = true;
      setFocus(step.focus);
    }
  };

  return (
    <Dialog
      title={title}
      onCancel={onClose}
      fill
      actions={
        <>
          <button type="button" onClick={onClose} style={BUTTON}>
            Cancel
          </button>
          <button
            type="button"
            disabled={chosen === null}
            onClick={() => chosen !== null && onChoose(chosen)}
            style={chosen === null ? { ...PRIMARY, opacity: 0.5, cursor: 'default' } : PRIMARY}
          >
            Move
          </button>
        </>
      }
    >
      {folders === null ? (
        <p style={{ ...LINE, color: COLOR.dim }}>loading the folders…</p>
      ) : (
        <div role="tree" aria-label="folders" onKeyDown={onKeyDown} style={TREE}>
          {rows.map((row) => {
            const picked = row.path === chosen;
            return (
              <div
                key={row.path === '' ? '/' : row.path}
                ref={(element) => {
                  if (element === null) {
                    drawn.current.delete(row.path);
                  } else {
                    drawn.current.set(row.path, element);
                  }
                }}
                role="treeitem"
                tabIndex={row.path === current ? 0 : -1}
                aria-selected={picked}
                aria-disabled={!row.takes}
                aria-expanded={row.expandable ? row.expanded : undefined}
                title={row.path === '' ? row.name : row.path}
                onClick={() => {
                  setFocus(row.path);
                  if (row.takes) {
                    setChosen(row.path);
                  }
                }}
                onDoubleClick={() => row.takes && onChoose(row.path)}
                style={{
                  ...ROW,
                  paddingLeft: 6 + row.depth * 14,
                  background: picked ? COLOR.accent : 'transparent',
                  color: picked ? COLOR.onAccent : row.takes ? COLOR.text : COLOR.faint,
                  cursor: row.takes ? 'pointer' : 'default',
                }}
              >
                {row.expandable ? (
                  <button
                    type="button"
                    tabIndex={-1}
                    aria-label={`${row.expanded ? 'collapse' : 'expand'} ${row.name}`}
                    onClick={(event) => {
                      event.stopPropagation();
                      toggle(row.path);
                    }}
                    onDoubleClick={(event) => event.stopPropagation()}
                    style={{ ...DISCLOSURE, color: picked ? COLOR.onAccent : COLOR.dim }}
                  >
                    <svg aria-hidden width={12} height={12} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2.5} strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block', transform: row.expanded ? 'rotate(90deg)' : undefined }}>
                      <path d="m9 6 6 6-6 6" />
                    </svg>
                  </button>
                ) : (
                  <span aria-hidden style={{ width: DISCLOSURE.width, flex: '0 0 auto' }} />
                )}
                <svg
                  aria-hidden
                  width={14}
                  height={14}
                  viewBox="0 0 24 24"
                  fill={picked ? 'none' : COLOR.folderFill}
                  stroke={picked ? COLOR.onAccent : COLOR.folderStroke}
                  strokeWidth={2}
                  strokeLinejoin="round"
                  style={{ flex: '0 0 auto', marginRight: 6 }}
                >
                  <path d={FOLDER_ICON_PATH} />
                </svg>
                <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{row.name}</span>
              </div>
            );
          })}
        </div>
      )}
    </Dialog>
  );
}

/** What a key pressed on the tree does, standing on `current`: `null` where it does nothing. */
function keyed(
  rows: readonly TargetRow[],
  current: string,
  key: string,
): { focus: string } | { toggle: string } | { choose: string } | null {
  const at = rows.findIndex((row) => row.path === current);
  if (at === -1) {
    return null;
  }
  const row = rows[at];
  switch (key) {
    case 'Enter':
    case ' ':
      return row.takes ? { choose: row.path } : null;
    case 'ArrowUp':
    case 'ArrowDown': {
      const next = rows[at + (key === 'ArrowDown' ? 1 : -1)];
      return next === undefined ? null : { focus: next.path };
    }
    case 'ArrowRight': {
      if (row.expandable && !row.expanded) {
        return { toggle: row.path };
      }
      const first = rows[at + 1];
      return row.expanded && first !== undefined && first.depth === row.depth + 1 ? { focus: first.path } : null;
    }
    case 'ArrowLeft': {
      if (row.expandable && row.expanded) {
        return { toggle: row.path };
      }
      if (row.path === '') {
        return null;
      }
      const slash = row.path.lastIndexOf('/');
      return { focus: slash === -1 ? '' : row.path.slice(0, slash) };
    }
    default:
      return null;
  }
}

const TREE: CSSProperties = {
  flex: '1 1 auto',
  minHeight: 0,
  overflow: 'auto',
  border: `1px solid ${COLOR.border}`,
  borderRadius: 8,
  background: COLOR.ground,
  padding: 4,
  userSelect: 'none',
};

const ROW: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  minHeight: 28,
  borderRadius: 6,
};

const DISCLOSURE: CSSProperties = {
  flex: '0 0 auto',
  width: 18,
  height: 18,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  border: 'none',
  background: 'none',
  padding: 0,
  cursor: 'pointer',
};
