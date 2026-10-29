import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type DragEvent,
  type FocusEvent,
  type KeyboardEvent,
} from 'react';

import type { DeleteTarget, DisplacedFill, Fill, Freeze, ListedFile, Listing } from '@coffret/api';

import { Banner, DropBanner, dropOutline } from './Banner';
import { canvasMeasure, columnWidth, type Measured } from './columns';
import { deleteOffered, DELETE } from './deleteEntries';
import { useDragOver } from './dragOver';
import { droppedFiles, type Drop } from './drop';
import { dropOutcome } from './dropTarget';
import { freezingHere, isFreezing, rowFill, type RowFill } from './fill';
import { size, time } from './humanize';
import { mapLabel, MAP_THE_ROOT } from './mapping';
import { packOffered, PACK_THIS_FOLDER } from './packFolder';
import { FORGET_FOLDER, NEW_FOLDER, parentOf, PROSPECTIVE } from './prospective';
import { TRY_AGAIN } from './Region';
import { RowMenu, type MenuItem } from './RowMenu';
import { ROWS_AHEAD, scrollAhead } from './scrollAhead';
import {
  chosen,
  everyToggled,
  kept,
  selectable,
  selectedLine,
  toggled as pickToggled,
  type Selection,
} from './selection';
import { COLOR } from './theme';
import {
  arrowed,
  contextOf,
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
 * Modified. A folder row opens and closes from the disclosure button at its
 * start; a file and an empty folder keep that room, so that every name at one
 * depth starts at one place. Every file is a row, each page of a scanned book
 * included. Each value column is as wide as the widest of its heading and the
 * values on show, measured from the rendered strings (see
 * [`columnWidth`](./columns)).
 *
 * The keys walk the rows the way a file manager's tree does (see
 * [`arrowed`](./tree)): `↑`/`↓` move the focus, `→` opens a folder and then
 * enters it, `←` closes one and then climbs to the folder above, and `Enter`
 * opens the focused row — the reader, for a file it can show or a folder
 * holding pages. The list scrolls ahead of the focus (see
 * [`scrollAhead`](./scrollAhead)). A click focuses a row, a double-click opens
 * it, and a press on the disclosure button opens or closes the folder without
 * moving the focus — except out of a folder being closed over it. On a touch
 * screen a tap opens, since a double tap is not a gesture a touch screen
 * reliably hands over.
 *
 * What the screen's own actions are about is the folder the focus is in (see
 * [`contextOf`](./tree)), and the block over the rows speaks for it: whether
 * it is on this device, whether it is being packed, "Select", "New folder…"
 * and "Pack this folder…".
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
 * The tree is also where files are added. A drop on a folder row adds to that
 * folder, on a file row to the folder it stands in, and anywhere else on the
 * list to the folder the focus is in; a folder no mapping of this device
 * reaches takes no drop, and says so while the drag is still in the air.
 * Every drop is confirmed before anything is sent, and that confirmation is
 * the screen's, after the tree has handed the files over.
 *
 * Files of the focused folder are picked out for one deletion together only
 * after "Select" is pressed: a box on every row, always, is a column of
 * controls for a gesture made now and then.
 *
 * The folders the Library does not hold yet (see [`prospective`](./prospective))
 * are rows too, marked as such, and so are the folders of books whose freeze
 * has not committed, dimmed. A prepared path's row offers "Forget this folder".
 */
export function TreeTable({
  rows,
  state,
  onChange,
  context,
  prospectiveHere,
  prepared,
  fillOf,
  freeze,
  stranded,
  strandedKnown,
  mappedAt,
  onOpenFolder,
  onOpenFile,
  onUnsupported,
  onAdd,
  onCollecting,
  onUnreadable,
  onUnmapped,
  onUnmappedDrop,
  onMap,
  onPack,
  onDelete,
  onNewFolder,
  onForget,
  onAskAgain,
  measure,
}: {
  /** Every row on show, top to bottom (see [`treeRows`](./tree)). */
  rows: readonly TreeRow[];
  /** Which row has the focus, and which folders are open. */
  state: TreeState;
  /** The focus moved, or a folder opened or closed. */
  onChange: (next: TreeState) => void;
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
   * Whether a mapping of this device reaches a folder, or `null` where that is
   * not known (see [`mappedAt`](./dropTarget)). Unknown is not refused: the
   * drop asks the folder's own listing before anything is sent.
   */
  mappedAt: (folder: string) => boolean | null;
  /** A folder row opened: the reader, at the first page of the folder. */
  onOpenFolder: (path: string) => void;
  /** A file row opened that the reader can show. */
  onOpenFile: (path: string) => void;
  /** A file row opened that the reader cannot show. */
  onUnsupported: (file: ListedFile) => void;
  /** A drop on `folder`: its files, each with its path relative to it, and what the desktop app did not take. */
  onAdd: (folder: string, drop: Drop) => void;
  /** A drop was taken and its files are being read out of it, which can take seconds. */
  onCollecting: () => void;
  /** The drop came to no files: the walk could not be finished, or the desktop app took nothing of it. */
  onUnreadable: (cause: unknown) => void;
  /**
   * A gesture made in a folder with nowhere on this device to put its files:
   * a file opened there, or a drop on the folder the focus is in. `held` is
   * whether this screen has a folder to talk about at all, and `where` is the
   * listing the gesture was made in, whose mapped folders are named as the
   * ones that would take it.
   */
  onUnmapped: (tried: Tried, held: boolean, where: Listing) => void;
  /** A drop onto a folder row no mapping of this device reaches. */
  onUnmappedDrop: (folder: string) => void;
  /** The banner's button: map a folder on this device to the top-level folder named, or the Library root. */
  onMap: (prefix: string | null) => void;
  /** "Pack this folder…" on the folder the focus is in. */
  onPack: (folder: string) => void;
  /** "Delete…" from a row's menu, or over the files picked out. */
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
}) {
  const scroller = useRef<HTMLDivElement>(null);
  const block = useRef<HTMLDivElement>(null);
  const drawn = useRef(new Map<string, HTMLTableRowElement>());
  const contextPath = contextOf(state);
  const listing = context !== null && context.path === contextPath ? context : null;

  // The files picked out for one deletion together, in the focused folder
  // (see [`selection`](./selection)), and whether the boxes are out.
  const [selection, setSelection] = useState<Selection | null>(null);
  const [picking, setPicking] = useState(false);
  const picked = picking && listing !== null ? chosen(selection, listing) : [];
  const pickable = listing === null ? [] : selectable(listing);
  useEffect(() => {
    if (listing !== null) {
      setSelection((held) => kept(held, listing));
    }
  }, [listing]);
  // Another folder leaves the mode: the boxes were put out for the files of the
  // folder they were put out in.
  useEffect(() => {
    setPicking(false);
    setSelection(null);
  }, [contextPath]);
  const leavePicking = () => {
    setPicking(false);
    setSelection(null);
  };
  // Whatever is picked goes the moment a deletion is asked for: the rows it
  // named are leaving, and a box left ticked would offer them a second time.
  const askToDelete = (target: DeleteTarget) => {
    leavePicking();
    onDelete(target);
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
  // The folder's own action, packing what it holds one file at a time into
  // Packs: not over a path the Library holds nothing at, nor over a folder it
  // does not hold yet with nothing standing in it on disk.
  const nothingToPack =
    listing !== null &&
    !listing.held &&
    listing.files.length === 0 &&
    listing.folders_on_disk.length === 0;
  const offersPack = listing !== null && packOffered(listing) && !unheld && !nothingToPack;
  const offersNewFolder = listing !== null && listing.mapped && !unheld;
  const offersSelect = pickable.length > 0 || picking;
  const emptyHere =
    listing !== null && listing.files.length === 0 && childFolders === 0;

  // What a drop comes to, read for the folder it would land in: the folder row
  // under the pointer, the folder a file row under it stands in, and the
  // focused folder anywhere else on the list.
  const drag = useDragOver();
  const [over, setOver] = useState<string | null>(null);
  useEffect(() => {
    if (!drag.dragged) {
      setOver(null);
    }
  }, [drag.dragged]);
  const target = over ?? contextPath;
  const targetMapped = (folder: string) =>
    folder === contextPath && listing !== null ? listing.mapped : mappedAt(folder);
  const outcome = dropOutcome({
    mapped: drag.dragged ? targetMapped(target) : null,
    freezing: isFreezing(freeze),
  });
  const onDrop = (event: DragEvent) => {
    event.preventDefault();
    drag.ended();
    setOver(null);
    const mapped = targetMapped(target);
    if (mapped === false) {
      // Said and not merely not done: a drop is a gesture with an outcome, and
      // a screen that goes on looking exactly as it did tells nobody that none
      // of those files were added.
      if (target === contextPath && listing !== null) {
        onUnmapped('add', !nowhere, listing);
      } else {
        onUnmappedDrop(target);
      }
      return;
    }
    // The walk is asynchronous and the event is not: what it carries is gone
    // by the first await, so the traversal is started here, and announced
    // first, because the walk itself is a wait a person is owed a word about.
    onCollecting();
    void droppedFiles(event.dataTransfer).then((drop) => onAdd(target, drop), onUnreadable);
  };

  // What opening a row does: what a double-click, a tap and `Enter` reach. A
  // folder no mapping reaches has nowhere on this device to put a file, so a
  // file there is not opened and the sentence says why instead.
  const openRow = (row: FolderRow | FileRow) => {
    if (row.kind === 'folder') {
      onOpenFolder(row.path);
      return;
    }
    if (!row.in.mapped) {
      onUnmapped('open', row.in.held || row.in.path === '', row.in);
    } else if (row.file.openable) {
      onOpenFile(row.path);
    } else {
      onUnsupported(row.file);
    }
  };
  const focused = rows.find(
    (row): row is FolderRow | FileRow => row.kind !== 'note' && row.key === state.focus,
  );

  // The keys the tree answers while it has the focus itself — which a click on
  // a row gives it — and not while a control inside it does: `Enter` on a
  // menu's button or a box is that control's.
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
    if (!ARROWS.has(event.key)) {
      return;
    }
    event.preventDefault();
    const next = arrowed(rows, state, event.key as Arrow);
    if (next !== state) {
      onChange(next);
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
    if (event.target !== event.currentTarget || byPress || picking || state.focus !== null) {
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
  const files = rows.filter((row): row is FileRow => row.kind === 'file');
  const sizes = files.map((row) => size(row.file.size));
  const times = files.map((row) => time(row.file.mtime));
  const widths = {
    size: columnWidth('Size', sizes, measured, 2 * CELL_PAD),
    modified: columnWidth('Modified', times, measured, 2 * CELL_PAD),
  };
  const columns = <Columns picking={picking} widths={widths} />;

  const menuOf = (row: FolderRow | FileRow): MenuItem[] => {
    if (row.kind === 'file') {
      return deleteOffered(row.file)
        ? [{ label: DELETE, onPress: () => askToDelete({ folder: null, paths: [row.path] }) }]
        : [];
    }
    return [
      // Where a drop into it would be taken, and nowhere else.
      ...(row.folder.mapped ? [{ label: NEW_FOLDER, onPress: () => onNewFolder(row.path) }] : []),
      // Not on a folder the Library does not hold: there is nothing of it to
      // take out.
      ...(row.prospective || row.stranded
        ? []
        : [{ label: DELETE, onPress: () => askToDelete({ folder: row.path, paths: [] }) }]),
      // Only on a path this page prepared, which is the page's to let go of;
      // an empty mapped prefix is the mapping's.
      ...(row.prospective && prepared.includes(row.path)
        ? [{ label: FORGET_FOLDER, onPress: () => onForget(row.path) }]
        : []),
    ];
  };

  return (
    <div
      ref={scroller}
      // What a row's menu measures the room below it against (see `RowMenu`).
      data-scroll-area=""
      role="tree"
      aria-label="Library"
      tabIndex={0}
      onKeyDown={onKeyDown}
      onPointerDown={() => {
        pressed.current = true;
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
      style={{
        flex: 1,
        overflow: 'auto',
        minWidth: 0,
        // The tree's own focus is drawn by the focused row, so the outline is a
        // drag's alone — around the whole list where the drop lands in the
        // focused folder, and around the folder's row where it lands there.
        outline: over === null || over === contextPath ? dropOutline(drag.dragged, outcome) : 'none',
        outlineOffset: -2,
      }}
    >
      {/* The bar and the banners stay at the top as the list scrolls,
          together: every one of them is true of the whole folder rather than
          of whichever rows happen to be in view. */}
      <div ref={block} style={{ position: 'sticky', top: 0, zIndex: 1 }}>
        {(offersPack || offersSelect || offersNewFolder) && listing !== null && (
          <div style={BAR}>
            <span style={{ flex: 1, color: COLOR.dim, fontSize: 12, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              {root ? 'Library' : contextPath}
            </span>
            {offersNewFolder && (
              <button type="button" onClick={() => onNewFolder(contextPath)} style={BAR_BUTTON}>
                {NEW_FOLDER}
              </button>
            )}
            {offersSelect && (
              <button
                type="button"
                aria-pressed={picking}
                onClick={picking ? leavePicking : () => setPicking(true)}
                style={BAR_BUTTON}
              >
                {picking ? END_SELECT : SELECT}
              </button>
            )}
            {offersPack && (
              <button type="button" onClick={() => onPack(listing.path)} style={BAR_BUTTON}>
                {PACK_THIS_FOLDER}
              </button>
            )}
          </div>
        )}
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
        {drag.dragged && drag.carriesFiles && (
          <DropBanner outcome={outcome} held={target === contextPath ? !nowhere : true} />
        )}
        {emptyHere && rows.length > 0 && (
          <Banner tone={COLOR.dim} background={COLOR.panel}>
            {emptyLine(listing, root, nowhere, prospectiveHere)}
          </Banner>
        )}
        <table style={{ ...TABLE, font: HEAD_FONT }}>
          {columns}
          <thead>
            {picked.length > 0 && listing !== null ? (
              // While files are picked out, what they come to and the one
              // deletion of them all — in the headings' own row and at its
              // height, so that no row moves under the pointer.
              <tr style={HEAD_ROW}>
                <th style={{ ...HEAD, ...PICK }}>
                  <EveryBox
                    picked={picked.length}
                    of={pickable.length}
                    onToggle={() => setSelection((held) => everyToggled(held, listing))}
                  />
                </th>
                <th colSpan={5} style={SELECTED_LINE}>
                  <span>{selectedLine(picked.length)}</span>
                  <button
                    type="button"
                    onClick={() => askToDelete({ folder: null, paths: picked })}
                    style={{ ...BAR_BUTTON, marginLeft: 10 }}
                  >
                    {DELETE}
                  </button>
                </th>
              </tr>
            ) : (
              <tr style={{ ...HEAD_ROW, color: COLOR.dim, textAlign: 'left' }}>
                {picking && (
                  <th style={{ ...HEAD, ...PICK }}>
                    {pickable.length > 0 && listing !== null && (
                      <EveryBox
                        picked={picked.length}
                        of={pickable.length}
                        onToggle={() => setSelection((held) => everyToggled(held, listing))}
                      />
                    )}
                  </th>
                )}
                <th style={HEAD}>Name</th>
                <th style={HEAD}>Size</th>
                <th style={HEAD}>
                  <span style={HIDDEN}>On this device</span>
                </th>
                <th style={HEAD}>Modified</th>
                <th style={HEAD} />
              </tr>
            )}
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
          <tbody>
            {rows.map((row) =>
              row.kind === 'note' ? (
                <tr key={row.key}>
                  {picking && <td style={CELL} />}
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
                  dropOutline={
                    row.kind === 'folder' && over === row.path && over !== contextPath
                      ? dropOutline(drag.dragged, outcome)
                      : 'none'
                  }
                  shown={row.kind === 'file' ? rowFill(row.file, row.in.path, fillOf(row.in.path)) : null}
                  pick={
                    !picking
                      ? undefined
                      : row.kind === 'file' && listing !== null && row.in.path === listing.path && deleteOffered(row.file)
                        ? {
                            picked: picked.includes(row.path),
                            onToggle: () => setSelection((held) => pickToggled(held, listing, row.path)),
                          }
                        : null
                  }
                  menu={menuOf(row)}
                  onFocus={() => onChange({ ...state, focus: row.key })}
                  onToggle={() => {
                    scroller.current?.focus();
                    onChange(toggled(state, row.path));
                  }}
                  onOpen={() => openRow(row)}
                  onDragOverRow={() => setOver(row.kind === 'folder' ? row.path : parentOf(row.path))}
                />
              ),
            )}
          </tbody>
        </table>
      )}
    </div>
  );
}

/** The keys that move the focus. */
const ARROWS = new Set(['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight']);

/** What puts the boxes out, and what takes them away again. */
export const SELECT = 'Select';
export const END_SELECT = 'Done';

/**
 * What the Library root says when it holds nothing and a mapping of this
 * device reaches it: the two ways to put the first files in, since nothing
 * else on the screen offers them.
 */
export const EMPTY_MAPPED_ROOT =
  'this Library is empty — drop files here to add them, or choose “New folder…” ' +
  'above the list to prepare where they go';

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
  dropOutline: outline,
  shown,
  pick,
  menu,
  onFocus,
  onToggle,
  onOpen,
  onDragOverRow,
}: {
  row: FolderRow | FileRow;
  rowRef: (element: HTMLTableRowElement | null) => void;
  focused: boolean;
  /** The outline a drag brings up where the drop lands in this row's folder. */
  dropOutline: string;
  /** What a file row's state is, the listing's and the fill's together; `null` for a folder. */
  shown: RowFill | null;
  /**
   * The row's box for picking it out, `null` on a row that cannot be while
   * files are being picked out, and `undefined` while they are not — when the
   * box column is not drawn at all.
   */
  pick: { picked: boolean; onToggle: () => void } | null | undefined;
  menu: readonly MenuItem[];
  onFocus: () => void;
  onToggle: () => void;
  onOpen: () => void;
  onDragOverRow: () => void;
}) {
  // How the last press on the row was made: a tap opens, a click from a mouse
  // or a pen focuses. Kept past the click, since the double-click a browser
  // makes of two taps comes after the second tap's click.
  const pointer = useRef<string | null>(null);
  const folder = row.kind === 'folder' ? row : null;
  const offered = row.kind === 'folder' || row.in.mapped;
  // While files are picked out, the highlight on a row that has a box is the
  // box's alone: the row a click focused before "Select" was pressed, drawn
  // the way a picked one is, would read as picked with its box empty. The
  // focus on such a row is drawn by a ring instead, so the keys still show
  // where they stand.
  const pickable = pick !== null && pick !== undefined;
  const selected = pickable ? pick.picked : focused;
  const ring = pickable && focused && outline === 'none' ? `1px solid ${COLOR.focusRing}` : outline;
  return (
    <tr
      ref={rowRef}
      className={offered ? 'row activatable' : 'row'}
      aria-selected={selected}
      title={row.name}
      onPointerDown={(event) => {
        pointer.current = event.pointerType;
      }}
      onClick={() => {
        // While files are picked out, a click on one that can be picked toggles
        // it, as its box does.
        if (pickable) {
          pick.onToggle();
          return;
        }
        onFocus();
        if (pointer.current === 'touch') {
          onOpen();
        }
      }}
      onDoubleClick={() => {
        if (pointer.current !== 'touch') {
          onOpen();
        }
      }}
      onDragOver={onDragOverRow}
      style={{
        background: selected ? COLOR.selected : undefined,
        outline: ring,
        outlineOffset: -2,
      }}
    >
      {pick !== undefined && (
        <td
          style={{ ...CELL, ...PICK }}
          onClick={pick !== null ? (event) => event.stopPropagation() : undefined}
        >
          {pick !== null && (
            <label style={{ display: 'block', cursor: 'pointer' }}>
              <input
                type="checkbox"
                aria-label={`select ${row.name}`}
                checked={pick.picked}
                onChange={pick.onToggle}
                style={{ margin: 0, cursor: 'pointer' }}
              />
            </label>
          )}
        </td>
      )}
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
              style={DISCLOSURE_BUTTON}
            >
              {folder.expanded ? '▾' : '▸'}
            </button>
          ) : (
            <span aria-hidden style={{ flex: '0 0 auto', width: DISCLOSURE }} />
          )}
          <span style={MARK}>{folder !== null && <FolderIcon />}</span>
          <span
            style={{
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
              color:
                folder !== null
                  ? folder.prospective || folder.stranded
                    ? COLOR.added
                    : COLOR.text
                  : row.kind === 'file' && !row.file.openable
                    ? COLOR.dim
                    : COLOR.text,
            }}
          >
            {row.name}
          </span>
          {folder !== null && folder.prospective && (
            <span title={PROSPECTIVE} style={NOTE}>
              not in Library
            </span>
          )}
          {folder !== null && !folder.folder.mapped && !folder.prospective && (
            <span style={{ ...NOTE, color: COLOR.warn }}>not here</span>
          )}
        </span>
      </td>
      <td style={{ ...CELL, textAlign: 'right', color: COLOR.dim }}>
        {row.kind === 'file' ? size(row.file.size) : ''}
      </td>
      <td style={{ ...CELL, textAlign: 'center', textOverflow: 'clip', padding: `0 ${DEVICE_PAD}px` }}>
        {folder !== null ? (
          folder.folder.present_throughout && !folder.prospective ? <Device state="present" /> : null
        ) : shown !== null ? (
          <Device state={shown.state} message={shown.message} />
        ) : null}
      </td>
      <td style={{ ...CELL, color: COLOR.dim }}>{row.kind === 'file' ? time(row.file.mtime) : ''}</td>
      <MenuCell name={row.name} items={menu} />
    </tr>
  );
}

/**
 * Whether one row is on this device, as an icon in its narrow column: nothing
 * for a row in the Library only, a laptop for one on this device, a small ring
 * while it is being fetched or uploaded, and a warning triangle where that
 * failed. The words are its label, and the sentence behind the state its
 * title.
 */
function Device({ state, message = null }: { state: RowFill['state']; message?: string | null }) {
  switch (state) {
    case 'remote':
      return null;
    case 'present':
      return <Laptop label={ON_THIS_DEVICE} />;
    case 'fetching':
      return <Ring label="Being fetched to this device" />;
    case 'added':
      return <Ring label={said('On this device, being added to the Library', message)} />;
    case 'reserved':
      return <Ring label={said('On this device, waiting to be packed into the Library', message)} />;
    case 'failed':
      return <Warning color={COLOR.refused} label={said('Fetching failed', message)} />;
    case 'declined':
      return <Warning color={COLOR.warn} label={said('Not fetched', message)} />;
  }
}

/** A state's words, and the sentence behind it after them where there is one. */
function said(state: string, message: string | null): string {
  return message === null ? state : `${state} — ${message}`;
}

/** What the laptop says: a file fetched here and one added from here alike. */
export const ON_THIS_DEVICE = 'On this device';

function Laptop({ label }: { label: string }) {
  return (
    <svg role="img" aria-label={label} width={ICON} height={ICON} viewBox="0 0 16 16" style={ICON_STYLE}>
      <title>{label}</title>
      <rect x="3" y="3.5" width="10" height="7" rx="1" fill="none" stroke={COLOR.present} strokeWidth={1.2} />
      <path d="M1.5 12.5h13" stroke={COLOR.present} strokeWidth={1.4} strokeLinecap="round" />
    </svg>
  );
}

function Ring({ label }: { label: string }) {
  return (
    <svg role="img" aria-label={label} width={ICON} height={ICON} viewBox="0 0 16 16" style={ICON_STYLE}>
      <title>{label}</title>
      <circle cx="8" cy="8" r="5.5" fill="none" stroke={COLOR.border} strokeWidth={2} />
      <path d="M8 2.5a5.5 5.5 0 0 1 5.5 5.5" fill="none" stroke={COLOR.fetching} strokeWidth={2} strokeLinecap="round">
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

/** A folder, drawn as one, in the text's own colour. */
function FolderIcon() {
  return (
    <svg role="img" aria-label="folder" width={ICON} height={ICON} viewBox="0 0 16 16" style={ICON_STYLE}>
      <path
        d="M1.5 3.5h4.2l1.5 1.5h7.3v7.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.2}
        strokeLinejoin="round"
      />
    </svg>
  );
}

/**
 * The cell a row's menu stands in. A click anywhere in it stops there: the row
 * under it focuses and opens, and a press meant for the menu must not also
 * reach the row.
 */
function MenuCell({ name, items }: { name: string; items: readonly MenuItem[] }) {
  return (
    <td
      style={{ ...CELL, overflow: 'visible', textAlign: 'center', padding: '0 4px' }}
      onClick={items.length > 0 ? (event) => event.stopPropagation() : undefined}
      onDoubleClick={items.length > 0 ? (event) => event.stopPropagation() : undefined}
    >
      <RowMenu name={name} items={items} />
    </td>
  );
}

/** Said while the book in the focused folder is being packed. */
function Packing() {
  return (
    <Banner tone={COLOR.added} background={COLOR.addedGround}>
      packing this folder into the Library — the pages go up together, as Packs,
      and become ordinary rows when the batch commits
    </Banner>
  );
}

/**
 * Said over the rows where the focused folder is not on this device, with the
 * button that maps the top-level folder it is under — or the Library root —
 * since a mapping is keyed by one top-level component (spec: EP-9).
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
  const button = (prefix: string | null, label: string) => (
    <>
      {' '}
      <button onClick={() => onMap(prefix)} style={MAP_BUTTON}>
        {label}
      </button>
    </>
  );
  return (
    <Banner tone={COLOR.warn} background={COLOR.warnGround}>
      {root && !files && !folders ? (
        <>
          this Library holds nothing yet, and nothing on this device is mapped to its
          root — map a folder here to start putting files in it
          {button(null, MAP_THE_ROOT)}
        </>
      ) : root ? (
        <>
          the Library root is not mapped on this device — files sitting directly in it
          cannot be fetched, though a folder below can be mapped on its own
          {button(null, MAP_THE_ROOT)}
        </>
      ) : (
        <>
          this folder is not on this device — map <code>{top}</code> to a folder here{' '}
          {files
            ? 'to fetch its files'
            : folders
              ? 'to fetch what is in the folders below it'
              : 'before putting anything in it'}
          {button(top, mapLabel(path, top))}
        </>
      )}
    </Banner>
  );
}

/** The header's box: ticked where every file is, part-ticked where some are. */
function EveryBox({ picked, of, onToggle }: { picked: number; of: number; onToggle: () => void }) {
  const box = useRef<HTMLInputElement>(null);
  const some = picked > 0 && picked < of;
  useEffect(() => {
    if (box.current !== null) {
      box.current.indeterminate = some;
    }
  }, [some]);
  return (
    <input
      ref={box}
      type="checkbox"
      aria-label="select every file"
      checked={picked === of}
      onChange={onToggle}
      style={{ margin: 0, cursor: 'pointer' }}
    />
  );
}

/**
 * The widths of the columns, given to both tables so the two line up: the box
 * (only while files are picked out), the name (the rest of the width), the
 * size, whether the row is on this device, the time, and the menu.
 */
function Columns({ picking, widths }: { picking: boolean; widths: { size: number; modified: number } }) {
  return (
    <colgroup>
      {picking && <col style={{ width: 32 }} />}
      <col />
      <col style={{ width: widths.size }} />
      <col style={{ width: ICON + 2 * DEVICE_PAD }} />
      <col style={{ width: widths.modified }} />
      <col style={{ width: 36 }} />
    </colgroup>
  );
}

/** The fonts the headings and the rows are drawn in, which their widths are measured in. */
const HEAD_FONT = '12px system-ui, sans-serif';
const ROW_FONT = '14px system-ui, sans-serif';

/** The padding either side of a value. */
const CELL_PAD = 10;

/** How wide an icon is drawn, and the room either side of one in its column. */
const ICON = 16;
const DEVICE_PAD = 6;

/** The room a disclosure button takes, kept on a row without one. */
const DISCLOSURE = 18;

/** How far in a row at `depth` starts. */
function indent(depth: number): number {
  return 6 + depth * 16;
}

const TABLE: CSSProperties = { width: '100%', borderCollapse: 'collapse', tableLayout: 'fixed' };

const HEAD: CSSProperties = {
  padding: `6px ${CELL_PAD}px`,
  borderBottom: `1px solid ${COLOR.border}`,
  fontWeight: 'normal',
  whiteSpace: 'nowrap',
  overflow: 'hidden',
};

const CELL: CSSProperties = {
  padding: `4px ${CELL_PAD}px`,
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
  border: 'none',
  background: 'none',
  color: COLOR.dim,
  cursor: 'pointer',
  font: 'inherit',
  padding: 0,
};

/** The room the folder icon takes, kept on a file's row so the names line up. */
const MARK: CSSProperties = { flex: '0 0 auto', width: ICON, marginRight: 6, color: COLOR.dim };

/** What a row says after its name. */
const NOTE: CSSProperties = { marginLeft: 10, color: COLOR.added, fontSize: 11, flex: '0 0 auto' };

/** The column the boxes stand in. */
const PICK: CSSProperties = { width: 32, textAlign: 'center' };

/** The headings' row, at one height whether it holds the headings or a selection's line. */
const HEAD_ROW: CSSProperties = { height: 30, fontSize: 12, background: COLOR.ground };

const SELECTED_LINE: CSSProperties = {
  padding: `0 ${CELL_PAD}px`,
  background: COLOR.panel,
  borderBottom: `1px solid ${COLOR.border}`,
  color: COLOR.text,
  fontSize: 12,
  fontWeight: 'normal',
  textAlign: 'left',
  whiteSpace: 'nowrap',
};

const BAR: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: 8,
  padding: '4px 12px',
  background: COLOR.panel,
  borderBottom: `1px solid ${COLOR.border}`,
};

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
