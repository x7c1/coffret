import { useRef, useState, type CSSProperties } from 'react';

import type { Library } from '@coffret/api';

import type { ActionItem } from './actions';
import { ActionsMenu, type MenuAt } from './ActionsMenu';
import { landingOf, placeOf } from './destination';
import { pickedFiles, type Drop } from './drop';
import { locationOf } from './location';
import { COLOR } from './theme';
import type { Asked } from './useAsked';

/**
 * The one row over the tree and the details pane: which Library is open on
 * the left, where the focus stands in the middle, and **+** (Add files) on the
 * right.
 *
 * - **The Library.** Its name, drawn like a button with no icon in front of it
 *   — a padlock there would read as "locked", which an open Library is not.
 *   It is the place switching Libraries will live; while a server serves one
 *   Library there is nothing to switch to, so it opens nothing and draws no
 *   arrow promising that it would. A request for the Library that was refused
 *   is said here in its place, coloured like every other refusal, since it is
 *   why the screen below is empty.
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
 *   is sent, as it does a drop.
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
   * The menu was let go of by `Escape`: the keys go back to the tree, whose
   * focused row the files would have landed by.
   */
  onKeysBack: () => void;
}) {
  const location = locationOf(focus);
  const addInto = landingOf(focus);
  const filesPicker = useRef<HTMLInputElement>(null);
  const folderPicker = useRef<HTMLInputElement | null>(null);
  const [adding, setAdding] = useState<{ at: MenuAt; into: string } | null>(null);
  // The folder the picker chosen was opened for, kept across the wait for the
  // operating system's answer: the focus may move meanwhile.
  const picking = useRef('');
  const openPicker = (item: ActionItem) => {
    const into = adding?.into ?? addInto;
    setAdding(null);
    picking.current = into;
    (item.action.kind === 'addFolder' ? folderPicker : filesPicker).current?.click();
  };
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
        <span
          data-testid="library"
          // The whole of it, for a name cut short.
          title={named(library)}
          style={library.status === 'failed' ? { ...LIBRARY, color: COLOR.refused } : LIBRARY}
        >
          {named(library)}
        </span>
        <span data-testid="location" title={location} style={LOCATION}>
          {location}
        </span>
      </div>
      <button
        type="button"
        aria-label="Add files"
        title={`Add files to ${placeOf(addInto)}`}
        aria-haspopup="menu"
        aria-expanded={adding !== null}
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
        style={ADD_BUTTON}
      >
        +
      </button>
      {adding !== null && (
        <ActionsMenu
          label={`add files to ${placeOf(adding.into)}`}
          at={adding.at}
          items={ADD_ITEMS}
          onChoose={openPicker}
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

/** What the **+** offers: the operating system's picker for files, or for one folder. */
const ADD_ITEMS: readonly ActionItem[] = [
  { label: 'Files…', action: { kind: 'addFiles' }, destructive: false, separated: false },
  { label: 'A folder…', action: { kind: 'addFolder' }, destructive: false, separated: false },
];

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
 * The Library and the location on the left, wrapping; the **+** beside them on
 * the right, never wrapped onto a line of its own.
 */
const HEADER: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: 12,
  padding: '6px 12px',
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

/** Drawn like a button, since switching Libraries is to live here. */
const LIBRARY: CSSProperties = {
  flex: '0 1 auto',
  minWidth: 0,
  maxWidth: '100%',
  padding: '2px 10px',
  border: `1px solid ${COLOR.border}`,
  borderRadius: 4,
  color: COLOR.text,
  fontSize: 13,
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

/** The **+**, square and a little larger than the words beside it. */
const ADD_BUTTON: CSSProperties = {
  flex: '0 0 auto',
  width: 26,
  height: 24,
  border: `1px solid ${COLOR.border}`,
  background: 'transparent',
  color: COLOR.text,
  font: 'inherit',
  fontSize: 16,
  lineHeight: 1,
  padding: 0,
  borderRadius: 4,
  cursor: 'pointer',
};
