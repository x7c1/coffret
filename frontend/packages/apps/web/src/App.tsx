import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  addFiles,
  askToUnlock,
  getFolders,
  getLibrary,
  getListing,
  getRemoteHead,
  previewDelete,
  previewFreeze,
  refreshCatalog,
  startDelete,
  startFreeze,
  startReconnect,
  type Added,
  type CatalogState,
  type DeleteTarget,
  type LibraryState,
  type ListedFile,
  type Listing,
  type Mapped,
  type ReconnectState,
} from '@coffret/api';

import {
  forgotten,
  parentOf,
  prepare,
  preparedAfter,
  prospectiveFolders,
} from './prospective';
import { ActivityPanel } from './ActivityPanel';
import { earlier, inProgress, offeredNow, type Entry, type PageNotice } from './activity';
import { DeleteConfirm } from './DeleteConfirm';
import { firstImage, uploadStopped } from './details';
import { DetailsPane } from './DetailsPane';
import { Header } from './Header';
import {
  askToDelete,
  refusedNotice,
  type DeleteNotice,
  type DeleteQuestion,
} from './deleteEntries';
import { destinationsOf, namedBy, onDiskIn, type Destination } from './destination';
import { nothingAddedLine, withNotTaken, type Drop, type NotTaken } from './drop';
import { systemFilesLine, type SystemFile } from './systemFiles';
import { askToAdd, overBudgetLine } from './dropped';
import { DropConfirm } from './DropConfirm';
import { addingLine, collectingLine, fillOfFolder, paced, rowFill } from './fill';
import { parseHash, sheetOpen, toHash, type ViewState } from './hash';
import { askForUnlock, lockLanded, lockUnheard, UnlockPrompting, unlockLanded } from './lock';
import { ChooseFolder } from './ChooseFolder';
import { folderToChoose } from './mapping';
import { NewFolderDialog } from './NewFolderDialog';
import {
  chooseAndAdd,
  heldIn,
  heldUnder,
  NOTHING_HELD,
  type Answer,
  type Choice,
  type DropSummary,
  type HeldFiles,
} from './packChoice';
import { askToPack, type PackQuestion } from './packFolder';
import { PackFolderConfirm } from './PackFolderConfirm';
import { ranOut } from './ranOut';
import { ReaderView } from './ReaderView';
import { NOTHING_SELECTED, selectedRows, type Selection } from './selection';
import { askToReconnect, grantRenewed } from './reconnect';
import { ReconnectOffer, type Offer } from './ReconnectOffer';
import type { Edits, Numbering } from './renumber';
import { askWhatIsNew, catalogLine, catchUpLanded } from './refresh';
import { RemoteHeadLooks } from './remoteHead';
import { counterOf, sheetKey, sheetOn } from './sheet';
import { Region } from './Region';
import { heldAfter, strandedFolders } from './stranded';
import { COLOR } from './theme';
import { useRunReloads } from './runReloads';
import {
  ancestry,
  contextOf,
  folderKey,
  keyed,
  shownFolders,
  treeRows,
  type FileRow,
  type FolderRow,
  type IncomingFile,
  type TreeState,
} from './tree';
import { opened, TreeTable } from './TreeTable';
import { heldBy } from './turns';
import { unmappedLine, type Tried } from './unmapped';
import { useListings } from './useListings';
import { useWork } from './useWork';
import { said, useAsked, type Asked } from './useAsked';

/**
 * The explorer's one screen: a header naming the Library, where the focus
 * stands, and Add files (see [`Header`](./Header)), the Library as one tree of
 * folders and files under it (see [`TreeTable`](./TreeTable)) with the details
 * pane beside it, and the Activity panel along the bottom (see
 * [`ActivityPanel`](./ActivityPanel)) — with the reader's sheet over the right
 * of the tree while it is open, following the focused row (see
 * [`ReaderView`](./ReaderView)).
 *
 * Where the screen is standing lives in the URL — the folder the focus is in,
 * and the row the reader's sheet is open on — so a reload and the back button both
 * come back to it rather than to the top of the Library. Walking the tree moves
 * the URL without adding to the history: the back button steps between the
 * places a person opened, not between the rows the arrow keys passed.
 *
 * A Library has no folders to make — a folder is the separators in the Entry
 * Paths under it (spec: EP-2) — so a folder comes to be by adding files to it:
 * picked with the header's **+** or dropped onto the tree, and confirmed with
 * the path it lands under before anything is sent. "New folder…" prepares the
 * path an add is to land under, held by this page alone and drawn as not in
 * the Library yet until one does (see [`prospective`](./prospective)).
 */
export function App() {
  const [view, setView] = useState<ViewState>(() => parseHash(window.location.hash));
  // Which row of the tree has the focus and which folders are open, opened at
  // first to wherever the URL says the screen is.
  const [tree, setTree] = useState<TreeState>(() =>
    restored({ focus: null, expanded: new Set() }, view),
  );
  // The rows selected besides the focused one, with modifier keys (see
  // [`selection`](./selection)). Read against the rows on show, so a row that
  // left the tree leaves it too.
  const [selection, setSelection] = useState<Selection>(NOTHING_SELECTED);
  const [fetching, setFetching] = useState<string | null>(null);
  const [adding, setAdding] = useState<string | null>(null);
  // How far the drop in flight has been sent, from 0 to 1, where the browser
  // has said: the bar beside its line in the Activity panel.
  const [sent, setSent] = useState<number | null>(null);
  // The answers to gestures made on this page that no request carried to the
  // server — a row asked to open and not opened, files dropped and not added —
  // newest first. They go to the Activity panel's **Earlier** beside the
  // server's own log, and live as long as the page: a sentence that answers a
  // request the server never heard, or that could not reach it at all, is
  // not one the server can be asked to keep.
  //
  // A deletion's notice may carry the target a *try again* beside it asks about
  // again (see [`DeleteNotice`](./deleteEntries)); every other notice is a
  // sentence alone, set through `setNotice`.
  const [notices, setNotices] = useState<readonly PageNotice[]>([]);
  const noticed = useRef(0);
  const showNotice = useCallback((notice: DeleteNotice) => {
    noticed.current += 1;
    const id = noticed.current;
    setNotices((held) =>
      [
        {
          id,
          at: new Date().toISOString(),
          text: notice.text,
          outcome: notice.retry === null ? ('notice' as const) : ('failure' as const),
          retry: notice.retry,
          settled: false,
        },
        ...held,
      ].slice(0, NOTICES_KEPT),
    );
  }, []);
  // `null` says nothing: a notice is part of what happened on this page, and a
  // later gesture does not take it back.
  const setNotice = useCallback(
    (text: string | null) => {
      if (text !== null) {
        showNotice({ text, retry: null });
      }
    },
    [showNotice],
  );

  // The back and forward buttons are the browser's, and this is all it takes to
  // honour them: wherever the URL says the screen is, is where it is.
  useEffect(() => {
    const restore = () => {
      const next = parseHash(window.location.hash);
      setView(next);
      setTree((held) => restored(held, next));
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
    const current = parseHash(window.location.hash);
    if (toHash(current) === toHash(next)) {
      return;
    }
    // A page turn replaces the entry it came from; everything else — a folder,
    // opening the reader, closing it — adds one. Otherwise reading fifty pages
    // would leave fifty entries between a reader and the folder they were in,
    // and the back button would walk them one at a time. A turn is the sheet
    // following the focus, which `changeTree` asks to replace anyway; this
    // catches the one made by any other road.
    //
    // `replace` is for a move nobody asked for. An entry for one of those is an
    // entry the back button lands on and is sent straight out of again, which
    // from the outside is a back button that does nothing.
    const turning = current.folder === next.folder && sheetOpen(current) && sheetOpen(next);
    const how = replace || turning ? 'replaceState' : 'pushState';
    window.history[how](null, '', toHash(next));
  }, []);

  // A move the screen makes on its own — out of a folder a deletion took away,
  // or a prepared path forgotten — which the tree follows: opened down to the
  // folder, and the focus on it.
  const moveTo = useCallback(
    (next: ViewState, replace = false) => {
      go(next, replace);
      setTree((held) => restored(held, next));
    },
    [go],
  );

  // What the server is bringing over behind the screen. Followed while the
  // reader is open — a page it fetches is what arms a fill — and for as long
  // after that as the fill runs, so closing the reader does not stop the folder
  // filling or the rows saying so.
  //
  // Above the three regions below because the screen's one try-again reaches it
  // too, and a callback cannot be named before it exists.
  const work = useWork(sheetOpen(view));
  const fill = work.fill;
  const sync = work.sync;
  const freeze = work.freeze;
  const deletion = work.deletion;
  const turn = work.turn;
  const recheckWork = work.recheck;

  const library = useAsked((signal) => getLibrary(signal), 'library');
  const folders = useAsked((signal) => getFolders(signal), 'folders');
  // The listing of every folder the tree shows — the root's, each open
  // folder's — of the folder the URL names, which is one of them once the
  // tree has opened down to it (see [`useListings`](./useListings)), and of
  // the focused folder, open or not: the details pane draws its first image
  // and counts its items out of it.
  const focusedFolder =
    tree.focus !== null && tree.focus.startsWith(folderKey(''))
      ? tree.focus.slice(folderKey('').length)
      : null;
  const shown = useMemo(() => {
    const open = shownFolders(tree.expanded);
    const named = open.includes(view.folder) ? open : [...open, view.folder];
    return focusedFolder === null || named.includes(focusedFolder) ? named : [...named, focusedFolder];
  }, [tree.expanded, view.folder, focusedFolder]);
  const listings = useListings(shown);
  const reloadListings = listings.reload;
  const heldListing = listings.listing;
  const rootState: Asked<Listing> = heldListing('') ?? LOADING;
  // The folder the screen's actions are about.
  const listing = {
    state: heldListing(view.folder) ?? LOADING,
  };

  // Each region's own "ask again", pulled out so that the things below built out
  // of them can be built once. Every one of them is stable — see
  // [`useAsked`](./useAsked) — so anything holding one holds it for the life of
  // the screen.
  const reloadLibrary = library.reload;
  const reloadFolders = folders.reload;
  // Every listing the tree holds, since what changed one row's state can
  // change the folders above it too: a folder is on this device whole only
  // while every file below it is.
  const reloadListing = useCallback(() => reloadListings(), [reloadListings]);
  // One folder's listing and the ones above it, which answer for whether it is
  // here whole.
  const reloadAbove = useCallback(
    (folder: string) => reloadListings(['', ...ancestry(folder)]),
    [reloadListings],
  );

  // One try-again for the screen, wherever it is pressed: the three requests
  // fail together far more often than apart, because what they fail at is the
  // server not being there. A retry that asked only for its own region would
  // leave the Activity panel naming a failure the rest of the screen had
  // recovered from — and the panel has no button of its own for these.
  //
  // What the server is doing on its own is asked again with them, although it
  // is not a region and shows no refusal of its own. The question a page asks
  // as it comes up can fail like any other, and a tab that read that failure as
  // an answer would never again say what is running — with nothing on the
  // screen to press about it, because the sentence a failed work request
  // would have written is deliberately not shown. The three below fail with it
  // far more often than apart, so the button that recovers them is the one that
  // recovers this.
  const retry = useCallback(() => {
    reloadLibrary();
    reloadFolders();
    reloadListing();
    recheckWork();
  }, [reloadLibrary, reloadFolders, reloadListing, recheckWork]);

  // The idle lock, arriving as news. The keys were derived when the server was
  // started or last unlocked, and they live until the interval the server goes
  // unasked for ends them (spec: DK-4) — or until the server stops. The lock
  // happens on the server's own clock and nobody is told, so the only place
  // this window can hear it is the answer it is already asking for while a
  // reader is open: what the server is doing.
  //
  // What it does to the screen is give up the pages this device decrypted and
  // ask the three questions again, and that is the whole of the reporting: the
  // listing and the tree come back refused with the server's own sentence about
  // the Passphrase, in the same place every other refusal is shown, and the
  // header keeps the Library's name because that is not a thing the Master
  // Key kept. Nothing here invents a locked state of its own — the server is
  // the one that knows, and it is asked.
  //
  // `discarded` is a count of the times the pages held on this device have been
  // given up, and it is deliberately not a state of being locked: the reader
  // reads it as one instruction to let go of what it is holding, and goes on
  // showing whatever its next request earns.
  //
  // What it ends is the holding and not the reading. The refused listing ends
  // that, a moment later and by itself: the row the sheet stands on (`sheetRow`
  // below) is a row of the tree, drawn out of the listings' answers, so a
  // folder that cannot be listed has no row open in it and the reader comes
  // off the screen, leaving the refusal standing over the list.
  // The discard does not wait for that answer — plaintext held for the width of
  // a round trip is plaintext held past the key.
  //
  // Which answers are news is [`lockLanded`](./lock). The state it is read
  // against is in a ref rather than in state because nothing on the screen is
  // drawn from it.
  //
  // And the unlock, arriving by the same road the other way: the Passphrase was
  // given in the desktop app's own window (spec: DK-1), and the answer turns
  // back to unlocked. Which answers are that news is
  // [`unlockLanded`](./lock); what it does is ask the three questions again,
  // so the tree and the listing come back without anybody pressing *try
  // again* — and with the listing, the row the reader was on, since the reader
  // is drawn from it and the URL still names the row. What the last press of
  // *unlock* came to goes too: it was about a locked Library.
  const [discarded, setDiscarded] = useState(0);
  const [unlockSaid, setUnlockSaid] = useState<string | null>(null);
  const [unlockRefused, setUnlockRefused] = useState<string | null>(null);
  const held = useRef<LibraryState | null>(null);
  const custody = work.library;
  useEffect(() => {
    if (custody === null) {
      return;
    }
    const before = held.current;
    held.current = custody;
    if (lockLanded(before, custody)) {
      setDiscarded((given) => given + 1);
      retry();
    } else if (unlockLanded(before, custody)) {
      setUnlockSaid(null);
      setUnlockRefused(null);
      retry();
    }
  }, [custody, retry]);

  // A lock heard first as a refusal: the work answer is read once, so the lock
  // above lands as news. When is [`lockUnheard`](./lock). Read against the
  // ref, not `custody`, so that an unlock landing while the refusals still
  // stand does not ask again.
  const refusedLocked = [library.state, folders.state, rootState, listing.state].some(
    (state) => state.status === 'failed' && state.locked === true,
  );
  useEffect(() => {
    if (lockUnheard(held.current, refusedLocked)) {
      recheckWork();
    }
  }, [refusedLocked, recheckWork]);

  // Asking for the unlock: the server has the desktop app put its own window in
  // front, and the Passphrase is typed there — never here. Only one press at a
  // time, in a ref for the reason the refresh's guard is.
  //
  // The page also asks on its own, once for each lock and only while somebody
  // is looking at it; which moments those are is
  // [`UnlockPrompting`](./lock). The ask it sends is this one, so the button
  // shows an automatic ask exactly as it shows a press, and what refused
  // either is what tells the page this server has no window to ask for.
  const [unlockAsking, setUnlockAsking] = useState(false);
  const unlocking = useRef(false);
  const [prompting] = useState(() => new UnlockPrompting());
  const unlock = useCallback(() => {
    if (unlocking.current) {
      return;
    }
    unlocking.current = true;
    setUnlockAsking(true);
    prompting.pressed();
    void askForUnlock({
      ask: () =>
        askToUnlock().catch((thrown: unknown) => {
          prompting.refused(thrown);
          throw thrown;
        }),
      line: setUnlockSaid,
      trouble: setUnlockRefused,
      recheck: recheckWork,
    }).finally(() => {
      unlocking.current = false;
      setUnlockAsking(false);
    });
  }, [prompting, recheckWork]);
  // The one unlock the screen offers, wherever it is offered: the panel's
  // while the work answer says the Library is locked, and a region's wherever
  // its own request was refused by the lock — whether or not that answer has
  // been heard yet, so the two can stand at once and share one press.
  const unlockOffer = useMemo(
    () => ({ asking: unlockAsking, said: unlockSaid, refused: unlockRefused, ask: unlock }),
    [unlockAsking, unlockSaid, unlockRefused, unlock],
  );
  useEffect(() => {
    if (custody !== null && prompting.heard(custody, inFront())) {
      unlock();
    }
  }, [custody, prompting, unlock]);
  // A region refused as locked is the lock heard as well, and behind the
  // explorer host the only way it is heard: the host refuses the work answer as
  // locked like everything else under `/api` (spec: LA-14). Where the work
  // answer says `locked` too, it is the same lock, and asked about once.
  useEffect(() => {
    if (refusedLocked && prompting.heard('locked', inFront())) {
      unlock();
    }
  }, [refusedLocked, prompting, unlock]);
  // Whether the Library has anything this device has not caught up with, which
  // is what draws the sync icon as waiting. Looked at as the page comes up,
  // when the tab comes back in front (below), and every few minutes while it
  // is shown — and never while a catch-up runs or the Library is locked; that
  // schedule is [`RemoteHeadLooks`](./remoteHead). A look moves nothing and
  // takes no key, so it keeps nothing unlocked (spec: DK-4).
  //
  // The two refs it reads are declared here rather than beside what keeps
  // them, because the looks are made here: `looking` is the refresh's guard
  // below, and `stood` is the catalog standing the server last said, kept by
  // the effect that reloads when a catch-up lands.
  const looking = useRef(false);
  const stood = useRef<CatalogState | null>(null);
  const [waiting, setWaiting] = useState(false);
  const [looks] = useState(
    () =>
      new RemoteHeadLooks({
        ask: () => getRemoteHead(),
        heard: (head) => setWaiting(head === 'newer'),
        held: () =>
          looking.current || stood.current === 'catching_up' || held.current === 'locked',
        shown: () => document.visibilityState === 'visible',
      }),
  );
  useEffect(() => looks.start(), [looks]);
  // And as an unlock lands. A look is not made while the Library is locked, and
  // one the page made before it knew was refused and said nothing, so a page
  // opened onto a locked Library would otherwise show the icon plain for up to
  // [`LOOK_EVERY_MS`](./remoteHead) after the unlock — which is exactly when
  // somebody, the Library just opened, looks at it. Its own ref rather than
  // `held`, which the lock's effect above has already moved on by now.
  const custodyLooked = useRef<LibraryState | null>(null);
  useEffect(() => {
    if (custody === null) {
      return;
    }
    const before = custodyLooked.current;
    custodyLooked.current = custody;
    if (unlockLanded(before, custody)) {
      looks.look();
    }
  }, [custody, looks]);

  // Coming back in front also asks what the server is doing, once. A page with
  // no reader open and nothing in flight asks nothing at the interval, so a
  // lock that landed while the person was away would otherwise not be seen —
  // and not asked about — until something else asked. The question takes no
  // key, so it keeps nothing unlocked (spec: DK-4). And it looks at the remote
  // head, which is when somebody would glance at the sync icon.
  useEffect(() => {
    let wasInFront = inFront();
    const looked = () => {
      const now = inFront();
      const cameBack = now && !wasInFront;
      wasInFront = now;
      if (prompting.front(now)) {
        unlock();
      } else if (cameBack) {
        recheckWork();
      }
      if (cameBack) {
        looks.front();
      }
    };
    looked();
    document.addEventListener('visibilitychange', looked);
    window.addEventListener('focus', looked);
    window.addEventListener('blur', looked);
    return () => {
      document.removeEventListener('visibilitychange', looked);
      window.removeEventListener('focus', looked);
      window.removeEventListener('blur', looked);
    };
  }, [prompting, unlock, recheckWork, looks]);

  // What is new in the Library, asked for and never caught up with on a timer.
  // The catalog is what every listing comes out of, so a refresh that advanced
  // it has changed the tree's answer and the folder's alike — which is why both
  // are asked again and neither is edited here.
  //
  // Only ever one at a time, in a ref rather than in the state the button is
  // disabled from: what this guards against is the second of two clicks in one
  // frame, which is a render apart from the first.
  const [refreshing, setRefreshing] = useState(false);
  const [refreshed, setRefreshed] = useState<string | null>(null);
  const [refreshTrouble, setRefreshTrouble] = useState<string | null>(null);
  const refresh = useCallback(() => {
    if (looking.current) {
      return;
    }
    looking.current = true;
    setRefreshing(true);
    // A look already on its way answers about the catalog before this moved it.
    looks.forget();
    void askWhatIsNew({
      ask: refreshCatalog,
      line: setRefreshed,
      trouble: setRefreshTrouble,
      landed: () => setWaiting(false),
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
      recheckWork();
      looking.current = false;
      setRefreshing(false);
    });
  }, [reloadFolders, reloadListing, recheckWork, looks]);

  // Renewing the permission Storage is reached through: one offer for the whole
  // screen, drawn wherever a refusal for a grant that ran out stands (see
  // `ReconnectOffer`).
  //
  // Only on a Library on Google Drive. A Storage on S3 refusing this device's
  // credentials is refused the same way, and nothing a consent page gives
  // would change the machine's own keys.
  const renewable =
    library.state.status === 'ready' && library.state.value.provider === 'drive';
  const [reconnectSaid, setReconnectSaid] = useState<string | null>(null);
  const [reconnectRefused, setReconnectRefused] = useState<string | null>(null);
  const [consentPage, setConsentPage] = useState<string | null>(null);
  const pressing = useRef(false);
  const follow = work.follow;
  const reconnect = useCallback(() => {
    if (pressing.current) {
      return;
    }
    pressing.current = true;
    void askToReconnect({
      ask: startReconnect,
      // A tab of its own: the consent page is Google's, and this screen is
      // still the one to come back to once it is answered.
      // Kept as well, for the link the offer draws in case the browser did
      // not open the tab: this runs after the server answered, not inside
      // the click.
      open: (url) => {
        setConsentPage(url);
        window.open(url, '_blank', 'noopener');
      },
      line: setReconnectSaid,
      trouble: setReconnectRefused,
      follow,
    }).finally(() => {
      pressing.current = false;
    });
  }, [follow]);
  const offer: Offer | null = renewable
    ? {
        reconnect: work.reconnect,
        said: reconnectSaid,
        refused: reconnectRefused,
        consentPage,
        ask: reconnect,
      }
    : null;

  // And the reconnect landing: the permission was renewed and the server has
  // caught the catalog up with it, so every folder's answer may have changed —
  // the refusal they were answered with above all. The notice says what
  // happened, because the refusal the button stood beside goes away by itself
  // and a screen that changed with nothing saying why reads as chance.
  const reconnectState = work.reconnect?.state ?? null;
  const reconnectMessage = work.reconnect?.message ?? null;
  const renewal = useRef<ReconnectState | null>(null);
  useEffect(() => {
    const before = renewal.current;
    renewal.current = reconnectState;
    // The page this window opened is good only while its flow waits: a flow
    // started later — from another window — waits behind a page of its own,
    // and linking to the old one would send the answer to nobody.
    if (reconnectState !== 'waiting') {
      setConsentPage(null);
    }
    if (grantRenewed(before, reconnectState)) {
      setReconnectSaid(null);
      setReconnectRefused(null);
      setNotice(reconnectMessage);
      reloadListing();
      reloadFolders();
    }
  }, [reconnectState, reconnectMessage, reloadListing, reloadFolders]);

  // Files landing is the listing changing, and which rows changed is the
  // server's to say: the folder is asked again as the counts advance rather than
  // the rows being edited here. `done` moving is a file placed — or several at
  // once, the pages one parcel read covered, which turn present together with
  // the one reload; the status changing is the last of them, or the end of
  // trying.
  //
  // Walking into a folder a fill has been working on asks once for the same
  // reason, which is not waste: what the listing arrived with may already be
  // behind what the fill has placed since.
  const fillFolder = fill?.folder ?? null;
  const progress = fill === null ? null : `${fill.folder}:${fill.done}:${fill.status}`;
  useEffect(() => {
    if (progress !== null && fillFolder !== null) {
      reloadAbove(fillFolder);
    }
  }, [progress, fillFolder, reloadAbove]);

  // And the sync and the freeze, whose commits change the rows and can make a
  // folder — see [`useRunReloads`](./runReloads).
  useRunReloads(sync, freeze, reloadListing, reloadFolders);

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
  //
  // A catch-up that landed has also taken in whatever the sync icon said was
  // waiting, so the icon goes back to plain, and a look asked before it is not
  // drawn after it.
  const catalogState = work.catalog?.state ?? null;
  useEffect(() => {
    if (catalogState === null) {
      return;
    }
    const before = stood.current;
    stood.current = catalogState;
    if (catchUpLanded(before, catalogState)) {
      looks.forget();
      setWaiting(false);
      reloadListing();
      reloadFolders();
    }
  }, [catalogState, reloadListing, reloadFolders, looks]);

  // The folders of books whose freeze has not committed — stopped by Storage,
  // waiting their turn behind another, thrown away by a worker that died, still
  // packing, or finished with nothing committed. The Library does not name them,
  // and the freeze naming them is in the answer this page asks for as it comes
  // up, so the tree draws them from that: the rows and the banner are
  // reachable, and the Activity panel's "pack again" is offered over a place
  // somebody can walk into rather than over a name with nothing behind it.
  //
  // Read against the tree's answer, which is why it waits for one: a book that
  // committed before the run ended left a folder the Library holds, and that
  // one is the server's to answer for. And held until the Library names it
  // (see [`heldAfter`](./stranded)), so that a folder does not leave the tree
  // between its freeze committing and the tree being asked again.
  const known = folders.state.status === 'ready' ? folders.state.value.folders : null;
  const [stranded, setStranded] = useState<readonly string[]>([]);
  const naming = useMemo(
    () => (known === null ? [] : strandedFolders(freeze, known)),
    [freeze, known],
  );
  useEffect(() => {
    if (known !== null) {
      setStranded((held) => heldAfter(held, naming, known));
    }
  }, [naming, known]);

  // The root's listing, which the tree always holds: it says which top-level
  // folders have a folder of this device chosen for them — those the add flow
  // asks for none (see [`folderToChoose`](./mapping)) — and which of them have
  // nothing under them yet, which are drawn to add into.
  const reloadRoot = useCallback(() => reloadListings(['']), [reloadListings]);
  const rootKnown: Listing | null = rootState.status === 'ready' ? rootState.value : null;
  // A folder's listing as the tree holds it, or asked for where it holds none.
  const listingOf = useCallback(
    (path: string): Promise<Listing> => {
      const held = heldListing(path);
      return held?.status === 'ready' ? Promise.resolve(held.value) : getListing(path);
    },
    [heldListing],
  );

  // The paths prepared with "New folder…" (see [`prospective`](./prospective)), each
  // let go of the moment the Library names it — the tree's answer, or a
  // listing's.
  const [prepared, setPrepared] = useState<readonly string[]>([]);
  useEffect(() => {
    if (known !== null) {
      setPrepared((held) => preparedAfter(held, (path) => known.includes(path)));
    }
  }, [known]);
  // The folder on the screen, as its listing last answered.
  const listed = listing.state.status === 'ready' ? listing.state.value : null;
  // And every listing the tree holds, any of which may be the first to name a
  // prepared path.
  useEffect(() => {
    const held = shown
      .map(heldListing)
      .flatMap((asked) => (asked?.status === 'ready' ? [asked.value] : []));
    if (held.length > 0) {
      setPrepared((before) =>
        preparedAfter(before, (path) =>
          held.some(
            (one) =>
              one.folders.some((folder) => folder.path === path) ||
              (one.held && one.path === path),
          ),
        ),
      );
    }
  }, [shown, heldListing]);
  // "Forget this folder" on a prepared path's row: that path and the ones
  // prepared under it are let go of, and nothing is sent (see
  // [`forgotten`](./prospective)). Where the screen is on one of them — the
  // tree offers it on the row of the folder the list is showing — the list
  // moves to the folder it was prepared in, rather than standing on a path
  // nothing draws any longer and the Library holds nothing at.
  const forget = useCallback(
    (path: string) => {
      setPrepared((held) => forgotten(held, path));
      if (view.folder === path || view.folder.startsWith(`${path}/`)) {
        moveTo({ folder: parentOf(path), open: null }, true);
      }
    },
    [view.folder, moveTo],
  );
  // Every folder drawn as not in the Library yet (see
  // [`prospectiveFolders`](./prospective)).
  const prospective = useMemo(
    () => prospectiveFolders(prepared, rootKnown, known),
    [prepared, rootKnown, known],
  );

  // A folder just prepared is brought into view, wherever it was prepared
  // from: "New folder…" on the row of a closed folder would otherwise prepare
  // a folder nobody can see. Only the ones not drawn before, so that a branch
  // somebody closed stays closed.
  const seen = useRef<ReadonlySet<string>>(new Set(prospective));
  useEffect(() => {
    const fresh = prospective.filter((path) => !seen.current.has(path));
    seen.current = new Set(prospective);
    if (fresh.length === 0) {
      return;
    }
    setTree((held) => {
      const above = fresh.flatMap((path) => ancestry(parentOf(path)));
      if (above.every((folder) => held.expanded.has(folder))) {
        return held;
      }
      return { ...held, expanded: new Set([...held.expanded, ...above]) };
    });
  }, [prospective]);

  // "New folder…": the name asked for in a dialog over the folder chosen, and
  // judged before anything is drawn (see [`prepare`](./prospective)). `null`
  // while nothing is being asked.
  const [namingIn, setNamingIn] = useState<string | null>(null);
  // The folder just created there, which the focus moves to once its row is
  // drawn (below), so that the next drop or "New folder…" is about it.
  const [focusPrepared, setFocusPrepared] = useState<string | null>(null);
  // Counts each time the dialog is opened or closed, so that a name still being
  // judged when the dialog was let go of — Cancel or Escape while the parent's
  // listing is out — prepares nothing, as the dialog says it will not.
  const namings = useRef(0);
  const openNaming = useCallback((parent: string) => {
    namings.current += 1;
    setNamingIn(parent);
  }, []);
  const closeNaming = useCallback(() => {
    namings.current += 1;
    setNamingIn(null);
  }, []);
  const prepareNamed = useCallback(
    async (typed: string): Promise<string | null> => {
      if (namingIn === null) {
        return null;
      }
      const asked = namings.current;
      const answer = await prepare({
        parent: namingIn,
        typed,
        prospective,
        list: listingOf,
      });
      if (namings.current !== asked) {
        return null;
      }
      if ('refused' in answer) {
        return answer.refused;
      }
      setPrepared((held) => (held.includes(answer.path) ? held : [...held, answer.path]));
      setFocusPrepared(answer.path);
      closeNaming();
      return null;
    },
    [namingIn, prospective, listingOf, closeNaming],
  );

  // Choosing a folder on this device for part of the Library (see
  // [`ChooseFolder`](./ChooseFolder)): from the banner over a folder none is
  // chosen for — the top-level folder it names, or the whole Library — and
  // from inside the add flow, which waits on the answer. A choice recorded is
  // what the listings read next, so they are asked again, which is what takes
  // the banner away and gives the rows their state. What the server said of it
  // is the notice, because it may name a folder the same part of the Library
  // pointed at before, and everything under that one has just left this
  // device's reach.
  const [choosing, setChoosing] = useState<{
    prefix: string | null;
    answer: (chosen: Mapped | null) => void;
  } | null>(null);
  const chooseFolder = useCallback(
    (prefix: string | null) =>
      new Promise<Mapped | null>((resolve) => {
        setChoosing({
          prefix,
          answer: (chosen) => {
            setChoosing(null);
            if (chosen !== null) {
              setNotice(chosen.message);
              reloadListing();
              reloadRoot();
            }
            resolve(chosen);
          },
        });
      }),
    [reloadListing, reloadRoot],
  );
  const chosenFolder = useCallback((answer: Mapped) => choosing?.answer(answer), [choosing]);
  const closeChoosing = useCallback(() => choosing?.answer(null), [choosing]);
  // The banner's button: the same question, with nothing waiting on it.
  const openChoosing = useCallback(
    (prefix: string | null) => void chooseFolder(prefix),
    [chooseFolder],
  );

  // The files this page is adding, drawn as rows at once (see
  // [`IncomingFile`](./tree)): each add's, from Add until the folder it lands
  // in has been listed again after its request ended — `settled` holds the
  // listing on hand as it ended, and a different one is the answer that names
  // what landed.
  const [incoming, setIncoming] = useState<
    readonly { id: number; landing: string; files: readonly IncomingFile[]; settled: Asked<Listing> | undefined | null }[]
  >([]);
  const adds = useRef(0);
  const listingNow = useRef(heldListing);
  listingNow.current = heldListing;
  useEffect(() => {
    setIncoming((held) => {
      const left = held.filter(
        (one) => one.settled === null || heldListing(one.landing) === one.settled,
      );
      return left.length === held.length ? held : left;
    });
  }, [heldListing]);
  const incomingFiles = useMemo(() => incoming.flatMap((one) => one.files), [incoming]);

  // Whether the folder on the screen is a stranded one. It says what the list
  // draws over a path the Library holds nothing at, and nothing about how a
  // drop into it is added: that follows what was dropped.
  const strandedHere = stranded.includes(view.folder);

  // The confirmation every drop is answered with before anything is sent, and
  // where its answer goes. `null` while nothing is being asked.
  const [asking, setAsking] = useState<{
    landing: string;
    summary: DropSummary;
    files: readonly Added[];
    held: HeldFiles;
    destinations: Destination[];
    notTaken: readonly NotTaken[];
    systemFiles: readonly SystemFile[];
    holding: (folders: readonly string[]) => Promise<HeldFiles>;
    answer: (choice: Choice, numbering: Numbering, edits: Edits) => void;
  } | null>(null);
  const askAboutDrop = useCallback(
    (
      landing: string,
      summary: DropSummary,
      files: readonly Added[],
      held: HeldFiles,
      destinations: Destination[],
      notTaken: readonly NotTaken[],
      systemFiles: readonly SystemFile[],
      holding: (folders: readonly string[]) => Promise<HeldFiles>,
    ) =>
      new Promise<Answer>((resolve) => {
        setAsking({
          landing,
          summary,
          files,
          held,
          destinations,
          notTaken,
          systemFiles,
          holding,
          answer: (choice, numbering, edits) => {
            setAsking(null);
            resolve(choice === 'cancel' ? 'cancel' : { choice, numbering, edits });
          },
        });
      }),
    [],
  );
  const choose = useCallback(
    (choice: Choice, numbering: Numbering, edits: Edits) =>
      asking?.answer(choice, numbering, edits),
    [asking],
  );

  // "Pack this folder…": what a freeze of the folder would pack is asked first
  // and shown, and only Pack arms it — after which it is followed exactly as a
  // drop's freeze is, through the work answer the Activity panel reads. Which
  // of those steps the press came to is [`askToPack`](./packFolder); a refusal
  // of either request is a notice of this page's own, since it answers a
  // gesture made on the folder on the screen.
  const [packAsking, setPackAsking] = useState<{
    question: PackQuestion;
    answer: (pack: boolean) => void;
  } | null>(null);
  const askAboutPacking = useCallback(
    (question: PackQuestion) =>
      new Promise<boolean>((resolve) => {
        setPackAsking({
          question,
          answer: (pack) => {
            setPackAsking(null);
            resolve(pack);
          },
        });
      }),
    [],
  );
  const choosePacking = useCallback(
    (pack: boolean) => packAsking?.answer(pack),
    [packAsking],
  );
  // A second press while the first is still being counted is dropped: the
  // first one's question is on its way, and a second would replace it on the
  // screen and leave the first press unanswered.
  const packPressed = useRef(false);
  const packFolder = useCallback(
    (folder: string) => {
      if (packPressed.current) {
        return;
      }
      packPressed.current = true;
      void askToPack({
        folder,
        preview: (path) => previewFreeze(path),
        ask: askAboutPacking,
        start: async (path) => {
          await startFreeze(path);
          follow();
        },
        refuse: (cause) => setNotice(`${folder} was not packed — ${said(cause)}`),
      }).finally(() => {
        packPressed.current = false;
      });
    },
    [askAboutPacking, follow],
  );

  // "Delete…" on a row: what deleting that file or folder would take out of the
  // Library is asked first and shown, and only Delete arms it — after which
  // it is followed through the work answer, and what it came to is the
  // server's to log once it is over (spec: LA-12). Which of those steps the
  // press came to is [`askToDelete`](./deleteEntries); a refusal of either
  // request is a notice of this page's own, since no deletion ran to log it.
  const [deleteAsking, setDeleteAsking] = useState<{
    question: DeleteQuestion;
    answer: (remove: boolean) => void;
  } | null>(null);
  const askAboutDeleting = useCallback(
    (question: DeleteQuestion) =>
      new Promise<boolean>((resolve) => {
        setDeleteAsking({
          question,
          answer: (remove) => {
            setDeleteAsking(null);
            resolve(remove);
          },
        });
      }),
    [],
  );
  const chooseDeleting = useCallback(
    (remove: boolean) => deleteAsking?.answer(remove),
    [deleteAsking],
  );
  // A second press while the first is still being counted is dropped, for the
  // reason a second "Pack this folder…" is. Whether this press was taken is
  // returned, so a *try again* dropped this way leaves its notice standing to be
  // pressed once that count is over, rather than clearing it for nothing.
  const deletePressed = useRef(false);
  const deleteEntries = useCallback(
    (target: DeleteTarget): boolean => {
      if (deletePressed.current) {
        return false;
      }
      deletePressed.current = true;
      void askToDelete({
        target,
        preview: (asked) => previewDelete(asked),
        ask: askAboutDeleting,
        start: async (asked) => {
          await startDelete(asked);
          // The rows it named are leaving; a selection left standing on them
          // would offer them a second time.
          setSelection(NOTHING_SELECTED);
          follow();
        },
        refuse: (cause) => showNotice(refusedNotice(target, cause, said(cause))),
      }).finally(() => {
        deletePressed.current = false;
      });
      return true;
    },
    [askAboutDeleting, follow],
  );
  // The end of a deletion: the rows and the tree are asked again, since what
  // left the Library leaves them. What it came to is the server's log entry.
  const deleted = deletion === null ? null : `${deletion.run}:${deletion.status}`;
  // The run itself is read through a ref, so the effect below is keyed on the
  // run and its status alone: every poll brings a fresh object for the same
  // ending, and an effect keyed on that would reload the folder on each one.
  //
  // The root's listing too, where the screen is not at the root: it is read
  // once there and not again, and it is what names a mapped top-level folder
  // whose last Entry just left — an empty mapped prefix now, which the tree
  // would otherwise stop drawing until the root was visited. At the root the
  // listing asked again above is that one.
  //
  // And where the deletion was of the folder on the screen or of one above
  // it, the list is moved up once the tree has answered again (below), since
  // that answer is what says which folder above the Library still holds.
  const lastDeletion = useRef(deletion);
  lastDeletion.current = deletion;
  const shownFolder = useRef(view.folder);
  shownFolder.current = view.folder;
  const leaving = useRef(false);
  useEffect(() => {
    const ended = lastDeletion.current;
    if (deleted === null || ended === null || ended.status === 'deleting') {
      return;
    }
    reloadListing();
    reloadFolders();
    const on = shownFolder.current;
    if (on !== '') {
      reloadRoot();
    }
    if (ended.folders.some((folder) => on === folder || on.startsWith(`${folder}/`))) {
      leaving.current = true;
    }
  }, [deleted, reloadListing, reloadFolders, reloadRoot]);
  // The tree's answer after such a deletion: the screen moves to the nearest
  // folder at or above it that the Library still holds — the root where none
  // does — rather than standing on a path it holds nothing at. A deletion
  // refused for a Pack can leave the folder held, and then nothing moves.
  useEffect(() => {
    if (!leaving.current || known === null) {
      return;
    }
    leaving.current = false;
    let above = shownFolder.current;
    while (above !== '' && !known.includes(above)) {
      above = parentOf(above);
    }
    if (above !== shownFolder.current) {
      moveTo({ folder: above, open: null }, true);
    }
  }, [known, moveTo]);

  // The Library, which a drop too large for this route names in the command
  // that can carry it in instead.
  const knownLibrary = library.state.status === 'ready' ? library.state.value : null;

  // Files picked with the header's **+** or dropped onto the tree are added to
  // the folder they were handed over for: the focused folder or the one a focused file stands
  // in, or the row the drop was let go over (see [`landingOf`](./destination)).
  // They are drawn on the tree at Add, in the uploading state, and the listing
  // is asked for again as soon as they land: they are in the folder from that
  // moment, and the folder is what knows they are there until the flow behind
  // them commits.
  //
  // A refusal about one part is a notice in the Activity panel, because it is
  // about a file on this screen; a refusal about the whole drop is said the
  // same way, since it is the same sentence about all of them at once.
  //
  // Which of those the drop came to, and what is asked again because of it, is
  // [`askToAdd`](./dropped) — the guards below are this screen's, since they
  // are about state only it holds.
  //
  // Before any of it, a folder on this device where adding needs one and none
  // is chosen — for the folder's top-level folder or for the whole Library
  // (spec: EP-9, see [`folderToChoose`](./mapping)) — asked inside the flow,
  // and recorded before the confirmation comes up. Cancelling it adds nothing,
  // quietly, as Cancel on the confirmation does.
  //
  // Then the confirmation, which is [`chooseAndAdd`](./packChoice): a drop that
  // cannot be sent at all is refused without a question, and every other one
  // is asked about — where it lands, and for a drop holding a folder whether
  // it goes as One Pack or as separate files. Where it lands is read against
  // that folder's listing, the tree's where it holds one and asked for where
  // not, and asked afresh after a folder on this device was chosen for it.
  //
  // What the desktop app met in the drop and did not take is said wherever the
  // drop is answered (spec: LA-26): in the confirmation, and where the drop is
  // answered without one — refused for its budgets, let go over a path the
  // Library holds nothing at, or where it lands could not be read — after the
  // reason nothing was added.
  const add = useCallback(
    (target: string, { files, notTaken, systemFiles }: Drop) => {
      const notAdded = (line: string) => setNotice(withNotTaken(line, notTaken));
      if (files.length === 0) {
        setAdding(null);
        // An empty folder, or something that was never a file — a selection of
        // text, an image dragged out of another page. The gesture was made and
        // there is nothing to show for it, and a screen that does not react at
        // all is one a person reads as broken rather than as answered. A drop
        // of nothing but what the system keeps says that instead, quietly as
        // the confirmation would (spec: EP-17).
        const kept = systemFilesLine(systemFiles);
        setNotice(
          kept === null
            ? 'nothing was added — there were no files to add'
            : `nothing was added — ${kept}`,
        );
        return;
      }
      const send = (sending: readonly Added[], freeze: boolean) => {
        setAdding(addingLine(sending.length, target));
        setSent(null);
        // The rows, at once, with every folder above them opened so they show.
        adds.current += 1;
        const id = adds.current;
        const prefix = target === '' ? '' : `${target}/`;
        setIncoming((held) => [
          ...held,
          {
            id,
            landing: target,
            files: sending.map((one) => ({ path: `${prefix}${one.path}`, size: one.size, packed: freeze })),
            settled: null,
          },
        ]);
        setTree((held) => {
          const above = target === '' ? [] : [...ancestry(target), target];
          return above.every((folder) => held.expanded.has(folder))
            ? held
            : { ...held, expanded: new Set([...held.expanded, ...above]) };
        });
        // How much of the request has gone, on the same line, as the browser
        // says it — paced, since it says it far more often than a line can be
        // read.
        const onProgress = paced((gone, total) => {
          setAdding(addingLine(sending.length, target, { sent: gone, total }));
          setSent(total > 0 ? gone / total : null);
        });
        return askToAdd({
          ask: () => addFiles(target, sending, { freeze, onProgress }),
          notice: setNotice,
          reload: reloadListing,
          follow: work.follow,
          library: knownLibrary,
        }).finally(() => {
          // Let go of once the folder is listed again — by the reload just
          // asked for — so the rows hand over to the listing's own.
          const settled = listingNow.current(target);
          setIncoming((held) => held.map((one) => (one.id === id ? { ...one, settled } : one)));
        });
      };
      const unlisted = (refused: unknown): HeldFiles => {
        console.warn('the explorer could not list where a drop lands', refused);
        return NOTHING_HELD;
      };
      const confirm = async (where: Listing) => {
        await chooseAndAdd({
          files,
          existing: where.folders.map((folder) => folder.name),
          // What the confirmation names as replaced or refused. A listing that
          // could not be had leaves it saying what it always said — the rule,
          // without the names — rather than holding the drop up: the sending
          // itself refuses a Pack's file part by part either way.
          held: (dropped) =>
            heldUnder(where, dropped, (path) => getListing(path)).catch(unlisted),
          ask: (summary, asked, held) => {
            // The line saying the drop is being read stands down while the
            // confirmation is up: the reading is over, and nothing is being
            // sent.
            setAdding(null);
            const destinations = destinationsOf(
              target,
              summary,
              namedBy(known, where),
              onDiskIn(where),
            );
            // Asked again of the folders a corrected volume name sends files
            // into, which no numbering would have.
            const holding = (folders: readonly string[]) =>
              heldIn(where, folders, (path) => getListing(path)).catch(unlisted);
            return askAboutDrop(target, summary, asked, held, destinations, notTaken, systemFiles, holding);
          },
          send,
          refuse: (overdrawn) => notAdded(overBudgetLine(overdrawn, knownLibrary)),
        });
      };
      const proceed = async () => {
        const where = await listingOf(target);
        if (!where.mapped && !(where.held || stranded.includes(target) || prospective.includes(target))) {
          // A path the Library holds nothing at: no folder of this device
          // would change that, and nothing is asked.
          notAdded(unmappedLine('add', false));
          return;
        }
        const asked = where.mapped ? null : folderToChoose(target, await listingOf(''));
        if (asked === null) {
          await confirm(where);
          return;
        }
        setAdding(null);
        const chosen = await chooseFolder(asked.prefix);
        if (chosen === null) {
          return;
        }
        await confirm(await getListing(target));
      };
      const unread = (refused: unknown) =>
        notAdded(`nothing was added — where those files go could not be read: ${said(refused)}`);
      void proceed().catch(unread).finally(() => setAdding(null));
    },
    [listingOf, known, stranded, prospective, askAboutDrop, work, reloadListing, knownLibrary, chooseFolder],
  );

  // The word the drop itself gets, before there is anything to send. A browser
  // hands a dropped folder over as something to walk, one batch of children at
  // a time, and a nested folder of several hundred pages is seconds of that
  // with nothing on the screen changing — the files land one folder down, so
  // the folder being looked at gains no row to say they are there. Replaced by
  // the line naming the count as soon as the walk has one.
  const collecting = useCallback(() => setAdding(collectingLine()), []);

  // And the end that walk can come to instead of files: a browser that refused
  // to read a dropped folder's children, or a desktop app that took nothing of
  // what was dropped — each said as what it is (see
  // [`nothingAddedLine`](./drop)). Nothing was sent, so nothing else on the
  // screen is going to change — and the line the drop put up would stand
  // saying the drop was being read for as long as the tab was open.
  const unreadable = useCallback((cause: unknown) => {
    setAdding(null);
    setNotice(nothingAddedLine(cause));
  }, []);

  // Which run the rows of a folder read, which is not always the one on
  // record. A fill Storage stopped keeps its entry and its Retry after the next
  // folder has taken the record from it, and the `failed` and `declined` chips
  // over these rows are the other half of what that entry says — so they are
  // owed for as long as the server keeps the run.
  const fillOf = useCallback(
    (folder: string) => fillOfFolder(fill, fill?.displaced ?? [], folder),
    [fill],
  );

  // Everywhere that is not the list or a row of the tree. A browser's own answer
  // to a file dropped on a page is to leave the page and show the file, and on a
  // screen that has just taught somebody that dropping files here adds them, a
  // miss — the tree below its rows, the Activity panel, the reader standing over the
  // list — would take the explorer away and whatever was open in it. So the page
  // refuses the drops nothing took. The list's and the rows' own handlers run
  // first and do the adding; this is only what happens to the ones that reach
  // nothing.
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
  const catalogSaid = catalogLine(work.catalog, renewable);
  // Whether that sentence is about a permission that ran out, which is what the
  // reconnect is drawn after it for.
  const catalogRanOut =
    offer !== null &&
    work.catalog?.state === 'behind' &&
    ranOut(work.catalog.stopped);

  // The row the reader's sheet is open on, by its key, or `null` while it is
  // closed.
  const openKey = sheetKey(view);

  // The reader and the tree agree: the row the sheet is open on is the focused
  // one, with every folder above it open — so a sheet restored from the URL
  // stands on a row of the tree, and closing it leaves the focus where the
  // reading stopped, the tree scrolled to it.
  useEffect(() => {
    if (sheetOpen(view)) {
      setTree((held) => restored(held, view));
    }
  }, [view]);

  // The rows of the tree, top to bottom.
  const rows = useMemo(
    () =>
      treeRows({
        listing: heldListing,
        expanded: tree.expanded,
        prospective,
        stranded,
        incoming: incomingFiles,
      }),
    [heldListing, tree.expanded, prospective, stranded, incomingFiles],
  );

  // The focus moved or a folder opened or closed: the URL follows the folder
  // the focus is in, without a history entry of its own (see `contextOf`) —
  // and, while the reader's sheet is open, the row it is on, which is the
  // sheet following the focus: walking the page rows of a book is paging.
  const sheeted = sheetOpen(view);
  const changeTree = useCallback(
    (next: TreeState) => {
      setTree(next);
      const folder = contextOf(next);
      if (sheeted && next.focus !== null) {
        go(sheetOn(folder, keyed(next.focus)), true);
      } else if (folder !== view.folder || sheeted) {
        go({ folder, open: null }, true);
      }
    },
    [view.folder, sheeted, go],
  );

  // A folder row opened: the reader's sheet, on the folder's cover, with the
  // focus left on the folder's row and the folder left open or closed as it
  // was — and where it has no image directly in it, or is not on this device,
  // the sentence saying why nothing opened. The same test as the details
  // pane's Open ([`opensInReader`](./details)), asked of the folder's own
  // listing: the sheet opens only where it has a picture to start on. Once
  // open, it follows the focus onto such a folder all the same, and says there
  // that it holds none.
  const openFolder = useCallback(
    (path: string) => {
      const name = path.slice(path.lastIndexOf('/') + 1);
      const show = (where: Listing) => {
        if (!where.mapped) {
          setNotice(unmappedLine('open', true));
          return;
        }
        if (firstImage(where) === null) {
          setNotice(`${name} holds nothing the reader can show — open a folder of pages, or a file`);
          return;
        }
        // The folder the screen's actions are about stays the one it was:
        // the folder itself where it is open, the one it stands in where not.
        const folder = tree.expanded.has(path) ? path : parentOf(path);
        go(sheetOn(folder, { kind: 'folder', path }));
      };
      listingOf(path).then(show, (refused: unknown) =>
        setNotice(`${name} was not opened — ${said(refused)}`),
      );
    },
    [listingOf, go, tree.expanded],
  );

  // The openers (see `opened`) shared by the tree's own gestures and the
  // details pane's Open, so that the two are one gesture.
  const openFile = useCallback(
    (path: string) => go(sheetOn(parentOf(path), { kind: 'file', path })),
    [go],
  );
  const unsupported = useCallback(
    (file: ListedFile) => setNotice(`${file.name} — preview of this format is not supported yet`),
    [],
  );
  // The sentence the server would have answered with, said here because
  // neither gesture becomes a request: what each says is
  // [`unmappedLine`](./unmapped), and `held` is which of the two reasons it
  // gives — no folder of this device chosen (spec: EP-9), or the folder the
  // Library does not have.
  const unmapped = useCallback(
    (tried: Tried, held: boolean) => setNotice(unmappedLine(tried, held)),
    [],
  );

  // What the details pane describes: the rows selected — the focused row
  // alone where nothing else is (see [`selectedRows`](./selection)).
  const selected = useMemo(
    () => selectedRows(rows, { focus: tree.focus, selection }),
    [rows, tree.focus, selection],
  );
  // The listing of a folder where it is on hand, which says whether the reader
  // can show the folder: the actions menu offers Open only there.
  const readyListing = useCallback(
    (folder: string) => {
      const held = heldListing(folder);
      return held?.status === 'ready' ? held.value : null;
    },
    [heldListing],
  );
  const shownOf = useCallback(
    (row: FileRow) => rowFill(row.file, row.in.path, fillOf(row.in.path)),
    [fillOf],
  );
  const uploadStoppedOf = useCallback(
    (file: ListedFile) => uploadStopped(file, sync, freeze),
    [sync, freeze],
  );
  // The tree's element, which Open from the details pane hands the focus back
  // to: the sheet follows the tree's focus, and the arrows that move it are
  // the tree's keys.
  const treeArea = useRef<HTMLDivElement>(null);
  const openFromPane = useCallback(
    (row: FolderRow | FileRow) => {
      opened(row, {
        onOpenFolder: openFolder,
        onOpenFile: openFile,
        onUnsupported: unsupported,
        onUnmapped: unmapped,
      });
      treeArea.current?.focus();
    },
    [openFolder, openFile, unsupported, unmapped],
  );
  // A folder created with "New folder…" is focused once its row is drawn: the
  // rows above it open first (see where `prospective` is followed), and the
  // listings they ask for may still be out when the dialog closes.
  useEffect(() => {
    if (focusPrepared === null) {
      return;
    }
    const key = folderKey(focusPrepared);
    if (!rows.some((row) => row.key === key)) {
      return;
    }
    setFocusPrepared(null);
    changeTree({ ...tree, focus: key });
    setSelection({ keys: new Set(), anchor: key });
    treeArea.current?.focus();
  }, [focusPrepared, rows, tree, changeTree]);

  // The header's Add files menu let go of by `Escape`: the keys go back to the
  // tree, as they do out of any menu opened over it.
  const keysBack = useCallback(() => treeArea.current?.focus(), []);

  // Closing the sheet leaves the focus and the folder where they are.
  const closeSheet = useCallback(() => go({ folder: view.folder, open: null }), [go, view.folder]);
  // A picture the sheet fetched is on this device now: the listing it stands
  // in, and the ones above it, which answer for whether it is here whole.
  const fetchedInto = useCallback((path: string) => reloadAbove(parentOf(path)), [reloadAbove]);

  // What the reader's sheet stands on: the row the URL has it open on, once
  // that row is drawn — a sheet restored from the URL waits for the listings
  // above it — and where it stands among the rows beside it.
  const sheetRow = useMemo(
    () =>
      openKey === null
        ? undefined
        : rows.find((row): row is FolderRow | FileRow => row.kind !== 'note' && row.key === openKey),
    [rows, openKey],
  );
  const counter = useMemo(() => (openKey === null ? null : counterOf(rows, openKey)), [rows, openKey]);
  // Whether a dialog stands over the screen, whose `Escape` is its own and not
  // the sheet's.
  const dialogUp =
    choosing !== null || namingIn !== null || asking !== null || packAsking !== null || deleteAsking !== null;

  // What the Activity panel lists: what is moving now, and what is over — the
  // server's log of its finished work beside this page's own notices.
  const running = useMemo(
    () =>
      inProgress({
        adding,
        sent: adding === null ? null : sent,
        turn,
        freeze,
        sync,
        deletion,
        fill,
        fetching,
      }),
    [adding, sent, turn, freeze, sync, deletion, fill, fetching],
  );
  const entries = useMemo(
    () => offeredNow(earlier(work.log, notices, custody === 'locked'), { fill, sync, freeze }),
    [work.log, notices, custody, fill, sync, freeze],
  );
  // One entry's Retry: the second attempt at the work that entry is about. A deletion's goes through the
  // confirmation again, since what it would take out of the Library may have
  // moved; a notice of this page's own whose *try again* was taken is settled,
  // and one dropped because a count is still out is left to be pressed again.
  const retryFill = work.retry;
  const retrySync = work.retrySync;
  const retryFreeze = work.retryFreeze;
  const retryEntry = useCallback(
    (entry: Entry) => {
      const again = entry.retry;
      if (again === null) {
        return;
      }
      switch (again.flow) {
        case 'fill':
          retryFill(again.folder);
          return;
        case 'freeze':
          retryFreeze(again.folder);
          return;
        case 'sync':
          retrySync();
          return;
        case 'delete':
          if (deleteEntries(again.target)) {
            setNotices((held) =>
              held.map((notice) =>
                `page:${notice.id}` === entry.key ? { ...notice, settled: true } : notice,
              ),
            );
          }
          return;
      }
    },
    [retryFill, retryFreeze, retrySync, deleteEntries],
  );

  // A hash naming a row the folder it stands in does not hold — the Entry is
  // gone, or the URL was written by hand. The tree is the answer, and the URL
  // is put back in step with it.
  //
  // Against that folder's own listing and no other. The back button, the
  // forward button, and a hash typed into the address bar can move the folder
  // and the open row in one step, and until the listing for the new one lands
  // the rows on hand are still the folder the screen just left — which holds
  // no such row for the plain reason that the row is not in it. Judged
  // against those, every such move would close the reader on the way into it
  // and write the correction over the very history entry it arrived by.
  const sheetPath = view.open ?? view.cover;
  const parentAsked = sheetPath === undefined ? undefined : heldListing(parentOf(sheetPath));
  const stale =
    parentAsked?.status === 'ready' &&
    (view.open !== null
      ? !parentAsked.value.files.some((file) => file.path === view.open)
      : view.cover !== undefined &&
        !parentAsked.value.folders.some((one) => one.path === view.cover) &&
        !prospective.includes(view.cover) &&
        !stranded.includes(view.cover));
  const folder = view.folder;
  useEffect(() => {
    if (stale) {
      go({ folder, open: null }, true);
    }
  }, [stale, folder, go]);

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      <Header
        library={library.state}
        focus={tree.focus}
        onAdd={add}
        onCollecting={collecting}
        onUnreadable={unreadable}
        onKeysBack={keysBack}
      />
      <div style={{ flex: 1, display: 'flex', minHeight: 0 }}>
        <main style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
          {/* Why the rows below may not be the Library. Every listing comes out
              of this device's catalog, and the catalog holds what this device
              has replayed — so a device fresh from `join` whose catch-up did
              not land shows an empty tree, and shows it in exactly the
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
              {catalogRanOut && offer !== null && (
                <>
                  {' — '}
                  <ReconnectOffer offer={offer} />
                </>
              )}
            </p>
          )}
          {/* The folder the focus is in, where its own listing was refused
              and the root's was not: the tree stands, and the refusal over it
              is about the one folder. */}
          {view.folder !== '' && listing.state.status === 'failed' && rootState.status === 'ready' && (
            <Region state={listing.state} onRetry={retry} reconnect={offer} unlock={unlockOffer}>
              {() => null}
            </Region>
          )}
          {/* The Library's folders, where they were refused and the root's
              listing was not. The tree draws without them, but the stranded
              folders, the prepared paths the Library has come to name, and
              the way out of a folder a deletion took away are all read off
              that answer, so its refusal is said and offered again rather
              than left to look like a Library with none of those. */}
          {folders.state.status === 'failed' && rootState.status === 'ready' && (
            <Region state={folders.state} onRetry={retry} reconnect={offer} unlock={unlockOffer}>
              {() => null}
            </Region>
          )}
          <Region state={rootState} onRetry={retry} reconnect={offer} unlock={unlockOffer}>
            {() => (
              // The tree, and the details pane on its right following the
              // focused row — and the reader's sheet over both but a strip of
              // the tree on the left, covering the tree's height and no more:
              // the catalog's sentence above it and the Activity panel below,
              // which says what the sheet is fetching, stay uncovered, and
              // opening the panel shrinks all three rather than covering them.
              <div style={{ flex: 1, display: 'flex', minHeight: 0, position: 'relative' }}>
                <TreeTable
                  rows={rows}
                  state={tree}
                  onChange={changeTree}
                  selection={selection}
                  onSelect={setSelection}
                  listingOf={readyListing}
                  context={listed}
                  prospectiveHere={prospective.includes(view.folder)}
                  prepared={prepared}
                  fillOf={fillOf}
                  freeze={freeze}
                  stranded={strandedHere}
                  // The stranded folders are read off the Library's folders, and
                  // `stranded` is false until that answer lands whether or not
                  // this folder is one of them.
                  strandedKnown={known !== null}
                  onOpenFolder={openFolder}
                  onOpenFile={openFile}
                  onUnsupported={unsupported}
                  onAdd={add}
                  onCollecting={collecting}
                  onUnreadable={unreadable}
                  onUnmapped={unmapped}
                  onMap={openChoosing}
                  onPack={packFolder}
                  onDelete={deleteEntries}
                  onNewFolder={openNaming}
                  onForget={forget}
                  onAskAgain={(folder) => reloadListings([folder])}
                  areaRef={treeArea}
                />
                <DetailsPane
                  selected={selected}
                  listingOf={heldListing}
                  shownOf={shownOf}
                  uploadStoppedOf={uploadStoppedOf}
                  onOpen={openFromPane}
                  onDelete={deleteEntries}
                />
                {sheetRow !== undefined && (
                  <ReaderView
                    row={sheetRow}
                    asked={sheetRow.kind === 'folder' ? heldListing(sheetRow.path) : undefined}
                    counter={counter}
                    discarded={discarded}
                    escapable={!dialogUp}
                    onClose={closeSheet}
                    onFetching={setFetching}
                    onFetched={fetchedInto}
                    reconnect={offer}
                  />
                )}
              </div>
            )}
          </Region>
        </main>
      </div>
      {choosing !== null && (
        <ChooseFolder prefix={choosing.prefix} onChosen={chosenFolder} onClose={closeChoosing} />
      )}
      {namingIn !== null && (
        <NewFolderDialog parent={namingIn} onPrepare={prepareNamed} onClose={closeNaming} />
      )}
      {asking !== null && (
        <DropConfirm
          landing={asking.landing}
          summary={asking.summary}
          files={asking.files}
          held={asking.held}
          destinations={asking.destinations}
          notTaken={asking.notTaken}
          systemFiles={asking.systemFiles}
          holding={asking.holding}
          onChoose={choose}
        />
      )}
      {packAsking !== null && (
        <PackFolderConfirm question={packAsking.question} onChoose={choosePacking} />
      )}
      {deleteAsking !== null && (
        <DeleteConfirm
          question={deleteAsking.question}
          // A deletion holding the turn is said by the question itself, from
          // the preview's own word that one is running already.
          startsAfter={turn === 'delete' ? null : heldBy(turn)}
          onChoose={chooseDeleting}
        />
      )}
      <ActivityPanel
        kept={work.kept}
        running={running}
        entries={entries}
        trouble={work.trouble}
        onRetry={retryEntry}
        refresh={{
          running: refreshing,
          waiting,
          said: refreshed,
          refused: refreshTrouble,
          ask: refresh,
        }}
        reconnect={offer}
        locked={custody === 'locked' ? unlockOffer : null}
      />
    </div>
  );
}

/** A listing not asked for yet, which is a listing on its way. */
const LOADING: Asked<Listing> = { status: 'loading' };

/**
 * The tree opened down to where `view` says the screen is: every folder above
 * the folder open, the folder itself too, and the focus on the row the
 * reader's sheet is open on — or on the folder, or on nothing at the Library
 * root. `held` comes back as it was where it shows that already.
 *
 * A folder the sheet is open on is not opened by it: the folder the screen's
 * actions are about is the one it stands in unless it was open already, and
 * only that folder and the ones above it are opened.
 */
function restored(held: TreeState, view: ViewState): TreeState {
  const focus = sheetKey(view) ?? (view.folder === '' ? null : folderKey(view.folder));
  const above = ancestry(view.folder);
  if (focus === held.focus && above.every((folder) => held.expanded.has(folder))) {
    return held;
  }
  return { focus, expanded: new Set([...held.expanded, ...above]) };
}

/**
 * Whether somebody is looking at this page: shown, and the window it is in
 * holding the focus. What the explorer asks on its own waits for this.
 */
function inFront(): boolean {
  return document.visibilityState === 'visible' && document.hasFocus();
}

/**
 * How many of its own notices a page keeps: the server's log is bounded, and
 * so is what a page adds to it.
 */
const NOTICES_KEPT = 50;
