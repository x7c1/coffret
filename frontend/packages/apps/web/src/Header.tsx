import { useEffect, useRef, useState, type CSSProperties, type DragEvent } from 'react';

import type { Library } from '@coffret/api';

import type { Action, ActionItem } from './actions';
import { ActionsMenu, type MenuAt } from './ActionsMenu';
import { AppearanceSwitch } from './AppearanceSwitch';
import { dropGround, dropOutline } from './Banner';
import type { Described } from './details';
import { landingOf, placeOf } from './destination';
import { pickedFiles, type Drop } from './drop';
import { locationOf } from './location';
import { moveLine, movesInto } from './moving';
import { NEW_FOLDER } from './prospective';
import { COLOR } from './theme';
import type { Asked } from './useAsked';

/**
 * The one row over the tree and the details pane: which Library is open on
 * the left, where the focus stands in the middle, and the appearance switch
 * and **+** (Add files) on the right.
 *
 * - **The Library.** Its name, drawn like a button with no icon in front of it
 *   — a padlock there would read as "locked", which an open Library is not.
 *   It is the place switching Libraries will live; while a server serves one
 *   Library there is nothing to switch to, so it opens nothing and draws no
 *   arrow promising that it would. A request for the Library that was refused
 *   is said here in its place, coloured like every other refusal, since it is
 *   why the screen below is empty. It is also where rows of the tree are
 *   dragged to move them to the Library root, which has no row of its own:
 *   while they are over it, it is outlined as a folder row is and says
 *   `Move to Library` — unless they stand at the root already, where nothing
 *   happens.
 * - **The location.** The focused row's containing folders and its own name
 *   (see [`locationOf`](./location)), following the focus as the details pane
 *   does, and cut short with an ellipsis where the row is too narrow for it —
 *   the whole of it in its tooltip.
 * - **Add files.** A small menu of the operating system's two pickers, since
 *   no browser's picker takes files and a folder at once, which opens over the
 *   focused folder — or the folder of the focused file (see
 *   [`landingOf`](./destination)) — read as the menu opens and kept for the
 *   answer. What is picked is handed to the screen, which asks for a folder on
 *   this device where none is chosen yet and confirms the add before anything
 *   is sent, as it does a drop. Below them, "New folder…" in that same folder,
 *   where a mapping reaches it: making a place for files is part of adding
 *   them, and it is how a folder is made at the Library root, which has no row
 *   and so no row menu.
 *
 * - **Appearance.** Beside the **+**, the switch between the sets of colours
 *   the explorer is drawn in — the system's, light or dark (see
 *   [`AppearanceSwitch`](./AppearanceSwitch)). Here rather than in a menu bar,
 *   which the desktop app hides, or on a settings page, which there is not.
 *
 * While the Library is locked the **+** is drawn disabled, since nothing can be
 * added to a locked Library, and a name the lock kept the page from reading is
 * left out rather than said as a refusal: the main area says the lock.
 *
 * One row on a wide window; on a narrow one it wraps rather than scrolling
 * sideways, the location taking a line of its own before anything is cut off
 * the Library's name or the **+**.
 */
export function Header({
  library,
  focus,
  onAdd,
  onCollecting,
  onUnreadable,
  onKeysBack,
  makesFolderIn,
  onNewFolder,
  moving = null,
  onMoveToRoot = () => {},
  locked = false,
}: {
  /** The open Library, as the server last answered for it. */
  library: Asked<Library>;
  /** The key of the focused row of the tree, or `null` where nothing is focused. */
  focus: string | null;
  /** Files picked with **+**, for `folder`: each with its path relative to it. */
  onAdd: (folder: string, drop: Drop) => void;
  /** Files were picked and are being read, which can take seconds. */
  onCollecting: () => void;
  /** They came to no files: the walk could not be finished, or the desktop app took nothing of them. */
  onUnreadable: (cause: unknown) => void;
  /**
   * A menu of the header's was let go of by `Escape`, or the appearance
   * switch's by a choice: the keys go back to the tree — whose focused row, for
   * the **+**'s, the files would have landed by.
   */
  onKeysBack: () => void;
  /** Whether "New folder…" can be offered in `folder`: a mapping reaches it. */
  makesFolderIn: (folder: string) => boolean;
  /** "New folder…" was chosen for `parent`. */
  onNewFolder: (parent: string) => void;
  /** The rows of the tree being dragged, while a drag of them is in the air. */
  moving?: readonly Described[] | null;
  /** They were let go of over the Library's name: moved to the root, once confirmed. */
  onMoveToRoot?: () => void;
  /** Whether the Library is locked, which leaves nothing to add to. */
  locked?: boolean;
}) {
  const location = locationOf(focus);
  const addInto = landingOf(focus);
  const filesPicker = useRef<HTMLInputElement>(null);
  const folderPicker = useRef<HTMLInputElement | null>(null);
  const [adding, setAdding] = useState<{ at: MenuAt; into: string } | null>(null);
  // The folder the picker chosen was opened for, kept across the wait for the
  // operating system's answer: the focus may move meanwhile.
  const picking = useRef('');
  const choose = (action: Action) => {
    const into = adding?.into ?? addInto;
    setAdding(null);
    if (action.kind === 'newFolder') {
      onNewFolder(into);
      return;
    }
    picking.current = into;
    (action.kind === 'addFolder' ? folderPicker : filesPicker).current?.click();
  };
  // A drag of rows over the Library's name, counted as the tree counts one
  // (see [`useDragOver`](./dragOver)), and let go of once the drag is over.
  const [movingOver, setMovingOver] = useState(0);
  const takesMove = moving !== null && movesInto('', moving);
  useEffect(() => {
    if (moving === null) {
      setMovingOver(0);
    }
  }, [moving]);
  const overLibrary = takesMove && movingOver > 0;
  const moveHandlers = takesMove
    ? {
        onDragEnter: (event: DragEvent) => {
          event.preventDefault();
          setMovingOver((crossed) => crossed + 1);
        },
        onDragOver: (event: DragEvent) => {
          event.preventDefault();
          event.dataTransfer.dropEffect = 'move';
        },
        onDragLeave: () => setMovingOver((crossed) => Math.max(0, crossed - 1)),
        onDrop: (event: DragEvent) => {
          event.preventDefault();
          setMovingOver(0);
          onMoveToRoot();
        },
      }
    : {};
  const pickedFrom = (input: HTMLInputElement) => {
    const files = Array.from(input.files ?? []);
    // Emptied, so that the same files picked again are a change the input
    // reports rather than one it swallows.
    input.value = '';
    if (files.length === 0) {
      return;
    }
    const into = picking.current;
    onCollecting();
    void pickedFiles(files).then((drop) => onAdd(into, drop), onUnreadable);
  };

  return (
    <header style={HEADER}>
      <div style={WHERE}>
        {!(locked && library.status !== 'ready') && (
          <span
            data-testid="library"
            // The whole of it, for a name cut short.
            title={named(library)}
            {...moveHandlers}
            style={{
              ...LIBRARY,
              ...(library.status === 'failed' ? { color: COLOR.refused } : {}),
              ...(overLibrary
                ? { outline: dropOutline(true, 'taken'), outlineOffset: -2, background: dropGround('taken') }
                : {}),
            }}
          >
            {overLibrary ? moveLine('') : named(library)}
          </span>
        )}
        <span data-testid="location" title={location} style={LOCATION}>
          {location}
        </span>
      </div>
      <AppearanceSwitch onKeysBack={onKeysBack} />
      <button
        type="button"
        aria-label="Add files"
        title={`Add to ${placeOf(addInto)}`}
        aria-haspopup="menu"
        aria-expanded={adding !== null}
        disabled={locked}
        // The menu closes on a press outside it, which this is; the click
        // below opens it, or closes it where it is open.
        onMouseDown={(event) => event.stopPropagation()}
        onClick={(event) => {
          if (adding !== null) {
            setAdding(null);
            return;
          }
          const box = event.currentTarget.getBoundingClientRect();
          // Hung from its right edge: the button is at the window's right end,
          // and a menu hung from its left would run past it.
          setAdding({
            at: { x: box.right, y: box.bottom, side: 'end', byKeys: event.detail === 0 },
            into: addInto,
          });
        }}
        style={locked ? { ...ADD_BUTTON, ...ADD_DISABLED } : ADD_BUTTON}
      >
        <svg
          aria-hidden
          width={18}
          height={18}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth={2}
          strokeLinecap="round"
          style={{ display: 'block' }}
        >
          <path d="M12 5v14M5 12h14" />
        </svg>
      </button>
      {adding !== null && !locked && (
        <ActionsMenu
          title={`Add to ${placeOf(adding.into)}`}
          label={`add to ${placeOf(adding.into)}`}
          at={adding.at}
          items={addItems(adding.into, makesFolderIn(adding.into))}
          onChoose={choose}
          onClose={(how) => {
            setAdding(null);
            if (how === 'escape') {
              onKeysBack();
            }
          }}
        />
      )}
      <input
        ref={filesPicker}
        type="file"
        multiple
        hidden
        data-testid="pick-files"
        onChange={(event) => pickedFrom(event.currentTarget)}
      />
      <input
        ref={(input) => {
          folderPicker.current = input;
          // Not in React's own attributes: what makes the picker take a
          // folder whole, in every browser the explorer runs in.
          input?.setAttribute('webkitdirectory', '');
        }}
        type="file"
        hidden
        data-testid="pick-folder"
        onChange={(event) => pickedFrom(event.currentTarget)}
      />
    </header>
  );
}

/**
 * What the **+** offers for `into`: the operating system's picker for files, or
 * for one folder, and "New folder…" there where a mapping reaches it.
 */
export function addItems(into: string, makesFolder: boolean): ActionItem[] {
  const items: ActionItem[] = [
    { label: 'Files…', action: { kind: 'addFiles' }, destructive: false, separated: false },
    { label: 'A folder…', action: { kind: 'addFolder' }, destructive: false, separated: false },
  ];
  if (makesFolder) {
    items.push({ label: NEW_FOLDER, action: { kind: 'newFolder', folder: into }, destructive: false, separated: true });
  }
  return items;
}

/**
 * What the header calls the Library: its name alone. Which Storage it is kept
 * on is the server's word for a provider, and not something a reader chose or
 * needs to tell one Library from another by. A Library nothing has named yet
 * is said to have no name, and its ID is given as its ID rather than where the
 * name goes.
 */
function named(library: Asked<Library>): string {
  switch (library.status) {
    case 'loading':
      return 'opening the Library…';
    case 'ready':
      return library.value.name ?? `a Library with no name yet (ID ${library.value.library_id})`;
    case 'failed':
      return library.message;
  }
}

/**
 * The Library and the location on the left, wrapping; the appearance switch
 * and the **+** beside them on the right, never wrapped onto a line of its own.
 */
const HEADER: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: 12,
  padding: '10px 16px',
  background: COLOR.panel,
  borderBottom: `1px solid ${COLOR.border}`,
  minWidth: 0,
};

/**
 * The Library's name and the location, the location wrapping under the name
 * where the two do not fit on one line.
 */
const WHERE: CSSProperties = {
  flex: '1 1 auto',
  minWidth: 0,
  display: 'flex',
  flexWrap: 'wrap',
  alignItems: 'center',
  gap: '6px 12px',
};

/**
 * Drawn like a button, since switching Libraries is to live here: a box 32px
 * high, the name on one line in the middle of it — by its line height rather
 * than a flex box, so that a name too long for the row is still cut short with
 * an ellipsis.
 */
const LIBRARY: CSSProperties = {
  flex: '0 1 auto',
  minWidth: 0,
  maxWidth: '100%',
  boxSizing: 'border-box',
  minHeight: 32,
  lineHeight: '30px',
  padding: '0 12px',
  border: `1px solid ${COLOR.border}`,
  borderRadius: 8,
  color: COLOR.text,
  fontSize: 14,
  fontWeight: 600,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
};

/**
 * Grows into the room between the two ends, and is the first thing to give
 * way: on a narrow window it wraps to a line of its own, and is cut short
 * there rather than pushing the row wider than the window.
 */
const LOCATION: CSSProperties = {
  flex: '1 1 12em',
  minWidth: 0,
  color: COLOR.dim,
  fontSize: 13,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
};

/** The **+**: a square as high as the Library's box, filled with the accent, a white plus in it. */
const ADD_BUTTON: CSSProperties = {
  flex: '0 0 auto',
  width: 32,
  height: 32,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  border: 'none',
  background: COLOR.accent,
  color: COLOR.onAccent,
  padding: 0,
  borderRadius: 8,
  cursor: 'pointer',
};

/** The **+** while the Library is locked: faint, on a ground of no colour, and not pressed. */
const ADD_DISABLED: CSSProperties = {
  background: COLOR.hover,
  color: COLOR.faint,
  cursor: 'default',
};
