import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type DragEvent,
  type FocusEvent,
  type KeyboardEvent,
  type MouseEvent,
  type RefObject,
} from 'react';

import type { DeleteTarget, DisplacedFill, Fill, Freeze, ListedFile, Listing } from '@coffret/api';

import { actionsFor, type ActionItem } from './actions';
import { ActionsMenu, type MenuAt } from './ActionsMenu';
import { Banner, dropGround, DropPill, dropOutline } from './Banner';
import { canvasMeasure, columnWidth, type Measured } from './columns';
import { counted, type Described } from './details';
import { useDragOver } from './dragOver';
import { landingOf } from './destination';
import { droppedFiles, type Drop } from './drop';
import { dropOutcome, type DropOutcome } from './dropTarget';
import { freezingHere, isFreezing, rowFill, type RowFill } from './fill';
import { size, time } from './humanize';
import { CHOOSE_A_FOLDER, noFolderChosenLine } from './mapping';
import { parentOf, PROSPECTIVE } from './prospective';
import { TRY_AGAIN } from './Region';
import { ROWS_AHEAD, scrollAhead } from './scrollAhead';
import {
  clicked,
  clickOf,
  isSelected,
  NOTHING_SELECTED,
  rightClicked,
  selectedRows,
  stepped,
  type Click,
  type Picked,
  type Selection,
} from './selection';
import { rowIconOf, type RowIcon } from './rowIcon';
import { COLOR, FONT } from './theme';
import {
  arrowed,
  contextOf,
  folderKey,
  keyed,
  toggled,
  type Arrow,
  type FileRow,
  type FolderRow,
  type TreeRow,
  type TreeState,
} from './tree';
import { NOTHING_AT_THIS_PATH, type Tried } from './unmapped';

/**
 * The Library as one tree table, in the middle of the screen: folders and
 * files as rows of one table, indented by depth, like the list view of a
 * desktop file manager.
 *
 * Columns, in order: Name, Size, whether the row is on this device, and
 * Modified, a folder's Size and Modified being those of everything below it
 * (see `valuesOf`). A folder row opens and closes from the disclosure button at its
 * start; a file and an empty folder keep that room, so that every name at one
 * depth starts at one place. Every file is a row, each page of a scanned book
 * included. Each value column is as wide as the widest of its heading and the
 * values on show, measured from the rendered strings (see
 * [`columnWidth`](./columns)).
 *
 * The keys walk the rows the way a file manager's tree does (see
 * [`arrowed`](./tree)): `↑`/`↓` move the focus, `→` opens a folder and then
 * enters it, `←` closes one and then climbs to the folder above, and `Enter`
 * opens the focused row in the reader's sheet — for an image, or a folder
 * holding images — which then follows the focus (see
 * [`ReaderView`](./ReaderView)); on a folder it also opens the folder, so
 * `↓` goes on into its files with the sheet following. The list scrolls ahead
 * of the focus (see [`scrollAhead`](./scrollAhead)). A click focuses a row, a
 * double-click opens it as `Enter` does, and a press on the disclosure button
 * opens or closes the folder without moving the focus — except out of a folder
 * being closed over it. On a touch screen a tap opens, since a double tap is
 * not a gesture a touch screen reliably hands over. `Escape` while the sheet is
 * closed climbs to the folder the focused row stands in, and from a top-level
 * row lets go of the focus; so does a click on the empty space below the rows.
 * With nothing focused the header's **+** is about the Library root again.
 *
 * Rows are selected together with modifier keys, as in a desktop file manager
 * (see [`selection`](./selection)): Shift+click selects a range from the
 * anchor, ⌘/Ctrl+click adds or removes one row, Shift+`↑`/`↓` grows the
 * range, and a plain click or arrow drops it. The selected rows are tinted,
 * and the focused row keeps its strong highlight.
 *
 * What a row offers is in one actions menu (see [`actionsFor`](./actions)),
 * opened by a right-click on the row or by the "⋮" at its right end — shown
 * on the focused row and the row under the pointer only. A right-click inside
 * a selection of several acts on all of them; on any other row it first makes
 * that row the only focus.
 *
 * What the screen's own actions are about is the folder the focus is in (see
 * [`contextOf`](./tree)), and the block over the rows speaks for it: whether a
 * folder of this device is chosen for it, and whether it is being packed. A
 * folder's own actions are in its row's menu; the Library root has no row to
 * open a menu on, and its one, "New folder…", is in the header's **+** menu
 * ("Pack this folder…" is never the root's, see [`packOffered`](./packFolder)).
 *
 * Each row's state is the listing's answer, with what the server is doing
 * about it over the top: `present`, `remote` and `added` are the listing's to
 * say, while `fetching`, `failed` and `declined` are the fill's — work in
 * flight, which the listing has no word for. They are drawn in the narrow
 * column right after Size, so whether a thing is here and how much room it
 * takes are read together: nothing for a file the Library holds and this
 * device does not, and a laptop for one this device has — fetched here or
 * added from here, which is why it is not called downloaded. A folder carries
 * the laptop only where every file at every depth below it is here (the
 * server's answer, `present_throughout`): the promise that the whole book reads
 * offline.
 *
 * The tree is also where files are dropped to add them (the header's **+** is
 * the other way in, see [`Header`](./Header)): a drop on a folder row adds to
 * that folder, on a file row to the folder it stands in, and anywhere else on
 * the list to the folder the focus is in. While files are dragged, the row of the
 * folder they would land in is outlined and a pill at the bottom of the tree
 * says `Drop to add to <path>`. The files are handed to the screen, which
 * asks for a folder on this device where none is chosen yet and confirms
 * every add before anything is sent.
 *
 * The folders the Library does not hold yet (see [`prospective`](./prospective))
 * are rows too, marked as such, and so are the folders of books whose freeze
 * has not committed, dimmed. A prepared path's row offers "Forget this folder".
 */
export function TreeTable({
  rows,
  state,
  onChange,
  selection,
  onSelect,
  listingOf,
  context,
  prospectiveHere,
  prepared,
  fillOf,
  freeze,
  stranded,
  strandedKnown,
  onOpenFolder,
  onOpenFile,
  onUnsupported,
  onAdd,
  onCollecting,
  onUnreadable,
  onUnmapped,
  onMap,
  onPack,
  onDelete,
  onNewFolder,
  onForget,
  onAskAgain,
  measure,
  areaRef,
  escapeLetsGo = true,
}: {
  /** Every row on show, top to bottom (see [`treeRows`](./tree)). */
  rows: readonly TreeRow[];
  /** Which row has the focus, and which folders are open. */
  state: TreeState;
  /** The focus moved, or a folder opened or closed. */
  onChange: (next: TreeState) => void;
  /** The rows selected besides the focus (see [`selection`](./selection)). */
  selection: Selection;
  /** The selection changed: always beside an `onChange` for the focus it goes with. */
  onSelect: (next: Selection) => void;
  /** A folder's own listing where it is on hand, which says whether the reader can show it. */
  listingOf: (folder: string) => Listing | null;
  /**
   * The listing of the folder the focus is in, which the block over the rows
   * speaks for; `null` while it is not on hand.
   */
  context: Listing | null;
  /**
   * Whether that folder is one the Library does not hold yet: a path prepared
   * here, or a mapped top-level folder with nothing under it. Its listing says
   * the Library holds nothing at it, which is true and is not "nothing at this
   * path".
   */
  prospectiveHere: boolean;
  /** The paths prepared with "New folder…" on this page: the rows that offer "Forget this folder". */
  prepared: readonly string[];
  /** The fill the rows of one folder are to read, or `null` where none speaks for them. */
  fillOf: (folder: string) => Fill | DisplacedFill | null;
  /** What the server is packing, wherever it is packing it. */
  freeze: Freeze | null;
  /**
   * Whether the folder the focus is in is the folder of a book whose freeze has
   * not committed, which the Library does not hold yet (see
   * [`strandedFolders`](./stranded)).
   */
  stranded: boolean;
  /**
   * Whether the stranded folders are known yet: they are read off the Library's
   * folders, a request of its own, and until that lands a `stranded` of false
   * means "not known to be one" rather than "not one".
   */
  strandedKnown: boolean;
  /**
   * A folder row opened: the reader's sheet, on the folder's cover — where it
   * holds images, which only its listing can say.
   */
  onOpenFolder: (path: string) => void;
  /** A file row opened that the reader can show: the reader's sheet, on the file. */
  onOpenFile: (path: string) => void;
  /** A file row opened that the reader cannot show. */
  onUnsupported: (file: ListedFile) => void;
  /**
   * Files dropped, for `folder`: each with its path relative to it, and what
   * the desktop app did not take.
   */
  onAdd: (folder: string, drop: Drop) => void;
  /** Files were dropped and are being read, which can take seconds. */
  onCollecting: () => void;
  /** They came to no files: the walk could not be finished, or the desktop app took nothing of them. */
  onUnreadable: (cause: unknown) => void;
  /**
   * A gesture made in a folder with nowhere on this device to put its files:
   * a file opened there, or a drop on a path the Library holds nothing at.
   * `held` is whether this screen has a folder to talk about at all.
   */
  onUnmapped: (tried: Tried, held: boolean) => void;
  /**
   * The banner's button: choose a folder on this device for the top-level
   * folder named, or for the whole Library.
   */
  onMap: (prefix: string | null) => void;
  /** "Pack this folder…" from a folder row's actions menu. */
  onPack: (folder: string) => void;
  /** "Delete…" from the actions menu, for one row or the rows selected. */
  onDelete: (target: DeleteTarget) => void;
  /** "New folder…": prepare a path inside `parent`. */
  onNewFolder: (parent: string) => void;
  /** "Forget this folder" on a prepared path's row. */
  onForget: (path: string) => void;
  /**
   * "try again" on an open folder whose listing was refused: that folder's
   * listing asked for once more.
   */
  onAskAgain: (folder: string) => void;
  /** How wide a string renders; the page's own canvas where not given. */
  measure?: (text: string, measured: Measured) => number;
  /**
   * The tree's own element, the one holding the keyboard's focus, for a screen
   * that hands the focus back to it: the details pane's Open leaves it on the
   * tree, so that the arrows walk the rows the reader's sheet follows.
   */
  areaRef?: RefObject<HTMLDivElement>;
  /**
   * Whether `Escape` is the tree's to let go of the focus with: not while the
   * reader's sheet is open, whose `Escape` closes it.
   */
  escapeLetsGo?: boolean;
}) {
  const own = useRef<HTMLDivElement>(null);
  const scroller = areaRef ?? own;
  const block = useRef<HTMLDivElement>(null);
  const drawn = useRef(new Map<string, HTMLTableRowElement>());
  const contextPath = contextOf(state);
  const listing = context !== null && context.path === contextPath ? context : null;

  // The focus and the selection, which the pointer and the keys move together.
  const picked: Picked = { focus: state.focus, selection };
  const pick = (next: Picked) => {
    onChange({ ...state, focus: next.focus });
    onSelect(next.selection);
  };

  // The actions menu, while it is open: where, and for which rows.
  const [menu, setMenu] = useState<{ at: MenuAt; rows: Described[] } | null>(null);
  const closeMenu = useCallback(
    (how: 'escape' | 'away' = 'away') => {
      setMenu(null);
      // Out by `Escape`, the keys go back to the tree they came from.
      if (how === 'escape') {
        scroller.current?.focus();
      }
    },
    [scroller],
  );
  const menuItems = (target: readonly Described[]) => actionsFor(target, { listingOf, prepared });
  // A right-click, or the "⋮", on the row `key`: the selection where the row is
  // in one of several, and the row alone otherwise.
  const openMenu = (key: string, at: MenuAt) => {
    // The "⋮" of a row the menu is open on closes it, as a button that says
    // it has opened something does when pressed again.
    if (at.side === 'end' && menu !== null && menu.rows.some((one) => one.key === key)) {
      closeMenu('escape');
      return;
    }
    const { picked: next, target } = rightClicked(rows, picked, key);
    if (next !== picked) {
      pick(next);
    }
    if (menuItems(target).length > 0) {
      setMenu({ at, rows: target });
    }
  };
  const choose = (item: ActionItem) => {
    setMenu(null);
    // The keys go back to the tree, so that the arrows walk the rows again.
    scroller.current?.focus();
    const action = item.action;
    switch (action.kind) {
      case 'open':
        openRow(action.row);
        break;
      case 'newFolder':
        onNewFolder(action.folder);
        break;
      case 'pack':
        onPack(action.folder);
        break;
      case 'delete':
        onDelete(action.target);
        break;
      case 'forget':
        onForget(action.folder);
        break;
    }
  };

  // What the block over the rows says about the folder the focus is in.
  const root = contextPath === '';
  // A path the Library names nothing at, as far as what is on hand goes: a
  // folder of the Library is what the separators under it imply, so `held`
  // false means nothing is under it — except the folder of a book whose
  // freeze has not committed, and a folder the Library does not hold yet,
  // which are states rather than mistakes.
  const unheld = listing !== null && !listing.held && !stranded && !prospectiveHere;
  // And the same thing said out loud, which waits for the one answer that can
  // still overturn it: the stranded folders are read off a request of their
  // own, and until it lands a folder whose book is being packed this minute
  // would read as nothing at all.
  const nowhere = unheld && strandedKnown;
  const childFolders = rows.filter((row) => row.kind === 'folder' && parentOf(row.path) === contextPath).length;
  // A Library root with no mapping of its own, no files sitting in it and
  // folders below it is the ordinary shape of a device that mapped one
  // top-level folder: the rows are folders that say for themselves, and there
  // is nothing for a banner to explain. Nor over a path the Library names
  // nothing at, where being offered to map it would have somebody choose a
  // folder on this device for a part of the Library that does not exist.
  const sayUnmapped =
    listing !== null &&
    !listing.mapped &&
    !unheld &&
    !(root && listing.files.length === 0 && childFolders > 0);
  // What is happening to this folder, said over the rows because it is true of
  // every one of them: the pages go up together, as Packs.
  const packing = listing !== null && freezingHere(freeze, listing.path);
  const emptyHere =
    listing !== null && listing.files.length === 0 && childFolders === 0;

  // What a drop comes to, read for the folder it would land in: the folder row
  // under the pointer, the folder a file row under it stands in, and the
  // focused folder anywhere else on the list. A folder no folder of this
  // device is chosen for takes it as any other — the screen asks for one —
  // and only a path the Library holds nothing at refuses it.
  const drag = useDragOver();
  const [over, setOver] = useState<string | null>(null);
  useEffect(() => {
    if (!drag.dragged) {
      setOver(null);
    }
  }, [drag.dragged]);
  const target = over ?? contextPath;
  const refusedHere = target === contextPath && nowhere;
  const outcome = dropOutcome({ held: !refusedHere, freezing: isFreezing(freeze) });
  // The row the drop lands in, outlined while the drag is over the tree; the
  // whole list where no row stands for it — the Library root.
  const outlinedRow = rows.some((row) => row.kind === 'folder' && row.path === target);
  const onDrop = (event: DragEvent) => {
    event.preventDefault();
    drag.ended();
    setOver(null);
    if (refusedHere) {
      // Said and not merely not done: a drop is a gesture with an outcome, and
      // a screen that goes on looking exactly as it did tells nobody that none
      // of those files were added.
      onUnmapped('add', false);
      return;
    }
    // The walk is asynchronous and the event is not: what it carries is gone
    // by the first await, so the traversal is started here, and announced
    // first, because the walk itself is a wait a person is owed a word about.
    onCollecting();
    void droppedFiles(event.dataTransfer).then((drop) => onAdd(target, drop), onUnreadable);
  };

  // Opening a folder also opens it in the tree, so that the arrows go on
  // straight into its files and the sheet, where it opened, follows them.
  const openRow = (row: FolderRow | FileRow) => {
    if (row.kind === 'folder' && !state.expanded.has(row.path)) {
      onChange(toggled({ ...state, focus: row.key }, row.path));
    }
    opened(row, { onOpenFolder, onOpenFile, onUnsupported, onUnmapped });
  };
  // `Escape` climbs: the focus goes to the folder the focused row stands in, and
  // from a top-level row, which stands in the root, it is let go of.
  const climbOrLetGo = (focus: string) => {
    const parent = parentOf(keyed(focus).path);
    if (parent === '') {
      letGo();
      return;
    }
    onChange({ ...state, focus: folderKey(parent) });
    if (selection.keys.size > 0) {
      onSelect(NOTHING_SELECTED);
    }
  };
  // Nothing focused: the screen's actions — the header's **+**, a drop on the
  // list's empty space — are about the Library root again.
  const letGo = () => {
    if (state.focus !== null) {
      onChange({ ...state, focus: null });
    }
    if (selection.keys.size > 0) {
      onSelect(NOTHING_SELECTED);
    }
  };
  const focused = rows.find(
    (row): row is FolderRow | FileRow => row.kind !== 'note' && row.key === state.focus,
  );

  // The keys the tree answers while it has the focus itself — which a click on
  // a row gives it — and not while a control inside it does: `Enter` on a
  // row's "⋮" or in the menu is that control's.
  //
  // Shift with `↑`/`↓` grows the selection to the row arrived at; any other
  // arrow drops it (see [`stepped`](./selection)). No key deletes: which key
  // means delete differs between systems, and invites mistakes.
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget) {
      return;
    }
    if (event.key === 'Enter') {
      if (focused !== undefined) {
        event.preventDefault();
        openRow(focused);
      }
      return;
    }
    if (event.key === 'Escape') {
      if (escapeLetsGo && state.focus !== null) {
        event.preventDefault();
        climbOrLetGo(state.focus);
      }
      return;
    }
    if (!ARROWS.has(event.key)) {
      return;
    }
    event.preventDefault();
    const next = arrowed(rows, state, event.key as Arrow);
    const extend = event.shiftKey && (event.key === 'ArrowUp' || event.key === 'ArrowDown');
    const kept = stepped(rows, picked, next.focus, extend);
    if (next !== state) {
      onChange(next);
    }
    // A plain arrow that moved nothing still drops the selection, as one
    // that moved would.
    if (next !== state || extend || selection.keys.size > 0) {
      onSelect(kept);
    }
  };
  // Whether the focus the tree is about to take comes from a press on it, which
  // focuses the row pressed, rather than from the keyboard.
  const pressed = useRef(false);
  // Tabbing into the tree with no row focused focuses the first, so that the
  // focus is drawn on the screen and the keys have somewhere to start.
  const onFocus = (event: FocusEvent<HTMLDivElement>) => {
    const byPress = pressed.current;
    pressed.current = false;
    if (event.target !== event.currentTarget || byPress || state.focus !== null) {
      return;
    }
    const first = rows.find((row) => row.kind !== 'note');
    if (first !== undefined) {
      onChange({ ...state, focus: first.key });
    }
  };

  // The list scrolls ahead of the focus: the focused row is kept three rows
  // from either edge of what is on show, and is kept in view as folders open
  // and close. Only when the focus moves, a folder opens or closes, or the
  // focused row is first drawn — a folder restored from the URL several
  // levels deep is drawn as the listings above it arrive — and not on every
  // answer: a listing asked again must not pull back a list somebody
  // scrolled away.
  const focusDrawn = focused !== undefined;
  useLayoutEffect(() => {
    const area = scroller.current;
    const row = state.focus === null ? undefined : drawn.current.get(state.focus);
    if (area === null || row === undefined) {
      return;
    }
    const box = row.getBoundingClientRect();
    const top = box.top - area.getBoundingClientRect().top + area.scrollTop;
    const to = scrollAhead({
      top,
      bottom: top + box.height,
      scrollTop: area.scrollTop,
      height: area.clientHeight,
      covered: block.current?.offsetHeight ?? 0,
      margin: ROWS_AHEAD * box.height,
    });
    if (to !== null) {
      area.scrollTop = to;
    }
    // The rows are read through the refs; what decides when is above.
  }, [state.focus, state.expanded, focusDrawn]);

  // The value columns, each as wide as its widest string on show.
  const measured = useMemo(
    () =>
      measure ??
      canvasMeasure({ heading: HEAD_FONT, value: ROW_FONT }) ??
      ((text: string) => text.length * 8),
    [measure],
  );
  const values = rows.flatMap((row) => (row.kind === 'note' ? [] : [valuesOf(row)]));
  const widths = {
    size: columnWidth(
      'Size',
      values.map((value) => value.size),
      measured,
      2 * CELL_PAD,
    ),
    // Its right edge kept close to the "⋮" beside it, which is about the same row.
    modified: columnWidth(
      'Modified',
      values.map((value) => value.modified),
      measured,
      CELL_PAD + MODIFIED_END_PAD,
    ),
  };
  const columns = <Columns widths={widths} />;

  // What the "⋮" of a row would open on: the selection where the row is in
  // one of several, and the row alone otherwise — the same rows a right-click
  // there acts on. A row with nothing to offer draws no "⋮".
  const selected = selectedRows(rows, picked);
  const offersOf = (row: FolderRow | FileRow): boolean =>
    menuItems(selected.length > 1 && selected.some((one) => one.key === row.key) ? selected : [row]).length > 0;

  return (
    // The tree and, over its bottom edge, the pill a drag brings up: a frame
    // of their own, so that the pill stays where it is as the rows scroll.
    <div style={{ flex: 1, display: 'flex', minWidth: 0, position: 'relative' }}>
      <div
        ref={scroller}
        role="tree"
        aria-label="Library"
        tabIndex={0}
        onKeyDown={onKeyDown}
        onPointerDown={() => {
          pressed.current = true;
        }}
        // A click on the list's empty space, below the last row, lets go of
        // the focus, as a file manager's does.
        onClick={(event) => {
          if (event.target === event.currentTarget) {
            letGo();
          }
        }}
        onFocus={onFocus}
        onBlur={(event) => {
          if (event.target === event.currentTarget) {
            pressed.current = false;
          }
        }}
        onDragEnter={drag.onDragEnter}
        onDragOver={drag.onDragOver}
        onDragLeave={drag.onDragLeave}
        onDrop={onDrop}
        // The menu is fixed to the window and the rows it was opened on would
        // move from under it as the list is scrolled, so it goes rather than
        // float over others. The wheel and not every scroll: opening it can move
        // the focus, and the list scrolling ahead of the focus is not somebody
        // scrolling away.
        onWheel={menu === null ? undefined : () => closeMenu()}
        style={{
          flex: 1,
          overflow: 'auto',
          minWidth: 0,
          // The tree's own focus is drawn by the focused row, so the outline is a
          // drag's alone — around the row of the folder the drop lands in, and
          // around the whole list where no row stands for it (the Library root).
          outline: outlinedRow ? 'none' : dropOutline(drag.dragged, outcome),
          outlineOffset: -2,
        }}
      >
        {/* The banners stay at the top as the list scrolls,
            together: every one of them is true of the whole folder rather than
            of whichever rows happen to be in view. */}
        <div ref={block} style={{ position: 'sticky', top: 0, zIndex: 1 }}>
          {sayUnmapped && listing !== null && (
            <Unmapped
              root={root}
              path={listing.path}
              files={listing.files.length > 0}
              folders={childFolders > 0}
              onMap={onMap}
            />
          )}
          {packing && <Packing />}
          {emptyHere && rows.length > 0 && (
            <Banner tone={COLOR.dim} background={COLOR.panel}>
              {emptyLine(listing, root, nowhere, prospectiveHere)}
            </Banner>
          )}
          <table style={{ ...TABLE, font: HEAD_FONT }}>
            {columns}
            <thead>
              <tr style={{ ...HEAD_ROW, color: COLOR.dim, textAlign: 'left' }}>
                <th style={HEAD}>Name</th>
                <th style={HEAD}>Size</th>
                <th style={HEAD}>
                  <span style={HIDDEN}>On this device</span>
                </th>
                <th style={{ ...HEAD, paddingRight: MODIFIED_END_PAD }}>Modified</th>
                <th style={HEAD} />
              </tr>
            </thead>
          </table>
        </div>
        {rows.length === 0 ? (
          <p style={{ padding: 16, color: COLOR.dim }}>
            {emptyLine(listing, root, nowhere, prospectiveHere)}
          </p>
        ) : (
          <table style={{ ...TABLE, font: ROW_FONT }}>
            {columns}
            {/* No text selection across rows: Shift+click selects rows here, as
                in a file manager, and would otherwise also select their text. */}
            <tbody style={{ userSelect: 'none' }}>
              {rows.map((row) =>
                row.kind === 'note' ? (
                  <tr key={row.key}>
                    <td
                      colSpan={5}
                      style={{
                        ...CELL,
                        paddingLeft: indent(row.depth) + DISCLOSURE,
                        color: row.refused === null ? COLOR.dim : COLOR.refused,
                      }}
                    >
                      {row.refused === null ? (
                        'loading…'
                      ) : (
                        // Offered on the line itself: an open folder below the
                        // one the focus is in has no other place that says its
                        // rows did not come, nor anything to press about it.
                        <>
                          {row.refused}
                          <button type="button" onClick={() => onAskAgain(row.path)} style={{ ...BAR_BUTTON, marginLeft: 10 }}>
                            {TRY_AGAIN}
                          </button>
                        </>
                      )}
                    </td>
                  </tr>
                ) : (
                  <Row
                    key={row.key}
                    row={row}
                    rowRef={(element) => {
                      if (element === null) {
                        drawn.current.delete(row.key);
                      } else {
                        drawn.current.set(row.key, element);
                      }
                    }}
                    focused={state.focus === row.key}
                    selected={isSelected(selection, row.key)}
                    // Taken out of the selection with ⌘/Ctrl+click, the focus
                    // is left on a row that is not selected.
                    focusOutside={selection.keys.size > 0 && !isSelected(selection, row.key)}
                    drop={row.kind === 'folder' && row.path === target && drag.dragged ? outcome : null}
                    shown={row.kind === 'file' ? rowFill(row.file, row.in.path, fillOf(row.in.path)) : null}
                    offers={offersOf(row)}
                    menuOpen={menu !== null && menu.rows.some((one) => one.key === row.key)}
                    onPress={(how) => pick(clicked(rows, picked, row.key, how))}
                    onMenu={(at) => openMenu(row.key, at)}
                    onToggle={() => {
                      scroller.current?.focus();
                      onChange(toggled(state, row.path));
                    }}
                    onOpen={() => openRow(row)}
                    onDragOverRow={() => setOver(landingOf(row.key))}
                  />
                ),
              )}
            </tbody>
          </table>
        )}
        {menu !== null && (
          <ActionsMenu
            title={menuTitle(menu.rows)}
            label={`actions for ${menuTitle(menu.rows)}`}
            at={menu.at}
            items={menuItems(menu.rows)}
            onChoose={choose}
            onClose={closeMenu}
          />
        )}
      </div>
      {drag.dragged && drag.carriesFiles && <DropPill outcome={outcome} landing={target} />}
    </div>
  );
}

/** What the actions menu over `rows` acts on: the one row's name, or how many rows there are. */
function menuTitle(rows: readonly Described[]): string {
  return rows.length === 1 ? rows[0].name : counted(rows.length, 'item', 'items');
}

/** What opening a row can come to, as [`opened`](#opened) hands it on. */
export interface Openers {
  /**
   * A folder row opened: the reader's sheet, on the folder's cover — where it
   * holds images, which only its listing can say.
   */
  onOpenFolder: (path: string) => void;
  /** A file row opened that the reader can show: the reader's sheet, on the file. */
  onOpenFile: (path: string) => void;
  /** A file row opened that the reader cannot show. */
  onUnsupported: (file: ListedFile) => void;
  /** A file row opened in a folder with nowhere on this device to put it. */
  onUnmapped: (tried: Tried, held: boolean) => void;
}

/**
 * What opening a row does: what a double-click, a tap, `Enter`, and the
 * details pane's Open all reach, so that the four are one gesture. A folder no
 * mapping reaches has nowhere on this device to put a file, so a file there is
 * not opened and the sentence says why instead. The sheet opens only on a row
 * the reader can show — an image, or a folder holding images (the details
 * pane's [`opensInReader`](./details), which is why the pane offers Open only
 * there); a file it cannot show is not opened and the sentence says so. Once
 * the sheet is open it follows the focus onto any row, and says there what it
 * cannot draw.
 *
 * A folder opened is shown in the sheet, its cover, with the focus left on its
 * row; the tree's `Enter`, double-click and Open also open the folder in the
 * tree (see `openRow`), so the arrows go on into its files. Opening never
 * closes a folder.
 */
export function opened(row: FolderRow | FileRow, openers: Openers): void {
  if (row.kind === 'folder') {
    openers.onOpenFolder(row.path);
    return;
  }
  if (!row.in.mapped) {
    openers.onUnmapped('open', row.in.held || row.in.path === '');
  } else if (row.file.openable) {
    openers.onOpenFile(row.path);
  } else {
    openers.onUnsupported(row.file);
  }
}

/** The keys that move the focus. */
const ARROWS = new Set(['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight']);

/**
 * What the Library root says when it holds nothing and a mapping of this
 * device reaches it: the two ways to put the first files in, since nothing
 * else on the screen offers them.
 */
export const EMPTY_MAPPED_ROOT =
  'this Library is empty — add files with + above the list or drop them here, or choose ' +
  '“New folder…” to prepare where they go';

/** What a folder row's "no folder chosen" says when pointed at. */
const NO_FOLDER_CHOSEN_TITLE =
  'No folder on this device is chosen for this part of the Library, so its files cannot be fetched here';

/**
 * What an empty folder says, in three states and not two: a folder of the
 * Library holds something by definition, so "this folder is empty" over a path
 * the Library has never held would invent a folder to describe.
 */
function emptyLine(listing: Listing | null, root: boolean, nowhere: boolean, prospectiveHere: boolean): string {
  if (nowhere) {
    return NOTHING_AT_THIS_PATH;
  }
  if (prospectiveHere) {
    return PROSPECTIVE;
  }
  if (root) {
    return listing?.mapped === true ? EMPTY_MAPPED_ROOT : 'this Library is empty';
  }
  return 'this folder is empty';
}

function Row({
  row,
  rowRef,
  focused,
  selected,
  focusOutside,
  drop,
  shown,
  offers,
  menuOpen,
  onPress,
  onMenu,
  onToggle,
  onOpen,
  onDragOverRow,
}: {
  row: FolderRow | FileRow;
  rowRef: (element: HTMLTableRowElement | null) => void;
  focused: boolean;
  /** Selected beside the focus: tinted, where the focused row is highlighted. */
  selected: boolean;
  /**
   * Whether others are selected and this row is not, so that where the focus
   * is on it, it is drawn by a ring rather than the strong highlight: the
   * highlight would read as selected, and the details pane and the menu act on
   * the rows selected.
   */
  focusOutside: boolean;
  /** What a drop would come to where the drag over the tree lands in this row's folder; `null` where it does not. */
  drop: DropOutcome | null;
  /** What a file row's state is, the listing's and the fill's together; `null` for a folder. */
  shown: RowFill | null;
  /** Whether the actions menu has anything for this row, which is whether it draws a "⋮". */
  offers: boolean;
  /** Whether the actions menu is open on this row, which keeps its "⋮" on show. */
  menuOpen: boolean;
  /** A click on the row, with what the keys held with it ask for (see [`clickOf`](./selection)). */
  onPress: (how: Click) => void;
  /** The actions menu asked for: by a right-click at the pointer, or by the "⋮". */
  onMenu: (at: MenuAt) => void;
  onToggle: () => void;
  onOpen: () => void;
  onDragOverRow: () => void;
}) {
  // How the last press on the row was made: a tap opens, a click from a mouse
  // or a pen focuses. Kept past the click, since the double-click a browser
  // makes of two taps comes after the second tap's click.
  const pointer = useRef<string | null>(null);
  const folder = row.kind === 'folder' ? row : null;
  const values = valuesOf(row);
  const offered = row.kind === 'folder' || row.in.mapped;
  const highlighted = focused && !focusOutside;
  const ring = drop !== null ? dropOutline(true, drop) : focused && focusOutside ? `1px solid ${COLOR.accent}` : 'none';
  // Whether the row is drawn on the accent: the focused row, unless a drag over
  // it has it tinted as the drop target instead — white words would vanish on
  // the light tint.
  const onAccent = highlighted && drop === null;
  // What is drawn on the accent of the focused row is drawn in white, the
  // name, the values and the icons alike; elsewhere each keeps its own colour.
  const ink = (own: string) => (onAccent ? COLOR.onAccent : own);
  const icon = rowIconOf(row);
  return (
    <tr
      ref={rowRef}
      className={offered ? 'row activatable' : 'row'}
      aria-selected={highlighted || selected}
      // What shows the row's "⋮" beside the pointer's hover (see `index.html`).
      data-focused={focused ? '' : undefined}
      title={row.name}
      onPointerDown={(event) => {
        pointer.current = event.pointerType;
      }}
      onClick={(event) => {
        const how = clickOf(event);
        onPress(how);
        if (how === 'plain' && pointer.current === 'touch') {
          onOpen();
        }
      }}
      onDoubleClick={(event) => {
        if (pointer.current !== 'touch' && clickOf(event) === 'plain') {
          onOpen();
        }
      }}
      onContextMenu={(event) => {
        event.preventDefault();
        onMenu({ x: event.clientX, y: event.clientY, side: 'start' });
      }}
      onDragOver={onDragOverRow}
      style={{
        background:
          drop !== null ? dropGround(drop) : onAccent ? COLOR.accent : selected ? COLOR.picked : undefined,
        outline: ring,
        outlineOffset: -2,
      }}
    >
      <td style={{ ...CELL, paddingLeft: indent(row.depth) }}>
        <span style={{ display: 'flex', alignItems: 'center', minWidth: 0 }}>
          {folder !== null && folder.expandable ? (
            <button
              type="button"
              tabIndex={-1}
              // What it will do when pressed, which is the opposite of what it
              // last did.
              aria-label={`${folder.expanded ? 'collapse' : 'expand'} ${row.name}`}
              aria-expanded={folder.expanded}
              // The focus stays on the tree, so the keys go on walking it.
              onMouseDown={(event) => event.preventDefault()}
              onClick={(event) => {
                event.stopPropagation();
                onToggle();
              }}
              onDoubleClick={(event) => event.stopPropagation()}
              style={{ ...DISCLOSURE_BUTTON, color: ink(COLOR.dim) }}
            >
              <Chevron open={folder.expanded} />
            </button>
          ) : (
            <span aria-hidden style={{ flex: '0 0 auto', width: DISCLOSURE }} />
          )}
          <span style={{ ...MARK, color: ink(icon === 'folder' ? COLOR.dim : COLOR.accent) }}>
            <RowIconMark icon={icon} />
          </span>
          <span
            style={{
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
              color: ink(
                folder !== null
                  ? folder.prospective || folder.stranded
                    ? COLOR.accentDeep
                    : COLOR.text
                  : row.kind === 'file' && !row.file.openable
                    ? COLOR.dim
                    : COLOR.text,
              ),
            }}
          >
            {row.name}
          </span>
          {folder !== null && folder.prospective && (
            <span title={PROSPECTIVE} style={{ ...NOTE, color: ink(COLOR.accentDeep) }}>
              not in Library
            </span>
          )}
          {folder !== null && !folder.folder.mapped && !folder.prospective && (
            <span title={NO_FOLDER_CHOSEN_TITLE} style={{ ...NOTE, color: ink(COLOR.warn) }}>
              no folder chosen
            </span>
          )}
          {folder?.incoming === true && <span style={{ ...NOTE, color: ink(COLOR.accentDeep) }}>adding…</span>}
        </span>
      </td>
      {/* A folder's size is the total of everything under it, drawn a step
          fainter than a file's so the two are not read as the same thing. */}
      <td
        title={folder !== null && values.size !== '' ? FOLDER_SIZE_TITLE : undefined}
        style={{ ...CELL, textAlign: 'right', color: ink(folder !== null ? COLOR.faint : COLOR.dim) }}
      >
        {values.size}
      </td>
      <td style={{ ...CELL, textAlign: 'center', textOverflow: 'clip', padding: `0 ${DEVICE_PAD}px` }}>
        {folder !== null ? (
          folder.folder.present_throughout && !folder.prospective ? <Device state="present" onAccent={onAccent} /> : null
        ) : shown !== null ? (
          <Device state={shown.state} message={shown.message} onAccent={onAccent} />
        ) : null}
      </td>
      <td style={{ ...CELL, paddingRight: MODIFIED_END_PAD, color: ink(COLOR.dim) }}>{values.modified}</td>
      <MenuCell name={row.name} offers={offers} open={menuOpen} ink={ink(COLOR.dim)} onMenu={onMenu} />
    </tr>
  );
}

/**
 * What one row shows in the Size and Modified columns, as the strings drawn.
 *
 * A file's are its own. A folder's are the Library's answers about everything
 * below it — the bytes of every file at every depth, and when the newest of
 * them was modified (see [`ListedFolder`](../../../gateway/api/src/list.ts)) —
 * formatted the same way, so a book reads beside its pages in one column. A
 * prepared folder and one waiting to be packed have nothing in the Library to
 * measure, and their row already says so, so their cells stay empty rather
 * than stating a zero that is not the folder's.
 *
 * One function for the cells and for the column widths, so that a column is
 * always as wide as the widest value it actually draws.
 */
function valuesOf(row: FolderRow | FileRow): { size: string; modified: string } {
  if (row.kind === 'file') {
    return { size: size(row.file.size), modified: time(row.file.mtime) };
  }
  if (row.prospective || row.stranded) {
    return { size: '', modified: '' };
  }
  return { size: size(row.folder.size), modified: time(row.folder.mtime) };
}

/**
 * Whether one row is on this device, as an icon in its narrow column: nothing
 * for a row in the Library only, a laptop for one on this device, a small ring
 * while it is being fetched or uploaded, and a warning triangle where that
 * failed. The words are its label, and the sentence behind the state its
 * title. On the focused row's accent every one of them is drawn in white, as
 * the rest of the row is.
 */
function Device({
  state,
  message = null,
  onAccent,
}: {
  state: RowFill['state'];
  message?: string | null;
  onAccent: boolean;
}) {
  const ink = (own: string) => (onAccent ? COLOR.onAccent : own);
  switch (state) {
    case 'remote':
      return null;
    case 'present':
      return <Laptop color={ink(COLOR.present)} label={ON_THIS_DEVICE} />;
    case 'fetching':
      return <Ring color={ink(COLOR.accent)} label="Being fetched to this device" />;
    case 'added':
      return <Ring color={ink(COLOR.accent)} label={said('On this device, being added to the Library', message)} />;
    case 'reserved':
      return (
        <Ring color={ink(COLOR.accent)} label={said('On this device, waiting to be packed into the Library', message)} />
      );
    case 'failed':
      return <Warning color={ink(COLOR.refused)} label={said('Fetching failed', message)} />;
    case 'declined':
      return <Warning color={ink(COLOR.warn)} label={said('Not fetched', message)} />;
  }
}

/** A state's words, and the sentence behind it after them where there is one. */
function said(state: string, message: string | null): string {
  return message === null ? state : `${state} — ${message}`;
}

/** What the laptop says: a file fetched here and one added from here alike. */
export const ON_THIS_DEVICE = 'On this device';

function Laptop({ color, label }: { color: string; label: string }) {
  return (
    <svg role="img" aria-label={label} width={ICON} height={ICON} viewBox="0 0 16 16" style={ICON_STYLE}>
      <title>{label}</title>
      <rect x="3" y="3.5" width="10" height="7" rx="1" fill="none" stroke={color} strokeWidth={1.2} />
      <path d="M1.5 12.5h13" stroke={color} strokeWidth={1.4} strokeLinecap="round" />
    </svg>
  );
}

function Ring({ color, label }: { color: string; label: string }) {
  return (
    <svg role="img" aria-label={label} width={ICON} height={ICON} viewBox="0 0 16 16" style={ICON_STYLE}>
      <title>{label}</title>
      <circle cx="8" cy="8" r="5.5" fill="none" stroke={COLOR.border} strokeWidth={2} />
      <path d="M8 2.5a5.5 5.5 0 0 1 5.5 5.5" fill="none" stroke={color} strokeWidth={2} strokeLinecap="round">
        <animateTransform attributeName="transform" type="rotate" from="0 8 8" to="360 8 8" dur="1.2s" repeatCount="indefinite" />
      </path>
    </svg>
  );
}

function Warning({ color, label }: { color: string; label: string }) {
  return (
    <svg role="img" aria-label={label} width={ICON} height={ICON} viewBox="0 0 16 16" style={ICON_STYLE}>
      <title>{label}</title>
      <path d="M8 1.8l6.6 11.7H1.4z" fill="none" stroke={color} strokeWidth={1.3} strokeLinejoin="round" />
      <path d="M8 6v3.6M8 11.2v.4" stroke={color} strokeWidth={1.4} strokeLinecap="round" />
    </svg>
  );
}

/**
 * The icon in front of a row's name (see [`rowIconOf`](./rowIcon)), in the
 * colour of the mark it stands in.
 *
 * The folder's is read out as `folder`, which is how a row is told for a
 * folder's from the outside; a file's is left unread, its name being all it
 * says — and the one image a file row holds stays the state icon in its narrow
 * column.
 */
function RowIconMark({ icon }: { icon: RowIcon }) {
  const shared = {
    width: ICON,
    height: ICON,
    viewBox: '0 0 24 24',
    fill: 'none',
    stroke: 'currentColor',
    strokeWidth: 2,
    strokeLinecap: 'round',
    strokeLinejoin: 'round',
    style: ICON_STYLE,
    'data-icon': icon,
  } as const;
  switch (icon) {
    case 'folder':
      return (
        <svg role="img" aria-label="folder" {...shared}>
          <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
        </svg>
      );
    case 'image':
      return (
        <svg aria-hidden {...shared}>
          <rect x={4} y={3} width={16} height={18} rx={2} />
          <circle cx={9.5} cy={9} r={1.5} />
          <path d="m20 16-4.5-4.5L7 20" />
        </svg>
      );
    case 'file':
      return (
        <svg aria-hidden {...shared}>
          <path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" />
          <path d="M14 3v6h6" />
        </svg>
      );
  }
}

/** A folder's disclosure: a chevron pointing at the name, turned down a quarter while the folder is open. */
function Chevron({ open }: { open: boolean }) {
  return (
    <svg
      aria-hidden
      width={12}
      height={12}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ display: 'block', transform: open ? 'rotate(90deg)' : undefined }}
    >
      <path d="m9 6 6 6-6 6" />
    </svg>
  );
}

/**
 * The cell a row's "⋮" stands in, which opens the actions menu under it. A
 * click anywhere in the cell stops there: the row under it focuses and opens,
 * and a press meant for the menu must not also reach the row.
 *
 * The mark is drawn on the focused row and the row under the pointer only (see
 * `index.html`): one quiet mark on every row is twenty marks down a list of
 * twenty rows, for something pressed now and then.
 */
function MenuCell({
  name,
  offers,
  open,
  ink,
  onMenu,
}: {
  name: string;
  offers: boolean;
  open: boolean;
  /** The colour the mark is drawn in: the row's dim, or white on the focused row. */
  ink: string;
  onMenu: (at: MenuAt) => void;
}) {
  const stop = offers ? (event: MouseEvent) => event.stopPropagation() : undefined;
  return (
    <td
      style={{ ...CELL, overflow: 'visible', textAlign: 'center', padding: '0 2px' }}
      onClick={stop}
      onDoubleClick={stop}
    >
      {offers && (
        <button
          type="button"
          className="row-actions"
          aria-label={`actions for ${name}`}
          aria-haspopup="menu"
          aria-expanded={open}
          // The menu closes on a press outside it, which this is; the click
          // below is what opens it, or closes it where it is open on this row.
          onMouseDown={(event) => event.stopPropagation()}
          onClick={(event) => {
            const box = event.currentTarget.getBoundingClientRect();
            // A click with no pointer behind it is `Enter` or Space on the "⋮".
            onMenu({ x: box.right, y: box.bottom, side: 'end', byKeys: event.detail === 0 });
          }}
          style={{ ...MENU_BUTTON, color: ink }}
        >
          ⋮
        </button>
      )}
    </td>
  );
}

/** Said while the book in the focused folder is being packed. */
function Packing() {
  return (
    <Banner tone={COLOR.accentDeep} background={COLOR.accentTint}>
      packing this folder into the Library — the pages go up together, as Packs,
      and become ordinary rows when the batch commits
    </Banner>
  );
}

/**
 * Said over the rows where no folder of this device is chosen for the folder
 * the focus is in, with the button that chooses one for the top-level folder
 * it is under — or for the whole Library — since a choice is keyed by one
 * top-level component (spec: EP-9). Never that the folder is "not on this
 * device": that is what the column says of a file that is not here.
 */
function Unmapped({
  root,
  path,
  files,
  folders,
  onMap,
}: {
  root: boolean;
  path: string;
  files: boolean;
  folders: boolean;
  onMap: (prefix: string | null) => void;
}) {
  const top = path.split('/')[0];
  const button = (prefix: string | null) => (
    <>
      {' — '}
      <button onClick={() => onMap(prefix)} style={MAP_BUTTON}>
        {CHOOSE_A_FOLDER}
      </button>
    </>
  );
  return (
    <Banner tone={COLOR.warn} background={COLOR.warnGround}>
      {root && !files && !folders ? (
        <>
          {noFolderChosenLine(null)}: this Library holds nothing yet, and a folder on this
          device is where its first files are kept
          {button(null)}
        </>
      ) : root ? (
        <>
          {noFolderChosenLine(null)}: files sitting directly in it cannot be fetched, though
          a folder below can have one of its own
          {button(null)}
        </>
      ) : (
        <>
          {noFolderChosenLine(top)}: choose one{' '}
          {files
            ? 'to fetch its files'
            : folders
              ? 'to fetch what is in the folders below it'
              : 'before putting anything in it'}
          {button(top)}
        </>
      )}
    </Banner>
  );
}

/**
 * The widths of the columns, given to both tables so the two line up: the name
 * (the rest of the width), the size, whether the row is on this device, the
 * time, and the "⋮".
 */
function Columns({ widths }: { widths: { size: number; modified: number } }) {
  return (
    <colgroup>
      <col />
      <col style={{ width: widths.size }} />
      <col style={{ width: ICON + 2 * DEVICE_PAD }} />
      <col style={{ width: widths.modified }} />
      <col style={{ width: MENU_BUTTON_SIZE + 4 }} />
    </colgroup>
  );
}

/** The fonts the headings and the rows are drawn in, which their widths are measured in. */
const HEAD_FONT = `500 12px ${FONT}`;
const ROW_FONT = `13px ${FONT}`;

/** The padding either side of a value. */
const CELL_PAD = 10;

/** What a folder's size is, said where the pointer rests on it. */
export const FOLDER_SIZE_TITLE = 'Total of everything inside';

/** The room after the modified time, less than a cell's: the row's "⋮" follows it. */
const MODIFIED_END_PAD = 2;

/** How wide an icon is drawn, and the room either side of one in its column. */
const ICON = 16;
const DEVICE_PAD = 6;

/** The room a disclosure button takes, kept on a row without one. */
const DISCLOSURE = 18;

/** How far in a row at `depth` starts: 14px a level. */
function indent(depth: number): number {
  return 6 + depth * 14;
}

/** The least height of a row. */
const ROW_HEIGHT = 32;

/** The side of a row's "⋮", square. */
const MENU_BUTTON_SIZE = 28;

const TABLE: CSSProperties = { width: '100%', borderCollapse: 'collapse', tableLayout: 'fixed' };

const HEAD: CSSProperties = {
  padding: `8px ${CELL_PAD}px`,
  borderBottom: `1px solid ${COLOR.border}`,
  fontWeight: 500,
  whiteSpace: 'nowrap',
  overflow: 'hidden',
};

const CELL: CSSProperties = {
  // A table cell's height is the least it is drawn at, as a row's is.
  height: ROW_HEIGHT,
  boxSizing: 'border-box',
  padding: `0 ${CELL_PAD}px`,
  borderBottom: `1px solid ${COLOR.rowRule}`,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
};

/** A heading read out and not drawn: the icon column's. */
const HIDDEN: CSSProperties = {
  position: 'absolute',
  width: 1,
  height: 1,
  overflow: 'hidden',
  clip: 'rect(0 0 0 0)',
  whiteSpace: 'nowrap',
};

const ICON_STYLE: CSSProperties = { display: 'block', margin: '0 auto', flex: '0 0 auto' };

const DISCLOSURE_BUTTON: CSSProperties = {
  flex: '0 0 auto',
  width: DISCLOSURE,
  height: DISCLOSURE,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  border: 'none',
  background: 'none',
  cursor: 'pointer',
  padding: 0,
};

/** The room the row's icon takes, and the 6px between it and the name. */
const MARK: CSSProperties = { flex: '0 0 auto', width: ICON, marginRight: 6 };

/** What a row says after its name. */
const NOTE: CSSProperties = { marginLeft: 10, fontSize: 11, flex: '0 0 auto' };

/** The headings' row. */
const HEAD_ROW: CSSProperties = { background: COLOR.ground };

/** A row's "⋮". */
const MENU_BUTTON: CSSProperties = {
  width: MENU_BUTTON_SIZE,
  height: MENU_BUTTON_SIZE,
  verticalAlign: 'middle',
  border: 'none',
  background: 'transparent',
  font: 'inherit',
  fontSize: 16,
  lineHeight: 1,
  padding: 0,
  borderRadius: 6,
  cursor: 'pointer',
};

/** A small button inside a row, such as "ask again" on a folder whose listing was refused. */
const BAR_BUTTON: CSSProperties = {
  border: `1px solid ${COLOR.border}`,
  background: 'transparent',
  color: COLOR.text,
  font: 'inherit',
  fontSize: 12,
  padding: '1px 8px',
  borderRadius: 4,
  cursor: 'pointer',
};

const MAP_BUTTON: CSSProperties = {
  marginLeft: 6,
  border: `1px solid ${COLOR.warn}`,
  background: 'transparent',
  color: COLOR.warn,
  font: 'inherit',
  padding: '0 8px',
  borderRadius: 4,
  cursor: 'pointer',
};
