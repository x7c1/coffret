import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';

import {
  addFiles,
  getFolders,
  getLibrary,
  getListing,
  refreshCatalog,
  type Added,
} from '@coffret/api';

import { FileList } from './FileList';
import { addingLine, isFreezing } from './fill';
import { FolderTree } from './FolderTree';
import { parseHash, toHash, type ViewState } from './hash';
import {
  folderUnder,
  foldersWith,
  isPending,
  nameDefect,
  pendingAfter,
  strandedFolder,
} from './newFolder';
import { pageAt, pagesOf } from './pages';
import { ReaderView } from './ReaderView';
import { askWhatIsNew } from './refresh';
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

  const library = useRemote((signal) => getLibrary(signal), 'library');
  const folders = useRemote((signal) => getFolders(signal), 'folders');
  const listing = useRemote((signal) => getListing(view.folder, signal), `list:${view.folder}`);

  // One try-again for the screen, wherever it is pressed: the three requests
  // fail together far more often than apart, because what they fail at is the
  // server not being there. A retry that asked only for its own region would
  // leave the status bar naming a failure the rest of the screen had recovered
  // from — and the status bar has no button of its own to press.
  const retry = () => {
    library.reload();
    folders.reload();
    listing.reload();
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
  const reloadFolders = folders.reload;
  const reloadListing = listing.reload;
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
      looking.current = false;
      setRefreshing(false);
    });
  }, [reloadFolders, reloadListing]);

  // What the server is bringing over behind the screen. Followed while the
  // reader is open — a page it fetches is what arms a fill — and for as long
  // after that as the fill runs, so closing the reader does not stop the folder
  // filling or the rows saying so.
  const activity = useActivity(view.open !== null);
  const fill = activity.fill;
  const sync = activity.sync;
  const freeze = activity.freeze;

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

  // And the one that comes back. A book whose freeze has not committed —
  // stopped by Storage, or still packing when the tab went away — is not an
  // abandoned folder, and the freeze naming it is in the answer this page asks
  // for as it comes up. So the folder goes back among the ones made here — the
  // tree names it, the rows and the banner are reachable again, and the status
  // bar's "pack again" is offered over a place somebody can walk into rather
  // than over a name with nothing behind it.
  //
  // Read against the tree's answer, which is why it waits for one: a book that
  // committed before the run ended left a folder the Library holds, and that
  // one is the server's to answer for.
  const stranded = known === null ? null : strandedFolder(freeze, known);
  useEffect(() => {
    if (stranded === null) {
      return;
    }
    setPending((made) => (isPending(made, stranded) ? made : [...made, stranded]));
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
  const add = useCallback(
    (files: Added[]) => {
      if (files.length === 0) {
        // An empty folder, or something that was never a file — a selection of
        // text, an image dragged out of another page. The gesture was made and
        // there is nothing to show for it, and a screen that does not react at
        // all is one a person reads as broken rather than as answered.
        setNotice('nothing was added — that drop carried no files');
        return;
      }
      // One book at a time. A second one dropped now would sit behind the first
      // on the server's one worker, with nothing on this screen able to say
      // where it had got to — so the gesture is answered rather than half taken.
      //
      // The sentence names the rule rather than the other book, because the
      // folder being dropped onto may well be the one already packing: somebody
      // adding the pages they missed would be told "one book is being packed
      // already" about their own.
      if (bookDrop && isFreezing(freeze)) {
        setNotice(
          'nothing was added — a book is being packed, and they are packed one at a time',
        );
        return;
      }
      setNotice(null);
      setAdding(addingLine(files.length, view.folder));
      void addFiles(view.folder, files, { freeze: bookDrop })
        .then(
          (upload) => {
            if (upload.refused.length > 0) {
              const [first] = upload.refused;
              const rest = upload.refused.length - 1;
              setNotice(
                rest === 0
                  ? `${first.name} — ${first.message}`
                  : `${first.name} — ${first.message} (and ${rest} more)`,
              );
            }
            if (upload.written.length > 0) {
              // The sync or the freeze the server armed as it answered. Nothing
              // has asked for the activity since this page last had a reason to,
              // so it is told there is something to follow.
              activity.follow();
            }
            reloadListing();
          },
          (refused: unknown) => setNotice(said(refused)),
        )
        .finally(() => setAdding(null));
    },
    [view.folder, bookDrop, freeze, activity, reloadListing],
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
                fill={fill}
                freeze={freeze}
                bookDrop={bookDrop}
                selected={selected}
                onOpenFolder={(chosen) => go({ folder: chosen, open: null })}
                onOpenFile={(path) => go({ folder: view.folder, open: path })}
                onUnsupported={(file) =>
                  setNotice(`${file.name} — preview of this format is not supported yet`)
                }
                onAdd={add}
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
        onRetryFill={activity.retry}
        onRetrySync={activity.retrySync}
        onRetryFreeze={activity.retryFreeze}
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
