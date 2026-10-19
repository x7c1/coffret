import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode,
} from 'react';

import type {
  Added,
  DeleteTarget,
  DisplacedFill,
  Fill,
  Freeze,
  ListedFile,
  Listing,
} from '@coffret/api';

import { Banner, DropBanner, dropOutline } from './Banner';
import { deleteOffered, DELETE } from './deleteEntries';
import { useDragOver } from './dragOver';
import { droppedFiles } from './drop';
import { dropOutcome } from './dropTarget';
import { freezingHere, isFreezing, rowFill, SAYS, type RowState } from './fill';
import { size, time } from './humanize';
import { mapLabel, MAP_THE_ROOT } from './mapping';
import { packOffered, PACK_THIS_FOLDER } from './packFolder';
import { RowMenu, type MenuItem } from './RowMenu';
import {
  chosen,
  everyToggled,
  kept,
  selectable,
  selectedLine,
  toggled,
  type Selection,
} from './selection';
import { COLOR } from './theme';
import { NOTHING_AT_THIS_PATH, type Tried } from './unmapped';

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
 * it over the top: `present`, `remote` and `added` are the listing's to say
 * and nothing here overrides them, while `fetching`, `failed` and `declined` are
 * the fill's — work in flight, which the listing has no word for.
 *
 * The list is also where files are added. Dropping them on it adds them to the
 * folder it is showing, which is the whole of the gesture: there is no upload
 * button and no dialog, because what a person means by dragging files onto a
 * folder is not in doubt. A folder no mapping of this device reaches takes no
 * drop — its rows open nothing, and where a banner stands over them it says why
 * — and it says so while the drag is still in the air rather than only by not
 * reacting to it: the outline a drag brings up is the refused colour there, and
 * letting go says in words that nothing was added, naming the folders below that
 * would have taken it. That is the only thing that turns a drop away.
 *
 * Every gesture such a folder refuses is answered that way, opening a row
 * included. A row there is not offered to the pointer and does not open, and it
 * still says why when it is opened: the alternative is a person clicking a
 * name over and over at a screen that never once reacts.
 *
 * A click on a row selects it and a double-click opens it, as a file manager's
 * rows do, so that a row can be pointed at without walking into it: a folder
 * navigates and a file opens the reader. `Enter` opens the selected row and the
 * arrow keys move the selection. On a touch screen a tap opens, since a double
 * tap is not a gesture a touch screen reliably hands over. Selecting changes
 * nothing else on the screen.
 *
 * Files are picked out for one deletion together only after "Select" is
 * pressed: a box on every row, always, is a column of controls for a gesture
 * made now and then. Leaving that mode lets what was picked go.
 *
 * Every drop is confirmed before anything is sent — where it lands, and for a
 * drop holding a folder whether it goes as a Pack or its files one by one —
 * and that confirmation is the screen's, after this list has handed the files
 * over. A row of the folder tree takes a drop the same way, for its own folder.
 */
export function FileList({
  listing,
  fill,
  freeze,
  stranded,
  strandedKnown,
  selected,
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
}: {
  listing: Listing;
  /**
   * The fill the rows are to read: what the server is bringing over, wherever
   * it is bringing it, or a stopped one a later run took the record from.
   */
  fill: Fill | DisplacedFill | null;
  /** What the server is packing, wherever it is packing it. */
  freeze: Freeze | null;
  /**
   * Whether this is the folder of a book whose freeze has not committed, which
   * the Library does not hold yet (see [`strandedFolders`](./stranded)). It
   * decides nothing about how a drop is added — only that a path the Library
   * holds nothing at is not said to be nothing.
   */
  stranded: boolean;
  /**
   * Whether the stranded folders are known yet.
   *
   * `stranded` is read off them, and they are not on hand when this screen
   * comes up: they are read against the folder tree's answer, which is a
   * request of its own beside this listing's. Until that lands, a `stranded`
   * of false means "not known to be one" rather than "not one" — and `false`
   * here says so, so that nothing on this screen states as a fact what only
   * that answer decides.
   */
  strandedKnown: boolean;
  /**
   * The Entry Path the reader was last opened at here, if any, which is the
   * row selected when the reader closes over the list.
   */
  selected: string | null;
  onOpenFolder: (path: string) => void;
  onOpenFile: (path: string) => void;
  onUnsupported: (file: ListedFile) => void;
  /** Files dropped on this folder, each with its path relative to it. */
  onAdd: (files: Added[]) => void;
  /**
   * A drop was taken and its files are being read out of it.
   *
   * Said before {@link onAdd} rather than left to it, because what stands
   * between the two is a walk: a browser hands a dropped folder over as
   * something to traverse, one batch of children at a time, and a nested folder
   * of several hundred pages is seconds of that before there is anything to
   * send. Nothing on the screen changes in the meantime — a nested drop adds no
   * row to the folder being looked at, since its files land one folder down —
   * so without this the gesture is answered by nothing at all until the upload
   * begins.
   */
  onCollecting: () => void;
  /**
   * The walk could not be finished, and this is what stopped it.
   *
   * A browser reads a dropped folder's children in batches and may refuse one,
   * which ends the walk with no files and no answer. Said rather than swallowed:
   * the line {@link onCollecting} put up would otherwise stand saying the drop
   * was being read until the tab was closed.
   */
  onUnreadable: (cause: unknown) => void;
  /**
   * A gesture made in a folder with nowhere on this device to put its files,
   * and which gesture it was.
   *
   * Both of the two this list offers reach it. A drop is refused because there
   * is nowhere to put a single one of the files; a row is not opened because
   * the fetch behind it would be declined for the same reason. Neither changes
   * anything on the screen, so neither is answered by the screen — and what the
   * banner above the rows says is why the folder is like this, which is not the
   * same as saying what became of the thing that was just tried.
   *
   * `held` is whether this screen has a folder to talk about at all, and it is
   * the list's to decide rather than the listing's field read again: one path
   * gets one explanation, so the answer here says the Library holds nothing at
   * this path exactly where the line under the rows does and the banner over
   * them keeps quiet. Everywhere else the folder is there and the mapping is
   * what is missing, which is the reason both the banner and this give.
   */
  onUnmapped: (tried: Tried, held: boolean) => void;
  /**
   * The banner's button was pressed: map a folder on this device to the
   * top-level folder named, or to the Library root where it is `null`.
   */
  onMap: (prefix: string | null) => void;
  /**
   * "Pack this folder…" was pressed on the folder this list is showing: ask
   * what a freeze of it would pack, and arm one if the person says so.
   */
  onPack: (folder: string) => void;
  /**
   * "Delete…" was chosen from a row's menu, or pressed over the files picked
   * out: ask what deleting that folder or those files would take out of the
   * Library, and arm the deletion if the person says so.
   *
   * Offered on every folder row and on every file the Library holds, mapped
   * here or not — a deletion changes the Library and touches no file on this
   * device. Not on a file only added here: the Library has nothing of it to
   * take out, so it has no menu item and no box to pick it with.
   */
  onDelete: (target: DeleteTarget) => void;
}) {
  // Whether files are being dragged over the list right now (see
  // [`useDragOver`](./dragOver)).
  const drag = useDragOver();
  const dragged = drag.dragged;
  // The files picked out for one deletion together (see
  // [`selection`](./selection)). Held here rather than by the screen: nothing
  // outside the list needs to know what is picked until a deletion names it.
  const [selection, setSelection] = useState<Selection | null>(null);
  // Whether the boxes are out: picking files for one deletion is a mode a
  // person enters with "Select", and leaving it lets what was picked go.
  const [picking, setPicking] = useState(false);
  const picked = picking ? chosen(selection, listing) : [];
  const pickable = selectable(listing);
  // A listing of another folder, or the same one asked again without some of
  // the files, leaves the selection holding only what is still offered. Read
  // through `chosen` above in the meantime, so no render shows a stale count.
  useEffect(() => {
    setSelection((held) => kept(held, listing));
  }, [listing]);
  // And another folder leaves the mode as well: the boxes were put out for the
  // files of the folder they were put out in.
  useEffect(() => {
    setPicking(false);
    setSelection(null);
  }, [listing.path]);
  const leavePicking = () => {
    setPicking(false);
    setSelection(null);
  };
  // Whatever is picked goes the moment a deletion is asked for, from the
  // selection or from a row: the rows it named are leaving, and a box left
  // ticked over them would offer to delete them a second time.
  const askToDelete = (target: DeleteTarget) => {
    leavePicking();
    onDelete(target);
  };
  // The row selected by a click, or by the arrow keys, which `Enter` opens:
  // one at a time, and nothing but a highlight. It follows the reader, so that
  // the row a reader closed over is the selected one, and starts over in
  // another folder.
  const [cursor, setCursor] = useState<string | null>(
    selected === null ? null : fileKey(selected),
  );
  useEffect(() => {
    setCursor(selected === null ? null : fileKey(selected));
  }, [listing.path, selected]);
  const root = listing.path === '';
  const empty = listing.folders.length === 0 && listing.files.length === 0;
  // A path the Library names nothing at, as far as what is on hand goes. A
  // folder of the Library is what the separators under it imply, so `held` being
  // false means nothing is under this one — and the folder of a book whose
  // freeze has not committed is the one place that is a state rather than a
  // mistake, since its pages are on the disk and the Library has not heard of
  // it yet.
  const unheld = !listing.held && !stranded;
  // And the same thing said out loud, which waits for the one answer that can
  // still overturn it. A stranded folder is read off the folder tree, a separate
  // request from this listing, so until it lands `stranded` is false even for a
  // folder whose book is being packed this minute: somebody who closed the tab
  // mid-packing and reopened that folder would read that the Library has
  // nothing of theirs there. Said once the folders are known, and not at all
  // where that request failed — the screen is already showing the tree's own
  // trouble, and a fact this page could not confirm is not one to assert.
  const nowhere = unheld && strandedKnown;
  // A Library root with no mapping of its own and no files sitting in it is the
  // ordinary shape of a device that mapped one top-level folder, and there is
  // nothing on this screen for a banner to explain: no row here is inert,
  // because the rows are folders and the folders say for themselves. Anywhere
  // else — and at a root that does hold files — unmapped is worth saying.
  //
  // Except where there is no such folder. A mistyped path is unmapped as often
  // as not, and being offered to map it would have somebody choose a folder on
  // this device for a part of the Library that does not exist. Kept back on what is
  // on hand rather than on the final answer, since the same objection holds
  // while the tree is still out: neither sentence about a path is worth saying
  // early, and this is the one that would send somebody somewhere.
  const sayUnmapped =
    !listing.mapped && !unheld && !(root && listing.files.length === 0);
  // What is happening to this folder, said over the rows because it is true of
  // every one of them: the pages are going up together, as Packs, and until the
  // batch commits none of them is an Entry.
  const packing = freezingHere(freeze, listing.path);
  // What a drop here would come to, which is the question a drag wants
  // answered while the files are still in the air; `dropOutcome` says why each
  // folder comes to what it does.
  const outcome = dropOutcome({
    mapped: listing.mapped,
    freezing: isFreezing(freeze),
  });
  // The folder's own action: packing what it holds one file at a time into
  // Packs. Offered on a mapped folder that is not the Library root (see
  // [`packOffered`](./packFolder)), and not over a path the Library holds
  // nothing at, where there is no folder to pack.
  const offersPack = packOffered(listing) && !unheld;
  // "Select" is offered where there is a file to pick; the mode is left from
  // the same place.
  const offersSelect = pickable.length > 0 || picking;
  // What opening a file's row does: what a double-click, a tap and `Enter`
  // reach.
  //
  // A folder no mapping reaches has nowhere on this device to put a file, so
  // every fetch under it would be declined: its rows are shown and not offered,
  // rather than letting a reader walk into the refusal. What opening one gets
  // instead is the sentence, the way the drop above does — the row stays
  // un-openable and the attempt stops being met by nothing at all, which is the
  // one answer a person cannot tell from a screen that is broken.
  const openFile = (file: ListedFile) =>
    !listing.mapped
      ? () => onUnmapped('open', !nowhere)
      : file.openable
        ? () => onOpenFile(file.path)
        : () => onUnsupported(file);
  // Every row in the order it is drawn, which is what the arrow keys walk.
  const rows: ListRow[] = [
    ...listing.folders.map((folder) => ({
      key: folderKey(folder.path),
      open: () => onOpenFolder(folder.path),
    })),
    ...listing.files.map((file) => ({ key: fileKey(file.path), open: openFile(file) })),
  ];
  // The keys the list answers while it has the focus itself — which a click on
  // a row gives it — and not while a control inside it does: `Enter` on a
  // menu's button or a box is that control's.
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget || rows.length === 0) {
      return;
    }
    if (event.key === 'Enter') {
      const open = rows.find((row) => row.key === cursor)?.open;
      if (open !== undefined) {
        event.preventDefault();
        open();
      }
      return;
    }
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') {
      return;
    }
    event.preventDefault();
    const at = rows.findIndex((row) => row.key === cursor);
    const next =
      at === -1
        ? event.key === 'ArrowDown'
          ? 0
          : rows.length - 1
        : Math.min(rows.length - 1, Math.max(0, at + (event.key === 'ArrowDown' ? 1 : -1)));
    setCursor(rows[next].key);
  };
  return (
    <div
      // What a row's menu measures the room below it against (see `RowMenu`).
      data-scroll-area=""
      // Focusable, so that the keys that move and open the selection reach it:
      // a click on a row focuses the list.
      tabIndex={0}
      onKeyDown={onKeyDown}
      style={{
        flex: 1,
        overflow: 'auto',
        minWidth: 0,
        // The list's own focus is drawn by the selected row rather than by a
        // ring around the whole of it, so the outline is a drag's alone.
        outline: dropOutline(dragged, outcome),
        // Inside the outline rather than around it, so that the outline moves
        // nothing on the screen when a drag arrives: one that took up room
        // would shift the list itself, edges and all. The drop line does move
        // the rows down, for the reason given at the sticky block below.
        //
        // A folder that would not take this drop is outlined too, and in the
        // refused colour. The answer to "will this take my files" is worth
        // having while the files are still in the air, and a list that simply
        // did not light up would leave a person to find out by letting go.
        outlineOffset: -2,
      }}
      // The three that have to be answered for a drop to happen at all (see
      // [`useDragOver`](./dragOver)).
      onDragEnter={drag.onDragEnter}
      onDragOver={drag.onDragOver}
      onDragLeave={drag.onDragLeave}
      onDrop={(event) => {
        event.preventDefault();
        drag.ended();
        if (!listing.mapped) {
          // Said and not merely not done. A drop is a gesture with an outcome,
          // and the outcome here is that none of those files were added — which
          // a screen that goes on looking exactly as it did does not tell
          // anybody. The banner over the rows is the standing reason; this is
          // the answer to the thing that was just tried.
          onUnmapped('add', !nowhere);
          return;
        }
        // The walk is asynchronous and the event is not: what it carries is
        // gone by the first await, so the traversal is started here and the
        // answer is handed over whole. It is announced first, because the walk
        // itself is a wait a person is owed a word about.
        onCollecting();
        void droppedFiles(event.dataTransfer).then(onAdd, onUnreadable);
      }}
    >
      {/* The banners stay at the top of the list as it scrolls, together:
          every one of them is true of the whole folder rather than of whichever
          rows happen to be in view, and a reason that scrolled away would leave
          a screenful of names with nothing to explain them. One sticky block
          rather than each banner sticky on its own, since two of them can
          stand at once — the drop line under the unmapped one — and two sticky
          to the same edge would be drawn one over the other. */}
      <div style={{ position: 'sticky', top: 0, zIndex: 1 }}>
        {(offersPack || offersSelect) && (
          <div
            style={{
              display: 'flex',
              justifyContent: 'flex-end',
              gap: 8,
              padding: '4px 12px',
              background: COLOR.panel,
              borderBottom: `1px solid ${COLOR.border}`,
            }}
          >
            {offersSelect && (
              <button
                type="button"
                aria-pressed={picking}
                // Entering the mode lets the row a click chose go: a highlight
                // is what a picked row is drawn with, and one left over a row
                // whose box is empty would read as picked.
                onClick={
                  picking
                    ? leavePicking
                    : () => {
                        setPicking(true);
                        setCursor(null);
                      }
                }
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
        {sayUnmapped && (
          <Unmapped
            root={root}
            path={listing.path}
            files={listing.files.length > 0}
            folders={listing.folders.length > 0}
            onMap={onMap}
          />
        )}
        {packing && <Packing />}
        {/* While a drag is over the list, the line saying what letting go
            will do. It takes its own room in the block rather than being laid
            over what is under it: laid over, it covered the top of whichever
            row was first in view once the list had scrolled. The rows move
            down by its height as a drag arrives, which costs nothing, since
            what takes the drop is the whole list and not the row under the
            pointer. */}
        {dragged && drag.carriesFiles && <DropBanner outcome={outcome} held={!nowhere} />}
        {/* The column headings stay with the banners, so that the line a
            selection is said on can stand in for them without covering a row:
            the headings are the one thing on the list it may hide. */}
        {!empty && (
          <table style={TABLE}>
            <Columns picking={picking} />
            <thead>
              {picked.length > 0 ? (
                // While files are picked out, what they come to and the one
                // deletion of them all — in the headings' own row and at its
                // height, so that no row moves under the pointer that ticked
                // the first box and is about to tick the next. The box column
                // is left as it was, so the header's box stays where it was to
                // pick the rest or clear them.
                <tr style={HEAD_ROW}>
                  <th style={{ ...HEAD, ...PICK }}>
                    <EveryBox
                      picked={picked.length}
                      of={pickable.length}
                      onToggle={() => setSelection((held) => everyToggled(held, listing))}
                    />
                  </th>
                  <th colSpan={6} style={SELECTED_LINE}>
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
                  {/* The box column is there only while files are being
                      picked out, and so is the box that picks them all. */}
                  {picking && (
                    <th style={{ ...HEAD, ...PICK }}>
                      {pickable.length > 0 && (
                        <EveryBox
                          picked={picked.length}
                          of={pickable.length}
                          onToggle={() => setSelection((held) => everyToggled(held, listing))}
                        />
                      )}
                    </th>
                  )}
                  <th style={HEAD} />
                  <th style={HEAD}>Name</th>
                  <th style={{ ...HEAD, textAlign: 'right' }}>Size</th>
                  <th style={HEAD}>Modified</th>
                  <th style={HEAD}>State</th>
                  <th style={HEAD} />
                </tr>
              )}
            </thead>
          </table>
        )}
      </div>
      {empty ? (
        <p style={{ padding: 16, color: COLOR.dim }}>
          {/* Three states and not two. A folder of the Library holds
              something by definition, so "this folder is empty" over a path
              the Library has never held is the screen inventing a folder to
              describe — which is what somebody who mistyped a component into
              the address bar, or followed a link written before the Entries
              went, would read it as. The listing says which of the two it
              answered and this says it back — once the stranded folders are
              known, since a folder whose book is still being packed is the
              one path the listing alone would have it wrong about, and where
              it is still out this falls back to the milder of the two. */}
          {nowhere
            ? NOTHING_AT_THIS_PATH
            : root
              ? 'this Library is empty'
              : 'this folder is empty'}
        </p>
      ) : (
        <table style={TABLE}>
          <Columns picking={picking} />
          <tbody>
            {listing.folders.map((folder) => {
              const key = folderKey(folder.path);
              return (
                <Row
                  key={key}
                  icon={<FolderIcon />}
                  name={folder.name}
                  selected={cursor === key}
                  onSelect={() => setCursor(key)}
                  onOpen={() => onOpenFolder(folder.path)}
                  // No box: the server takes one folder per deletion, and a
                  // folder is deleted from its own row's menu.
                  pick={picking ? null : undefined}
                >
                  <td style={CELL} />
                  <td style={CELL} />
                  <td style={CELL}>
                    {!folder.mapped && <Chip color={COLOR.warn}>not here</Chip>}
                  </td>
                  <MenuCell
                    name={folder.name}
                    items={[
                      {
                        label: DELETE,
                        onPress: () => askToDelete({ folder: folder.path, paths: [] }),
                      },
                    ]}
                  />
                </Row>
              );
            })}
            {listing.files.map((file) => {
              const key = fileKey(file.path);
              const pick = !picking
                ? undefined
                : deleteOffered(file)
                  ? {
                      picked: picked.includes(file.path),
                      onToggle: () => setSelection((held) => toggled(held, listing, file.path)),
                    }
                  : null;
              return (
                <Row
                  key={key}
                  // No mark: a file is told from a folder by the folder's, and
                  // one the reader cannot draw by its dimmed name.
                  icon={null}
                  name={file.name}
                  dim={!file.openable}
                  selected={cursor === key || (pick?.picked ?? false)}
                  onSelect={() => setCursor(key)}
                  onOpen={openFile(file)}
                  offered={listing.mapped}
                  pick={pick}
                >
                  <td style={{ ...CELL, textAlign: 'right', color: COLOR.dim }}>
                    {size(file.size)}
                  </td>
                  <td style={{ ...CELL, color: COLOR.dim }}>{time(file.mtime)}</td>
                  <td style={CELL}>
                    <StateChip file={file} folder={listing.path} fill={fill} />
                  </td>
                  <MenuCell
                    name={file.name}
                    items={
                      deleteOffered(file)
                        ? [
                            {
                              label: DELETE,
                              onPress: () => askToDelete({ folder: null, paths: [file.path] }),
                            },
                          ]
                        : []
                    }
                  />
                </Row>
              );
            })}
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
  // In the folder and not in the Library, which is a state to notice rather
  // than one to worry about: the next run is what ends it.
  added: COLOR.added,
  // Work still to come rather than work refused or done: the chip a row being
  // brought over wears, since this one is about to be carried in too.
  reserved: COLOR.fetching,
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
  borderBottom: `1px solid ${COLOR.rowRule}`,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
};

/** What a list row is called by while it is selected, a folder's told from a file's. */
function folderKey(path: string): string {
  return `folder:${path}`;
}

function fileKey(path: string): string {
  return `file:${path}`;
}

/** One row of the list as the keys see it: which it is, and what opening it does. */
interface ListRow {
  key: string;
  open: () => void;
}

/** What puts the boxes out, and what takes them away again. */
export const SELECT = 'Select';
export const END_SELECT = 'Done';

function Row({
  icon,
  name,
  dim,
  selected,
  onSelect,
  onOpen,
  offered = true,
  pick,
  children,
}: {
  /** The mark beside the name, and `null` for a row that has none. */
  icon: ReactNode;
  name: string;
  dim?: boolean;
  /** Whether the row is drawn selected: the one a click chose, or one picked out. */
  selected: boolean;
  /** A click on the row, which selects it and opens nothing. */
  onSelect: () => void;
  /** A double-click, a tap, or `Enter` on the row while it is selected. */
  onOpen: () => void;
  /**
   * Whether this row is offered to the pointer, which is what the cursor and the
   * hover say.
   *
   * Apart from whether it opens, because one row answers an open and is not
   * offered: a file in a folder no mapping of this device reaches answers with
   * the reason it will not open, and a row that invited the gesture first would
   * make that sentence the second surprise rather than the first. A row whose
   * format the reader cannot draw is offered, and remains so — the sentence it
   * answers with is about the one file, in a folder where every other row does
   * open.
   */
  offered?: boolean;
  /**
   * The row's box for picking it out, `null` on a row that cannot be while
   * files are being picked out, and `undefined` while they are not — when the
   * box column is not drawn at all.
   */
  pick: { picked: boolean; onToggle: () => void } | null | undefined;
  children: ReactNode;
}) {
  // The selected row is brought into view: the one the reader was last opened
  // at when the reader closes over it — a folder is not scrolled by turning
  // pages, and one restored from a URL was never scrolled at all — and the one
  // the arrow keys moved to.
  const here = useRef<HTMLTableRowElement>(null);
  useEffect(() => {
    if (selected) {
      here.current?.scrollIntoView({ block: 'nearest' });
    }
  }, [selected]);
  // How the last press on the row was made. A tap opens, since a double tap is
  // not a gesture a touch screen reliably hands over; a click from a mouse or a
  // pen selects. Kept past the click rather than cleared by it: the
  // double-click a browser makes of two taps comes after the second tap's
  // click, and is told for a tap's by it.
  const pointer = useRef<string | null>(null);

  return (
    <tr
      ref={here}
      className={offered ? 'row activatable' : 'row'}
      aria-selected={selected}
      onPointerDown={(event) => {
        pointer.current = event.pointerType;
      }}
      onClick={() => {
        // While files are picked out, a click on one that can be picked toggles
        // it, as its box does: the rows are what a person points at. It does
        // not also select the row, whose highlight would then stay on after a
        // second click took the pick off again.
        if (pick !== null && pick !== undefined) {
          pick.onToggle();
          return;
        }
        onSelect();
        if (pointer.current === 'touch') {
          onOpen();
        }
      }}
      onDoubleClick={() => {
        // A double tap, where a browser hands one over, has been opened by its
        // first tap already.
        if (pointer.current !== 'touch') {
          onOpen();
        }
      }}
      title={name}
      style={selected ? { background: COLOR.selected } : undefined}
    >
      {/* The whole cell is the box's, and a click anywhere in it stops there:
          the row under it would toggle the box a second time. A row with no
          box keeps the cell as part of the row. */}
      {pick !== undefined && (
        <td
          style={{ ...CELL, ...PICK }}
          onClick={pick !== null ? (event) => event.stopPropagation() : undefined}
        >
          {pick !== null && (
            <label style={{ display: 'block', cursor: 'pointer' }}>
              <input
                type="checkbox"
                aria-label={`select ${name}`}
                checked={pick.picked}
                onChange={pick.onToggle}
                style={{ margin: 0, cursor: 'pointer' }}
              />
            </label>
          )}
        </td>
      )}
      <td style={{ ...CELL, ...MARK }}>{icon}</td>
      <td style={{ ...CELL, color: dim === true ? COLOR.dim : COLOR.text }}>{name}</td>
      {children}
    </tr>
  );
}

/**
 * A folder, drawn as one: the mark on a folder's row, which navigates into it
 * rather than opening anything out beneath it, so it is drawn as the thing and
 * not as an arrow. In the text's own colour, whatever the theme draws that in.
 */
function FolderIcon() {
  return (
    <svg
      role="img"
      aria-label="folder"
      width={ICON}
      height={ICON}
      viewBox="0 0 16 16"
      style={{ display: 'block', margin: '0 auto' }}
    >
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
 * under it selects and opens, and a press meant for the menu must not also
 * reach the row. A row with nothing to offer keeps the cell as part of the row.
 */
function MenuCell({ name, items }: { name: string; items: readonly MenuItem[] }) {
  return (
    <td
      // Not clipped like the other cells, since the menu hangs out of it.
      style={{ ...CELL, overflow: 'visible', textAlign: 'center' }}
      onClick={items.length > 0 ? (event) => event.stopPropagation() : undefined}
      onDoubleClick={items.length > 0 ? (event) => event.stopPropagation() : undefined}
    >
      <RowMenu name={name} items={items} />
    </td>
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
  fill: Fill | DisplacedFill | null;
}) {
  const shown = rowFill(file, folder, fill);
  return (
    <Chip color={CHIP[shown.state]} title={shown.message}>
      {SAYS[shown.state]}
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
 * Said while the book in this folder is being packed.
 *
 * The rows say "not in Library" throughout, which is true and is not the whole
 * answer: a freeze builds and commits one batch, so the pages become Entries
 * together or not at all, and a person watching row after row stay the same
 * would have nothing to tell them it was working.
 */
function Packing() {
  return (
    <Banner tone={COLOR.added} background={COLOR.addedGround}>
      packing this folder into the Library — the pages go up together, as Packs,
      and become ordinary rows when the batch commits
    </Banner>
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
 * And what the folder holds decides the rest of it, which is the same treatment
 * widened. The root was told apart because a sentence about its files is a
 * sentence about nothing where it has none; a folder of nothing but subfolders
 * is in exactly that position, and "to fetch its files" over rows that are all
 * folders reads as a reference to something that is not on the screen. So the
 * clause that names what mapping would be for names what is actually here: the
 * files in this folder, or the files in the folders below it — or, where there
 * is neither, nothing fetched at all. A stranded folder inside an unmapped
 * subtree that has come to be empty still needs the banner, because a drop into
 * it is the gesture that would be refused; over rows that are not there, both
 * of the other clauses would point at something to fetch and the line under
 * them would say the folder is empty.
 *
 * What it offers to map is the top-level folder and not this one. A mapping is
 * keyed by one top-level component of the Library (spec: EP-9), so a mapping
 * of `books/vol-1` is refused as a subtree no mapping can stand for; it is
 * `books` that has to be given a folder on this device, and the button says so
 * wherever it is not the folder on the screen.
 *
 * The button opens the picker that chooses that folder (see
 * [`mapping`](./mapping)), and a mapping recorded there reloads this listing,
 * which is what takes the banner away.
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
      {root ? (
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

/**
 * The header's box: ticked where every file that can be picked is, part-ticked
 * where some are, and pressing it picks them all or clears them.
 */
function EveryBox({ picked, of, onToggle }: { picked: number; of: number; onToggle: () => void }) {
  const box = useRef<HTMLInputElement>(null);
  const some = picked > 0 && picked < of;
  // Only settable from script: there is no attribute for it.
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
 * The rows' table, and the headings' one over it in the sticky block: the same
 * layout and the same {@link Columns}, so that the two line up column for
 * column although they are two tables.
 */
const TABLE: CSSProperties = { width: '100%', borderCollapse: 'collapse', tableLayout: 'fixed' };

/**
 * The widths of the columns, given to both tables: the box (only while files
 * are being picked out), the mark, the name (the rest of the width), the size,
 * the time, the state, and the menu.
 */
function Columns({ picking }: { picking: boolean }) {
  return (
    <colgroup>
      {picking && <col style={{ width: 32 }} />}
      <col style={{ width: MARK_WIDTH }} />
      <col />
      <col style={{ width: 100 }} />
      <col style={{ width: 210 }} />
      <col style={{ width: 90 }} />
      <col style={{ width: 40 }} />
    </colgroup>
  );
}

/** How wide the mark is drawn, and the room its column leaves either side of it. */
const ICON = 16;
const MARK_PAD = 8;
const MARK_WIDTH = ICON + 2 * MARK_PAD;

/**
 * The column the marks stand in: as wide as the folder's icon and its padding,
 * and never cut short — a mark clipped to an ellipsis is a mark that says
 * nothing.
 */
const MARK: CSSProperties = {
  padding: `5px ${MARK_PAD}px`,
  color: COLOR.dim,
  textAlign: 'center',
  textOverflow: 'clip',
};

/** The column the boxes stand in. */
const PICK: CSSProperties = { width: 32, textAlign: 'center' };

/**
 * The headings' row, at one height whether it holds the headings or the line a
 * selection is said on, so that nothing below it moves as the first box is
 * ticked.
 *
 * The height is only the least the row may be: a cell whose line is taller
 * grows it. The font size is given here for both rows, not to the headings
 * alone, for that reason: at the page's own size the box's cell in the
 * selection's row drew a taller line and pushed every row down at the first
 * tick.
 */
const HEAD_ROW: CSSProperties = { height: 30, fontSize: 12, background: COLOR.ground };

/** The line a selection is said on, in the headings' row. */
const SELECTED_LINE: CSSProperties = {
  padding: '0 10px',
  background: COLOR.panel,
  borderBottom: `1px solid ${COLOR.border}`,
  color: COLOR.text,
  fontSize: 12,
  fontWeight: 'normal',
  textAlign: 'left',
  whiteSpace: 'nowrap',
};

/** A button on one of the bars over the rows. */
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
