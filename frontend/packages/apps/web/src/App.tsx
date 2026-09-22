import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';

import {
  addFiles,
  getFolders,
  getLibrary,
  getListing,
  lockServer,
  refreshCatalog,
  type Added,
  type CatalogState,
  type LibraryState,
} from '@coffret/api';

import { askToAdd } from './dropped';
import { FileList } from './FileList';
import { isPutAway, shownRuns } from './dismissed';
import { addingLine, collectingLine, fillOfFolder } from './fill';
import { FolderTree } from './FolderTree';
import { parseHash, toHash, type ViewState } from './hash';
import { askToLock, lockLanded } from './lock';
import {
  folderUnder,
  foldersWith,
  isPending,
  nameDefect,
  pendingAfter,
  strandedFolders,
} from './newFolder';
import { pageAt, pagesOf } from './pages';
import { ReaderView } from './ReaderView';
import { askWhatIsNew, catalogLine, catchUpLanded } from './refresh';
import { StatusBar } from './StatusBar';
import { COLOR } from './theme';
import { useActivity } from './useActivity';
import { said, useRemote, type Remote } from './useRemote';

/**
 * The explorer's one screen: a folder tree, the current folder's children, and
 * a status bar — with the reader over the list when a file is open.
 *
 * Where the screen is standing lives in the URL, so a reload and the back button
 * both come back to the folder that was open rather than to the top of the
 * Library.
 *
 * One thing on it is not in the URL and cannot be: the folders somebody made
 * here that the Library has never heard of. A Library has no folders to make —
 * a folder is the separators in the Entry Paths under it — so such a place is
 * this screen's until a book dropped into it commits, and a reload is where an
 * abandoned one goes.
 */
export function App() {
  const [view, setView] = useState<ViewState>(() => parseHash(window.location.hash));
  const [fetching, setFetching] = useState<string | null>(null);
  const [adding, setAdding] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // The back and forward buttons are the browser's, and this is all it takes to
  // honour them: wherever the URL says the screen is, is where it is.
  useEffect(() => {
    const restore = () => {
      setView(parseHash(window.location.hash));
      // The notice answers one click on one row, and a move away from that row
      // is the end of it. `go` clears it for the moves it makes; these are the
      // moves it never sees, and a notice left standing would name a file the
      // folder now on the screen does not hold.
      setNotice(null);
    };
    window.addEventListener('hashchange', restore);
    window.addEventListener('popstate', restore);
    return () => {
      window.removeEventListener('hashchange', restore);
      window.removeEventListener('popstate', restore);
    };
  }, []);

  const go = useCallback((next: ViewState, replace = false) => {
    setView(next);
    setNotice(null);
    const current = parseHash(window.location.hash);
    if (current.folder === next.folder && current.open === next.open) {
      return;
    }
    // A page turn replaces the entry it came from; everything else — a folder,
    // opening the reader, closing it — adds one. Otherwise reading fifty pages
    // would leave fifty entries between a reader and the folder they were in,
    // and the back button would walk them one at a time.
    //
    // `replace` is for a move nobody asked for. An entry for one of those is an
    // entry the back button lands on and is sent straight out of again, which
    // from the outside is a back button that does nothing.
    const turning =
      current.folder === next.folder && current.open !== null && next.open !== null;
    const how = replace || turning ? 'replaceState' : 'pushState';
    window.history[how](null, '', toHash(next));
  }, []);

  // What the server is bringing over behind the screen. Followed while the
  // reader is open — a page it fetches is what arms a fill — and for as long
  // after that as the fill runs, so closing the reader does not stop the folder
  // filling or the rows saying so.
  //
  // Above the three regions below because the screen's one try-again reaches it
  // too, and a callback cannot be named before it exists.
  const activity = useActivity(view.open !== null);
  const fill = activity.fill;
  const sync = activity.sync;
  const freeze = activity.freeze;
  const recheckActivity = activity.recheck;

  const library = useRemote((signal) => getLibrary(signal), 'library');
  const folders = useRemote((signal) => getFolders(signal), 'folders');
  const listing = useRemote((signal) => getListing(view.folder, signal), `list:${view.folder}`);

  // Each region's own "ask again", pulled out so that the things below built out
  // of them can be built once. Every one of them is stable — see
  // [`useRemote`](./useRemote) — so anything holding one holds it for the life of
  // the screen.
  const reloadLibrary = library.reload;
  const reloadFolders = folders.reload;
  const reloadListing = listing.reload;

  // One try-again for the screen, wherever it is pressed: the three requests
  // fail together far more often than apart, because what they fail at is the
  // server not being there. A retry that asked only for its own region would
  // leave the status bar naming a failure the rest of the screen had recovered
  // from — and the status bar has no button of its own to press.
  //
  // What the server is doing on its own is asked again with them, although it
  // is not a region and shows no refusal of its own. The question a page asks
  // as it comes up can fail like any other, and a tab that read that failure as
  // an answer would never again say what is running — with nothing on the
  // screen to press about it, because the sentence a failed activity request
  // would have written is deliberately not shown. The three below fail with it
  // far more often than apart, so the button that recovers them is the one that
  // recovers this.
  const retry = useCallback(() => {
    reloadLibrary();
    reloadFolders();
    reloadListing();
    recheckActivity();
  }, [reloadLibrary, reloadFolders, reloadListing, recheckActivity]);

  // Ending this server's hold on the Master Key. The keys were derived once,
  // when the server was started, and they live until this — or the interval it
  // goes unasked for — ends them. What one takes to do is in
  // [`lock`](./lock); this is where the screen is wired to it.
  //
  // What it does to the screen is give up the pages this device decrypted and
  // ask the three questions again, and that is the whole of the reporting: the
  // listing and the tree come back refused with the server's own sentence about
  // the Passphrase, in the same place every other refusal is shown, and the
  // status bar keeps the Library's name because that is not a thing the Master
  // Key kept. Nothing here invents a locked state of its own — the server is
  // the one that knows, and it is asked.
  //
  // `discarded` is a count of the times the pages held on this device have been
  // given up, and it is deliberately not a state of being locked: the reader
  // reads it as one instruction to let go of what it is holding, and goes on
  // showing whatever its next request earns. Which is what lets the other lock
  // use the same road — the interval the server goes unasked for ends the keys
  // without anybody pressing anything (spec: DK-4), and the screen hears of it
  // in the answer about what the server is doing and counts it here. `held`
  // below is the last state that answer gave, and the press records `locked` on
  // it so that the poll behind it is not read as a second lock.
  //
  // What it ends is the holding and not the reading. The refused listing ends
  // that, a moment later and by itself: `pages` below comes out of the listing's
  // answer, so a folder that cannot be listed has no page open in it and the
  // reader comes off the screen, leaving the refusal standing over the list.
  // The discard does not wait for that answer — plaintext held for the width of
  // a round trip is plaintext held past the key.
  //
  // `locking` — a lock is in flight — is in a ref as well as in state, for the
  // reason the refresh below gives.
  const [locking, setLocking] = useState(false);
  const [discarded, setDiscarded] = useState(0);
  const held = useRef<LibraryState | null>(null);
  const shutting = useRef(false);
  const lock = () => {
    if (shutting.current) {
      return;
    }
    shutting.current = true;
    setLocking(true);
    void askToLock({
      ask: lockServer,
      discard: () => {
        held.current = 'locked';
        setDiscarded((given) => given + 1);
      },
      reload: retry,
      trouble: setNotice,
    }).finally(() => {
      shutting.current = false;
      setLocking(false);
    });
  };

  // What is new in the Library, asked for and never polled for. The catalog is
  // what every listing comes out of, so a refresh that advanced it has changed
  // the tree's answer and the folder's alike — which is why both are asked
  // again and neither is edited here.
  //
  // Only ever one at a time, in a ref rather than in the state the button is
  // disabled from: what this guards against is the second of two clicks in one
  // frame, which is a render apart from the first.
  const [refreshing, setRefreshing] = useState(false);
  const [refreshed, setRefreshed] = useState<string | null>(null);
  const [refreshTrouble, setRefreshTrouble] = useState<string | null>(null);
  const looking = useRef(false);
  const refresh = useCallback(() => {
    if (looking.current) {
      return;
    }
    looking.current = true;
    setRefreshing(true);
    void askWhatIsNew({
      ask: refreshCatalog,
      line: setRefreshed,
      trouble: setRefreshTrouble,
      reload: () => {
        reloadFolders();
        reloadListing();
      },
    }).finally(() => {
      // How the catalog stands is the server's to say, and this is the gesture
      // that moves it: one that landed takes "this device has not caught up"
      // off the screen, and one Storage refused puts it there. Asked here
      // rather than beside the reload above, because it is true of both endings
      // and the reload happens only for one of them.
      recheckActivity();
      looking.current = false;
      setRefreshing(false);
    });
  }, [reloadFolders, reloadListing, recheckActivity]);

  // The other lock, arriving as news rather than as a gesture. The only place
  // this window can hear it is the answer it is already asking for while a
  // reader is open. What it does about it is what the press does once the
  // server has answered — give up the pages this device decrypted, and ask the
  // screen's questions again — minus the asking, since the Library is shut
  // already.
  //
  // Which answers are news is [`lockLanded`](./lock). The state it is read
  // against is in a ref rather than in state because nothing on the screen is
  // drawn from it.
  //
  // The notice goes down with the pages, as the press takes its own down before
  // it asks ([`askToLock`](./lock)): what stands there answers a gesture made
  // over rows that are about to leave the screen, and one of the sentences it
  // can be holding — "the Library is still open on this device", from a refused
  // press — would otherwise stand over a screen refusing everything.
  const custody = activity.library;
  useEffect(() => {
    if (custody === null) {
      return;
    }
    const before = held.current;
    held.current = custody;
    if (lockLanded(before, custody)) {
      setDiscarded((given) => given + 1);
      setNotice(null);
      retry();
    }
  }, [custody, retry]);

  // Files landing is the listing changing, and which rows changed is the
  // server's to say: the folder is asked again as the counts advance rather than
  // the rows being edited here. `done` moving is one file placed; the status
  // changing is the last of them, or the end of trying.
  //
  // Walking into a folder a fill has been working on asks once for the same
  // reason, which is not waste: what the listing arrived with may already be
  // behind what the fill has placed since.
  const progress = fill?.folder === view.folder ? `${fill.done}:${fill.status}` : null;
  useEffect(() => {
    if (progress !== null) {
      reloadListing();
    }
  }, [progress, reloadListing]);

  // And the same for the sync, for the same reason: the rows an added file has
  // are the folder's own answer about it, and the moment the sync commits they
  // become ordinary Entry rows. Which is why this watches the status and not the
  // count — a sync reports what it did when it has done it.
  const syncing = sync?.status ?? null;
  useEffect(() => {
    if (syncing !== null) {
      reloadListing();
    }
  }, [syncing, reloadListing]);

  // And the freeze, which is the same again with the tree added: a book that
  // committed is the first Entry under a folder made here, and until that
  // commit the Library has no such folder to name. So the tree is asked again
  // too, and the folder stops being this screen's own the moment the server
  // answers with it.
  //
  // The folder is in the key beside the status, the way a fill's count is. A
  // freeze is of one folder, so the second book of a session that was over
  // before the first poll of it would otherwise read `done` after `done` and
  // ask for nothing — leaving its pages saying `uploading` and its folder
  // dimmed until something else happened to reload them.
  const packing = freeze === null ? null : `${freeze.folder}:${freeze.status}`;
  useEffect(() => {
    if (packing !== null) {
      reloadListing();
      reloadFolders();
    }
  }, [packing, reloadListing, reloadFolders]);

  // And a catch-up landing, which is the largest of these: every folder's answer
  // comes out of the catalog, so one that reached the Library's head has changed
  // the tree and the open folder at once. The banner above the rows says so
  // while it runs — "the rest arrives when the catch-up lands" — and this is
  // what makes that true for somebody who waited rather than pressing anything.
  // Without it the banner goes away on its own and leaves the rows the catalog
  // had before, with nothing left on the screen to say they are not the Library.
  //
  // Which answers are that news is [`catchUpLanded`](./refresh), read the way
  // the lock's is read above: against the last state this window was told,
  // which is in a ref because nothing on the screen is drawn from it.
  const catalogState = activity.catalog?.state ?? null;
  const stood = useRef<CatalogState | null>(null);
  useEffect(() => {
    if (catalogState === null) {
      return;
    }
    const before = stood.current;
    stood.current = catalogState;
    if (catchUpLanded(before, catalogState)) {
      reloadListing();
      reloadFolders();
    }
  }, [catalogState, reloadListing, reloadFolders]);

  // The folders made in this browser that the Library does not hold yet, which
  // is what `newFolder` is about. They are pruned against every answer the
  // server gives, so the moment the freeze commits the folder becomes an
  // ordinary one this screen has no second opinion about.
  const [pending, setPending] = useState<readonly string[]>([]);
  const known = folders.state.status === 'ready' ? folders.state.value.folders : null;
  useEffect(() => {
    if (known !== null) {
      setPending((made) => {
        const left = pendingAfter(made, known);
        return left.length === made.length ? made : left;
      });
    }
  }, [known]);

  // And the ones that come back. A book whose freeze has not committed —
  // stopped by Storage, waiting its turn behind another, thrown away by a
  // worker that died, or still packing when the tab went away — is not an
  // abandoned folder, and the freeze naming it is in the answer this page asks
  // for as it comes up. So the folders go back among the ones made here — the
  // tree names them, the rows and the banner are reachable again, and the
  // status bar's "pack again" is offered over a place somebody can walk into
  // rather than over a name with nothing behind it.
  //
  // Read against the tree's answer, which is why it waits for one: a book that
  // committed before the run ended left a folder the Library holds, and that
  // one is the server's to answer for.
  const stranded = useMemo(
    () => (known === null ? [] : strandedFolders(freeze, known)),
    [freeze, known],
  );
  useEffect(() => {
    if (stranded.length === 0) {
      return;
    }
    setPending((made) => {
      const back = stranded.filter((folder) => !isPending(made, folder));
      return back.length === 0 ? made : [...made, ...back];
    });
  }, [stranded]);

  const drawn = useMemo(
    () => (known === null ? [] : foldersWith(known, pending)),
    [known, pending],
  );

  // Making one. The name is asked for the way a browser asks for one, because
  // there is nothing else on this screen it could be typed into and a field that
  // appeared for one gesture would be a second thing to dismiss.
  //
  // What it does is move the screen there. The folder is empty by construction —
  // nothing has ever been in it — so what a person does next is drop the book it
  // was made for.
  const newFolder = useCallback(() => {
    const typed = window.prompt(
      view.folder === ''
        ? 'a name for the new folder in the Library'
        : `a name for the new folder in ${view.folder}`,
    );
    if (typed === null) {
      return;
    }
    const name = typed.trim();
    const defect = nameDefect(name);
    if (defect !== null) {
      setNotice(`no folder was made — ${defect}`);
      return;
    }
    const path = folderUnder(view.folder, name);
    if (known?.includes(path) === true || isPending(pending, path)) {
      setNotice(`there is already a folder called ${name} here`);
      return;
    }
    setPending((made) => [...made, path]);
    go({ folder: path, open: null });
  }, [view.folder, known, pending, go]);

  // Whether a drop onto the folder on the screen is a book being brought in.
  //
  // A folder made here and not yet in the Library is one being filled in a
  // single gesture, which is what importing a book is; anything else is files
  // being added to a folder that already exists, and that is a sync as it always
  // was. The server is told which of the two this is rather than left to guess,
  // because from there the two look identical.
  const bookDrop = isPending(pending, view.folder);

  // Files dropped on the list are added to the folder it is showing. The listing
  // is asked for again as soon as they land, which is what puts them on the
  // screen: they are in the folder from that moment, and the folder is what
  // knows they are there until the flow behind them commits.
  //
  // A refusal about one part is said in the notice area beside the rows, because
  // it is about a file on this screen; a refusal about the whole drop is said the
  // same way, since it is the same sentence about all of them at once.
  //
  // Which of those the drop came to, and what is asked again because of it, is
  // [`askToAdd`](./dropped) — the two guards below are this screen's, since both
  // are about state only it holds.
  const add = useCallback(
    (files: Added[]) => {
      if (files.length === 0) {
        setAdding(null);
        // An empty folder, or something that was never a file — a selection of
        // text, an image dragged out of another page. The gesture was made and
        // there is nothing to show for it, and a screen that does not react at
        // all is one a person reads as broken rather than as answered.
        setNotice('nothing was added — that drop carried no files');
        return;
      }
      setAdding(addingLine(files.length, view.folder));
      void askToAdd({
        ask: () => addFiles(view.folder, files, { freeze: bookDrop }),
        notice: setNotice,
        reload: reloadListing,
        follow: activity.follow,
      }).finally(() => setAdding(null));
    },
    [view.folder, bookDrop, activity, reloadListing],
  );

  // The word the drop itself gets, before there is anything to send. A browser
  // hands a dropped folder over as something to walk, one batch of children at
  // a time, and a nested folder of several hundred pages is seconds of that
  // with nothing on the screen changing — the files land one folder down, so
  // the folder being looked at gains no row to say they are there. Replaced by
  // the line naming the count as soon as the walk has one.
  const collecting = useCallback(() => setAdding(collectingLine()), []);

  // And the end that walk can come to instead of files: a browser that refused
  // to read a dropped folder's children. Nothing was sent, so nothing else on
  // the screen is going to change — and the line the drop put up would stand
  // saying the drop was being read for as long as the tab was open.
  const unreadable = useCallback((cause: unknown) => {
    setAdding(null);
    setNotice(`nothing was added — that drop could not be read: ${said(cause)}`);
  }, []);

  // The fill as the rows are allowed to read it. A person who put the fill's
  // line away has put away what that fill had to say, and the `failed` and
  // `declined` chips it wrote over the rows are the same sentence in the other
  // place: leaving them standing would be a dismissal that dismissed half of
  // one thing. The rows fall back to what the listing says, which is the one
  // answer about what is on this device anyway.
  const shownFill = isPutAway(activity.dismissed, 'fill', fill) ? null : fill;

  // And which run the rows of the folder on the screen read, which is not always
  // the one on record. A fill Storage stopped keeps its line and its offer of a
  // second attempt after the next folder has taken the record from it, and the
  // `failed` and `declined` chips over these rows are the other half of what
  // that line says — so they are owed for as long as it is. Which of those runs
  // the bar is still showing is read the way the bar reads it: by name, since a
  // run the record was taken from is never the one a dismissal's number names.
  const rowsFill = useMemo(
    () =>
      fillOfFolder(
        shownFill,
        shownRuns(activity.dismissed, 'fill', fill?.displaced ?? []),
        view.folder,
      ),
    [shownFill, activity.dismissed, fill, view.folder],
  );

  // Everywhere that is not the list. A browser's own answer to a file dropped on
  // a page is to leave the page and show the file, and on a screen that has just
  // taught somebody that dropping files here adds them, a miss — the tree, the
  // status bar, the reader standing over the list — would take the explorer away
  // and whatever was open in it. So the page refuses the drops the list did not
  // take. The list's own handler runs first and does the adding; this is only
  // what happens to the ones that reach nothing.
  useEffect(() => {
    const ignore = (event: DragEvent) => event.preventDefault();
    window.addEventListener('dragover', ignore);
    window.addEventListener('drop', ignore);
    return () => {
      window.removeEventListener('dragover', ignore);
      window.removeEventListener('drop', ignore);
    };
  }, []);

  // What the screen says about a catalog that is not the Library's, which is
  // nothing at all while it is.
  const catalogSaid = catalogLine(activity.catalog);

  const listed = listing.state.status === 'ready' ? listing.state.value : null;
  const pages = useMemo(() => (listed === null ? [] : pagesOf(listed.files)), [listed]);
  const openAt = view.open === null ? null : pageAt(pages, view.open);

  // The last place the reader stood. Closing it comes back to the list with
  // that row marked, so a folder of two hundred names does not have to be read
  // through again to find where the reading stopped — and a reload that came
  // back straight into the reader leaves the list scrolled to it, not to the
  // top of a folder the reader was never at the top of.
  const [wasOpen, setWasOpen] = useState<ViewState>(view);
  useEffect(() => {
    if (view.open !== null) {
      setWasOpen(view);
    }
  }, [view]);
  const selected = wasOpen.folder === view.folder ? wasOpen.open : null;

  // A hash naming a file this folder does not offer — the Entry is gone, or the
  // URL was written by hand and names a row a browser draws nothing from. The
  // list is the answer, and the URL is put back in step with it.
  //
  // Against this folder's own listing and no other. The back button, the
  // forward button, and a hash typed into the address bar can move the folder
  // and the open file in one step, and until the listing for the new one lands
  // the rows on hand are still the folder the screen just left — which offers
  // no such file for the plain reason that the file is not in it. Judged
  // against those, every such move would close the reader on the way into it
  // and write the correction over the very history entry it arrived by.
  const stale = view.open !== null && openAt === null && listed?.path === view.folder;
  const folder = view.folder;
  useEffect(() => {
    if (stale) {
      go({ folder, open: null }, true);
    }
  }, [stale, folder, go]);

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      {/* The reader covers the tree and the list and stops at the status bar,
          which is where it says what it is fetching. */}
      <div style={{ flex: 1, display: 'flex', minHeight: 0, position: 'relative' }}>
        {/* The tree's column, drawn whether or not the tree is in it yet: a
            loading line and a refusal stand where the tree will stand, rather
            than shouldering the list across the screen and handing the width
            back when the folders arrive. */}
        <aside
          style={{
            width: 260,
            flex: '0 0 auto',
            overflow: 'auto',
            background: COLOR.panel,
            borderRight: `1px solid ${COLOR.border}`,
          }}
        >
          <Region state={folders.state} onRetry={retry}>
            {() => (
              <FolderTree
                folders={drawn}
                pending={pending}
                current={view.folder}
                onOpen={(chosen) => go({ folder: chosen, open: null })}
                onNewFolder={newFolder}
              />
            )}
          </Region>
        </aside>
        <main style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
          {/* Why the rows below may not be the Library. Every listing comes out
              of this device's catalog, and the catalog holds what this device
              has replayed — so a device fresh from `join` whose catch-up did
              not land shows an empty folder tree, and shows it in exactly the
              way a Library with nothing in it does. It stands above the notice
              because it is the older and larger fact: a gesture that came to
              nothing is about one click, and this is about everything on the
              screen. */}
          {catalogSaid !== null && (
            <p
              style={{
                margin: 0,
                padding: '8px 12px',
                borderBottom: `1px solid ${COLOR.border}`,
                color: COLOR.warn,
                fontSize: 13,
              }}
            >
              {catalogSaid}
            </p>
          )}
          {/* The answer to a gesture that came to nothing — a row clicked and
              not opened, files dropped and not added. It stands above the list
              and not inside it, so that it is on the screen whatever the list
              is scrolled to — and it is said in the colour of something a
              reader is told rather than in the grey of the columns, because a
              gesture that was made and answered by nothing else is where a
              sentence has to be noticed to be of any use. */}
          {notice !== null && (
            <p
              style={{
                margin: 0,
                padding: '8px 12px',
                borderBottom: `1px solid ${COLOR.border}`,
                color: COLOR.warn,
                fontSize: 13,
              }}
            >
              {notice}
            </p>
          )}
          <Region state={listing.state} onRetry={retry}>
            {(held) => (
              <FileList
                listing={held}
                fill={rowsFill}
                freeze={freeze}
                bookDrop={bookDrop}
                selected={selected}
                onOpenFolder={(chosen) => go({ folder: chosen, open: null })}
                onOpenFile={(path) => go({ folder: view.folder, open: path })}
                onUnsupported={(file) =>
                  setNotice(`${file.name} — preview of this format is not supported yet`)
                }
                onAdd={add}
                onCollecting={collecting}
                onUnreadable={unreadable}
                // The same sentence the server would have answered with, said
                // here because this drop never becomes a request: the folder is
                // not on this device, so there is nowhere to put a single one of
                // its files (spec: EP-9).
                onUnmapped={() =>
                  setNotice(
                    'nothing was added — no folder on this device holds this part of the Library',
                  )
                }
              />
            )}
          </Region>
        </main>
        {openAt !== null && (
          <ReaderView
            pages={pages}
            at={openAt}
            discarded={discarded}
            onNavigate={(next) => go({ folder: view.folder, open: pages[next].path })}
            onClose={() => go({ folder: view.folder, open: null })}
            onFetching={setFetching}
            onFetched={listing.reload}
          />
        )}
      </div>
      <StatusBar
        library={library.state}
        fetching={fetching}
        adding={adding}
        fill={fill}
        sync={sync}
        freeze={freeze}
        trouble={activity.trouble}
        dismissed={activity.dismissed}
        onDismiss={activity.dismiss}
        onRetryFill={activity.retry}
        onRetrySync={activity.retrySync}
        onRetryFreeze={activity.retryFreeze}
        onLock={lock}
        locking={locking}
        refresh={{
          running: refreshing,
          said: refreshed,
          refused: refreshTrouble,
          ask: refresh,
        }}
      />
    </div>
  );
}

/**
 * One region of the screen, in whichever of its three states it is in.
 *
 * Loading and failed are both states the screen can be left from, which is the
 * point of stating them: a request that failed says what the server said and
 * offers to ask again, rather than leaving the region saying "loading" for as
 * long as the tab is open.
 */
function Region<T>({
  state,
  onRetry,
  children,
}: {
  state: Remote<T>;
  onRetry: () => void;
  children: (value: T) => ReactNode;
}) {
  switch (state.status) {
    case 'loading':
      return <p style={{ padding: 16, color: COLOR.dim }}>loading…</p>;
    case 'failed':
      return (
        <div style={{ padding: 16 }}>
          <p style={{ color: COLOR.refused }}>{state.message}</p>
          <button
            onClick={onRetry}
            style={{
              border: `1px solid ${COLOR.border}`,
              background: COLOR.panel,
              color: COLOR.text,
              font: 'inherit',
              padding: '4px 12px',
              borderRadius: 4,
              cursor: 'pointer',
            }}
          >
            try again
          </button>
        </div>
      );
    case 'ready':
      return <>{children(state.value)}</>;
  }
}
