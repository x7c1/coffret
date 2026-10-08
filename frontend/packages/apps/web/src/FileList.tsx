import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';

import type { Added, DisplacedFill, Fill, Freeze, ListedFile, Listing } from '@coffret/api';

import { droppedFiles } from './drop';
import { dropLine, dropOutcome, type DropOutcome } from './dropTarget';
import { freezingHere, isFreezing, rowFill, SAYS, type RowState } from './fill';
import { size, time } from './humanize';
import { mapLabel, MAP_THE_ROOT } from './mapping';
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
 * Every gesture such a folder refuses is answered that way, the click on a row
 * included. A row there is not offered to the pointer and does not open, and it
 * still says why when it is clicked: the alternative is a person clicking a
 * name over and over at a screen that never once reacts.
 *
 * What a drop is added as follows what was dropped, not where: files on their
 * own go one at a time, and a drop holding a folder is asked about before
 * anything is sent — that question is the screen's, after this list has handed
 * the files over.
 */
export function FileList({
  listing,
  fill,
  freeze,
  madeHere,
  madeHereKnown,
  selected,
  onOpenFolder,
  onOpenFile,
  onUnsupported,
  onAdd,
  onCollecting,
  onUnreadable,
  onUnmapped,
  onMap,
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
   * Whether this is a folder made in this browser that the Library does not
   * hold yet. It decides nothing about how a drop is added — only what is said
   * over a folder with nothing in it.
   */
  madeHere: boolean;
  /**
   * Whether the folders made in this browser are known yet.
   *
   * `madeHere` is read off them, and they are not all on hand when this screen
   * comes up: a folder whose book never committed is put back among them out of
   * the folder tree's answer, which is a request of its own beside this
   * listing's. Until that lands, a `madeHere` of false means "not known to be
   * one" rather than "not one" — and `false` here says so, so that nothing on
   * this screen states as a fact what only that answer decides.
   */
  madeHereKnown: boolean;
  /** The Entry Path the reader was last opened at here, if any. */
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
}) {
  // Whether something is being dragged over the list right now. A `dragenter` and
  // a `dragleave` fire for every element the pointer crosses inside it, so this
  // is counted rather than set: a `dragleave` off a row onto the row below it
  // would otherwise take the outline away in the middle of the drag.
  const [over, setOver] = useState(0);
  const dragged = over > 0;
  // Whether what is being dragged carries files, as the browser says on the way
  // in. Only such a drag gets the line saying what letting go will do: a
  // selection of text or a link dropped here adds nothing (the drop says "that
  // drop carried no files"), and a line promising otherwise would be wrong.
  const [carriesFiles, setCarriesFiles] = useState(false);
  const root = listing.path === '';
  const empty = listing.folders.length === 0 && listing.files.length === 0;
  // A path the Library names nothing at, as far as what is on hand goes. A
  // folder of the Library is what the separators under it imply, so `held` being
  // false means nothing is under this one — and a folder made in this browser is
  // the one place that is a state rather than a mistake, since the Library has
  // not heard of it yet and the whole point of it is what gets dropped in next.
  const unheld = !listing.held && !madeHere;
  // And the same thing said out loud, which waits for the one answer that can
  // still overturn it. A folder stranded by a book that never committed rejoins
  // the ones made here off the folder tree, a separate request from this
  // listing, so until it lands `madeHere` is false even for a folder whose book
  // is being packed this minute: somebody who closed the tab mid-packing and
  // reopened that folder would read that the Library has nothing of theirs
  // there. Said once the folders are known, and not at all where that request
  // failed — the screen is already showing the tree's own trouble, and a fact
  // this page could not confirm is not one to assert.
  const nowhere = unheld && madeHereKnown;
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
  const takesADrop = outcome !== 'refused';
  // A folder made here, still empty, waiting for what it was made for. Said
  // because the Library has no such folder yet and the rows cannot say why the
  // place exists — and said only where a drop would in fact be taken, since
  // inviting files into a folder no mapping of this device reaches would
  // contradict the banner above it.
  const waitingForADrop = madeHere && takesADrop && empty;
  return (
    <div
      style={{
        flex: 1,
        overflow: 'auto',
        minWidth: 0,
        // Inside the outline rather than around it, so that nothing on the
        // screen moves when a drag arrives: an outline that took up room would
        // shift every row under the pointer at the moment of dropping.
        //
        // A folder that would not take this drop is outlined too, and in the
        // refused colour. The answer to "will this take my files" is worth
        // having while the files are still in the air, and a list that simply
        // did not light up would leave a person to find out by letting go.
        outline: dragged
          ? `2px dashed ${takesADrop ? COLOR.added : COLOR.refused}`
          : undefined,
        outlineOffset: -2,
      }}
      // The three that have to be answered for a drop to happen at all. The
      // browser's own default for a dropped file is to navigate to it, which
      // would replace the explorer with the picture — so both of the first two
      // are prevented, and `dragover` on every pass rather than once.
      onDragEnter={(event) => {
        event.preventDefault();
        setOver((crossed) => crossed + 1);
        setCarriesFiles(event.dataTransfer.types.includes('Files'));
      }}
      onDragOver={(event) => event.preventDefault()}
      onDragLeave={() => setOver((crossed) => Math.max(0, crossed - 1))}
      onDrop={(event) => {
        event.preventDefault();
        setOver(0);
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
        {waitingForADrop && <WaitingForADrop />}
        {/* While a drag is over the list, the line saying what letting go will
            do. Laid over the rows under the banners rather than among them, for
            the reason the outline is drawn inside the list: a line that took up
            room would shift every row under the pointer as the drag arrived. */}
        {dragged && carriesFiles && (
          <div style={{ position: 'absolute', top: '100%', left: 0, right: 0 }}>
            <DropBanner outcome={outcome} held={!nowhere} />
          </div>
        )}
      </div>
      {empty ? (
        // A folder waiting for a drop has been told what it is for by the
        // banner above, and "this folder is empty" under it would be the screen
        // saying the same thing twice, the second time as though something were
        // missing.
        !waitingForADrop && (
          <p style={{ padding: 16, color: COLOR.dim }}>
            {/* Three states and not two. A folder of the Library holds
                something by definition, so "this folder is empty" over a path
                the Library has never held is the screen inventing a folder to
                describe — which is what somebody who mistyped a component into
                the address bar, or followed a link written before the Entries
                went, would read it as. The listing says which of the two it
                answered and this says it back — once the folders made here are
                known, since a folder whose book is still being packed is the
                one path the listing alone would have it wrong about, and where
                it is still out this falls back to the milder of the two. */}
            {nowhere
              ? NOTHING_AT_THIS_PATH
              : root
                ? 'this Library is empty'
                : 'this folder is empty'}
          </p>
        )
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
                // into the refusal. What the click gets instead is the sentence,
                // the way the drop above does — the row stays un-openable and
                // the attempt stops being met by nothing at all, which is the
                // one answer a person cannot tell from a screen that is broken.
                onActivate={
                  !listing.mapped
                    ? () => onUnmapped('open', !nowhere)
                    : file.openable
                      ? () => onOpenFile(file.path)
                      : () => onUnsupported(file)
                }
                offered={listing.mapped}
              >
                <td style={{ ...CELL, textAlign: 'right', color: COLOR.dim }}>
                  {size(file.size)}
                </td>
                <td style={{ ...CELL, color: COLOR.dim }}>{time(file.mtime)}</td>
                <td style={CELL}>
                  <StateChip file={file} folder={listing.path} fill={fill} />
                </td>
              </Row>
            ))}
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

function Row({
  icon,
  name,
  dim,
  selected,
  onActivate,
  offered = onActivate !== undefined,
  children,
}: {
  icon: string;
  name: string;
  dim?: boolean;
  selected?: boolean;
  onActivate?: () => void;
  /**
   * Whether this row is offered to the pointer, which is what the cursor and the
   * hover say.
   *
   * Apart from whether there is a handler at all, because one row has a handler
   * and is not offered: a file in a folder no mapping of this device reaches
   * answers a click with the reason it will not open, and a row that invited the
   * click first would make that sentence the second surprise rather than the
   * first. A row whose format the reader cannot draw is offered, and remains so
   * — the sentence it answers with is about the one file, in a folder where
   * every other row does open.
   */
  offered?: boolean;
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
      className={offered ? 'row activatable' : 'row'}
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
 * What a banner over the rows is drawn as.
 *
 * Not sticky of its own: the block the list draws its banners in is, so that
 * two standing at once stack rather than overlap as the rows scroll under them.
 */
function Banner({ tone, background, children }: { tone: string; background: string; children: ReactNode }) {
  return (
    <p
      style={{
        margin: 0,
        padding: '8px 12px',
        background,
        borderBottom: `1px solid ${tone}`,
        color: tone,
        fontSize: 13,
      }}
    >
      {children}
    </p>
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
 * Said in a folder made here that is still waiting for what it was made for.
 *
 * Making a folder decides nothing about how what is dropped into it is added:
 * that follows the drop, as it does everywhere else.
 */
function WaitingForADrop() {
  return (
    <Banner tone={COLOR.added} background={COLOR.addedGround}>
      this folder was made here and the Library does not have it yet — drop files
      or a folder in; a folder is asked about first, as a Pack or its files one by one
    </Banner>
  );
}

/**
 * Said while files are dragged over the list: what letting go of them here
 * will do, as `dropLine` words it. A folder that takes no drop says so in the
 * refused colour the outline around it is drawn in.
 */
function DropBanner({ outcome, held }: { outcome: DropOutcome; held: boolean }) {
  const refused = outcome === 'refused';
  return (
    <Banner
      tone={refused ? COLOR.refused : COLOR.added}
      background={refused ? COLOR.refusedGround : COLOR.addedGround}
    >
      {dropLine(outcome, held)}
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
 * is neither, nothing fetched at all. A folder made in this browser inside an
 * unmapped subtree is empty and still needs the banner, because the drop it was
 * made for is the gesture that would be refused; over rows that are not there,
 * both of the other clauses would point at something to fetch and the line
 * under them would say the folder is empty.
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
