import { expect, it } from 'vitest';

import type { DisplacedFreeze, Freeze, FreezeStatus, Refused } from '@coffret/api';

import { foldersWith, heldAfter, strandedFolders } from './stranded';

/** What stopped the books in these cases: Storage not answering. */
const STORAGE: Refused = {
  kind: 'storage',
  message: "the Library's Storage did not answer",
  reason: null,
  surfaced: null,
};

/**
 * A book Storage stopped packing, over the shape a freeze always has — or, where
 * `over` names another status, a freeze in that state, which carries no
 * refusal.
 */
function stoppedFreeze(
  over: Partial<Omit<Freeze, 'status' | 'stopped'>> & { status?: FreezeStatus } = {},
): Freeze {
  const { status = 'stopped', ...rest } = over;
  const own = {
    run: 1,
    folder: 'books/vol-1',
    packs: 0,
    entries: 0,
    findings: [],
    step: null,
    waiting: [],
    discarded: [],
    displaced: [],
    ...rest,
  };
  return status === 'stopped'
    ? { ...own, status, stopped: STORAGE }
    : { ...own, status, stopped: null };
}

/** A book Storage stopped packing that a later one took the record from. */
function displacedFreeze(folder: string): DisplacedFreeze {
  return {
    run: 1,
    folder,
    packs: 0,
    entries: 0,
    findings: [],
    step: null,
    status: 'stopped',
    stopped: STORAGE,
  };
}

// The order is the byte order of the canonical paths, which is the one order
// every device agrees on. A stranded folder that sat at the end of the tree
// would move the moment its first Entry commits, and a row that jumps when the
// freeze lands is a row a person loses.
it('draws a stranded folder where the Library will put it, not at the end', () => {
  expect(foldersWith(['albums', 'books', 'films'], ['books/vol-1'])).toEqual([
    'albums',
    'books',
    'books/vol-1',
    'films',
  ]);
  expect(foldersWith(['albums', 'films'], ['books'])).toEqual([
    'albums',
    'books',
    'films',
  ]);
});

// Nothing stranded is the ordinary case, and it leaves the server's own answer
// exactly as it arrived — order included.
it('leaves the Library’s folders alone when nothing is stranded', () => {
  expect(foldersWith(['albums', 'books'], [])).toEqual(['albums', 'books']);
});

// A folder the Library already names is not this screen's to draw a second time:
// one place, one row.
it('does not draw a folder twice when the Library has caught up', () => {
  expect(foldersWith(['albums', 'books'], ['books'])).toEqual(['albums', 'books']);
});

// The whole of the lifecycle's end: the freeze commits, the tree is asked again,
// the server names the folder, and it stops being the screen's to draw.
it('lets go of a folder the moment the Library names it', () => {
  expect(heldAfter(['books/vol-1', 'books/vol-2'], [], ['books', 'books/vol-1'])).toEqual([
    'books/vol-2',
  ]);
});

// And keeps one the Library does not name yet, even where the freeze that named
// it has let go of it: the tree is asked again after a commit, and a row gone
// in the moment between would be a row gone from under the pointer.
it('keeps a folder until the Library names it, and adds the ones newly stranded', () => {
  expect(heldAfter(['books/vol-1'], [], ['albums', 'books'])).toEqual(['books/vol-1']);
  expect(heldAfter(['books/vol-1'], ['books/vol-1', 'books/vol-2'], ['books'])).toEqual([
    'books/vol-1',
    'books/vol-2',
  ]);
  const held = ['books/vol-1'];
  expect(heldAfter(held, ['books/vol-1'], ['books'])).toBe(held);
});

// The other way in, and the only one a reload survives. Nothing was written
// down — the server is still holding the freeze that stopped, and the folder is
// named in it. Without this the pages sit on the disk, out of the Library, with
// no row in the tree to reach them by and no second attempt offered.
it('takes back the folder of a book a stopped freeze left behind', () => {
  expect(strandedFolders(stoppedFreeze(), ['albums', 'books'])).toEqual(['books/vol-1']);

  // And it is drawn in the Library's own order.
  const back = ['books/vol-1'];
  expect(foldersWith(['albums', 'books'], back)).toEqual([
    'albums',
    'books',
    'books/vol-1',
  ]);
});

// A run that committed before it stopped left a folder the Library names, and
// that one is the server's to answer for: taking it back would draw it twice and
// would make the next drop into it a book import rather than the files being
// added that it is.
it('leaves alone a folder the Library has taken over', () => {
  expect(
    strandedFolders(stoppedFreeze(), ['albums', 'books', 'books/vol-1']),
  ).toEqual([]);
});

// A freeze still packing comes back too: a tab reloaded mid-pack has the same
// folder mid-flight, and taking it back is what keeps a drop of forgotten pages
// refused while the pack runs instead of silently synced. In the tab that never
// reloaded this is a no-op — the folder is on the screen there already. Only a
// finished freeze that committed a Pack handed its folder to the Library.
it('takes back a freeze still packing, and nothing from one that committed', () => {
  expect(strandedFolders(null, [])).toEqual([]);
  expect(strandedFolders(stoppedFreeze({ status: 'freezing' }), [])).toEqual([
    'books/vol-1',
  ]);
  expect(strandedFolders(stoppedFreeze({ status: 'done', packs: 1 }), [])).toEqual([]);
});

// A freeze that finished with every file a finding committed nothing: it is
// `done`, but its pages are on the disk and out of the Library, and after a
// reload nothing else on the screen names the folder. It is held like a stopped
// one — until the Library names it, which is what lets it go.
it('takes back the folder of a freeze that finished having committed nothing', () => {
  expect(strandedFolders(stoppedFreeze({ status: 'done', packs: 0 }), ['books'])).toEqual([
    'books/vol-1',
  ]);
  expect(
    strandedFolders(stoppedFreeze({ status: 'done', packs: 0 }), ['books', 'books/vol-1']),
  ).toEqual([]);
});

// The server queues a second book rather than refusing it, so a book dropped
// while another is packing sits in `waiting` — a folder with
// pages on the disk, out of the Library, and named nowhere else on the screen.
// A reload that drew only the running freeze's folder would leave no row to
// walk into until its turn came.
it('takes back the folders of the books waiting their turn', () => {
  const queued = stoppedFreeze({
    status: 'freezing',
    waiting: ['books/vol-2', 'books/vol-3'],
  });

  expect(strandedFolders(queued, ['albums', 'books'])).toEqual([
    'books/vol-1',
    'books/vol-2',
    'books/vol-3',
  ]);
});

// And what a worker that died threw away. The status bar reaches one of these
// by name, so it is not a dead end — but between the panic and the press there
// is no row for it, and pressing it would move the screen to a folder the tree
// does not draw.
it('takes back the folders of the books a worker threw away', () => {
  const lost = stoppedFreeze({ discarded: ['books/vol-2'] });

  expect(strandedFolders(lost, ['albums', 'books'])).toEqual([
    'books/vol-1',
    'books/vol-2',
  ]);

  // Even where the run on record is over: what was queued behind it did not end
  // with it, and the Library still names none of it.
  expect(strandedFolders(stoppedFreeze({ status: 'done', packs: 1, discarded: ['books/vol-2'] }), [])).toEqual([
    'books/vol-2',
  ]);
});

// And the book the run after it took the record from, which is what two books in
// one session reaches: the first is stopped by Storage, the worker takes the
// second off the queue, and the first is then in none of the lists above. Its
// folder is not one the Library names, so a reload that did not take it
// back would draw no row for it at all — the pages on the disk, out of the
// Library, and nothing on the screen saying so.
it('takes back the folder of a book the next run took the record from', () => {
  const after = stoppedFreeze({
    folder: 'books/vol-2',
    status: 'freezing',
    displaced: [displacedFreeze('books/vol-1')],
  });

  expect(strandedFolders(after, ['albums', 'books'])).toEqual([
    'books/vol-2',
    'books/vol-1',
  ]);

  // Even once the book on record has committed: that one is the Library's from
  // then on, and the one it displaced is still outside it.
  expect(
    strandedFolders(
      stoppedFreeze({
        folder: 'books/vol-2',
        status: 'done',
        packs: 1,
        displaced: [displacedFreeze('books/vol-1')],
      }),
      ['albums', 'books', 'books/vol-2'],
    ),
  ).toEqual(['books/vol-1']);
});

// One place, one row. A book thrown away and then asked for again is named
// twice over in one answer while the queue takes it up, and a folder drawn
// twice is two rows for one place.
it('names a folder once however many lists hold it', () => {
  const twice = stoppedFreeze({
    status: 'freezing',
    folder: 'books/vol-1',
    waiting: ['books/vol-2'],
    discarded: ['books/vol-2', 'books/vol-1'],
  });

  expect(strandedFolders(twice, [])).toEqual(['books/vol-1', 'books/vol-2']);
});

// EP-2: the Library root is not a path and not a folder, so it is
// never one of these — and a root taken back would make every drop at the top of
// the Library a book import.
it('never takes back the Library root', () => {
  expect(strandedFolders(stoppedFreeze({ folder: '' }), [])).toEqual([]);
});
