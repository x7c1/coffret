import { expect, it } from 'vitest';

import type { Freeze } from '@coffret/api';

import {
  folderUnder,
  foldersWith,
  isPending,
  nameDefect,
  pendingAfter,
  strandedFolders,
} from './newFolder';

/** A book Storage stopped packing, over the shape a freeze always has. */
function stoppedFreeze(over: Partial<Freeze> = {}): Freeze {
  return {
    run: 1,
    folder: 'books/vol-1',
    status: 'stopped',
    packs: 0,
    entries: 0,
    noted: [],
    step: null,
    waiting: [],
    dropped: [],
    displaced: [],
    stopped: { error: 'storage', message: "the Library's Storage did not answer" },
    ...over,
  };
}

// EP-2: one component of an Entry Path and nothing else. A name carrying a
// separator is somebody asking for two folders at once, and the two relative
// references are not names — the server refuses every one of these, and saying
// so here means it is said before anything is on the screen.
it('refuses a name that is not one component of an Entry Path', () => {
  expect(nameDefect('')).not.toBeNull();
  expect(nameDefect('vol 1/scans')).not.toBeNull();
  expect(nameDefect('.')).not.toBeNull();
  expect(nameDefect('..')).not.toBeNull();
  expect(nameDefect('a\0b')).not.toBeNull();
});

// A name that merely begins with a dot is a name, and so is one with spaces and
// accents in it: what a person calls their own folder is theirs.
it('takes an ordinary name', () => {
  expect(nameDefect('vol-1')).toBeNull();
  expect(nameDefect('.hidden')).toBeNull();
  expect(nameDefect('café — scans')).toBeNull();
});

it('puts a new folder under the one that is open', () => {
  expect(folderUnder('books', 'vol-1')).toBe('books/vol-1');
  expect(folderUnder('', 'books')).toBe('books');
});

// EP-1: a name just typed is text from outside the Library, and it goes into NFC
// on the way in — the same form the server puts it in. Equality against the
// paths the server answers with is byte-exact over that form (EP-3), so a name
// left decomposed would name the folder the Library names and match none of it:
// never let go of, and drawn a second time beside it.
it('puts a typed name into the form the Library names it in', () => {
  // The decomposed spelling is derived rather than typed: the two are the same
  // word on the screen, and only the code points tell them apart.
  const composed = 'café';
  const decomposed = composed.normalize('NFD');
  expect(decomposed).not.toBe(composed);

  const path = folderUnder('books', decomposed);
  expect(path).toBe(`books/${composed}`);
  expect(pendingAfter([path], [`books/${composed}`])).toEqual([]);
  expect(foldersWith([`books/${composed}`], [path])).toEqual([`books/${composed}`]);
});

// The order is the byte order of the canonical paths, which is the one order
// every device agrees on. A folder made here that sat at the end of the tree
// would move the moment its first Entry commits, and a row that jumps when the
// freeze lands is a row a person loses.
it('draws a new folder where the Library will put it, not at the end', () => {
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

// Nothing pending is the ordinary case, and it leaves the server's own answer
// exactly as it arrived — order included.
it('leaves the Library’s folders alone when nothing is pending', () => {
  expect(foldersWith(['albums', 'books'], [])).toEqual(['albums', 'books']);
});

// A folder the Library already names is not this screen's to draw a second time:
// one place, one row.
it('does not draw a folder twice when the Library has caught up', () => {
  expect(foldersWith(['albums', 'books'], ['books'])).toEqual(['albums', 'books']);
});

// The whole of the lifecycle's end: the freeze commits, the tree is asked again,
// the server names the folder, and it stops being this screen's business.
it('lets go of a folder the moment the Library names it', () => {
  expect(pendingAfter(['books/vol-1', 'books/vol-2'], ['books', 'books/vol-1'])).toEqual([
    'books/vol-2',
  ]);
});

// And a folder made and never dropped into stays: the Library will never name
// it, and one that vanished from under somebody about to drop a book into it
// would be worse than one that outstays its usefulness. It goes with the tab.
it('keeps a folder nothing has been dropped into yet', () => {
  expect(pendingAfter(['books/vol-1'], ['albums', 'books'])).toEqual(['books/vol-1']);
});

// The other way in, and the only one a reload survives. Nothing was written
// down — the server is still holding the freeze that stopped, and the folder is
// named in it. Without this the pages sit on the disk, out of the Library, with
// no row in the tree to reach them by and no second attempt offered.
it('takes back the folder of a book a stopped freeze left behind', () => {
  expect(strandedFolders(stoppedFreeze(), ['albums', 'books'])).toEqual(['books/vol-1']);

  // And it enters the lifecycle exactly where a folder made by hand does:
  // drawn in the Library's own order, and a drop into it a book coming in.
  const back = ['books/vol-1'];
  expect(foldersWith(['albums', 'books'], back)).toEqual([
    'albums',
    'books',
    'books/vol-1',
  ]);
  expect(isPending(back, 'books/vol-1')).toBe(true);
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
// reloaded this is a no-op — the folder is pending there already. Only a
// finished freeze handed its folder to the Library.
it('takes back a freeze still packing, and nothing from one that is over', () => {
  expect(strandedFolders(null, [])).toEqual([]);
  expect(strandedFolders(stoppedFreeze({ status: 'freezing' }), [])).toEqual([
    'books/vol-1',
  ]);
  expect(strandedFolders(stoppedFreeze({ status: 'done' }), [])).toEqual([]);
});

// The server queues a second book rather than refusing it, so a book dropped
// into a folder made while another is packing sits in `waiting` — a folder with
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
  const lost = stoppedFreeze({ dropped: ['books/vol-2'] });

  expect(strandedFolders(lost, ['albums', 'books'])).toEqual([
    'books/vol-1',
    'books/vol-2',
  ]);

  // Even where the run on record is over: what was queued behind it did not end
  // with it, and the Library still names none of it.
  expect(strandedFolders(stoppedFreeze({ status: 'done', dropped: ['books/vol-2'] }), [])).toEqual([
    'books/vol-2',
  ]);
});

// And the book the run after it took the record from, which is what two books in
// one session reaches: the first is stopped by Storage, the worker takes the
// second off the queue, and the first is then in none of the lists above. Its
// folder was never anything but this browser's, so a reload that did not take it
// back would draw no row for it at all — the pages on the disk, out of the
// Library, and nothing on the screen saying so.
it('takes back the folder of a book the next run took the record from', () => {
  const after = stoppedFreeze({
    folder: 'books/vol-2',
    status: 'freezing',
    displaced: [stoppedFreeze({ folder: 'books/vol-1' })],
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
        displaced: [stoppedFreeze({ folder: 'books/vol-1' })],
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
    dropped: ['books/vol-2', 'books/vol-1'],
  });

  expect(strandedFolders(twice, [])).toEqual(['books/vol-1', 'books/vol-2']);
});

// EP-2: the Library root is not a path and not a folder anybody made, so it is
// never one of these — and a root taken back would make every drop at the top of
// the Library a book import.
it('never takes back the Library root', () => {
  expect(strandedFolders(stoppedFreeze({ folder: '' }), [])).toEqual([]);
});

// What the drop reads to know which gesture it is. A folder made here is a book
// being brought in; every other folder is files being added to one that exists,
// and nothing about that changes.
it('says which folder a drop would be a book import into', () => {
  expect(isPending(['books/vol-1'], 'books/vol-1')).toBe(true);
  expect(isPending(['books/vol-1'], 'books')).toBe(false);
  expect(isPending([], 'books/vol-1')).toBe(false);
});
