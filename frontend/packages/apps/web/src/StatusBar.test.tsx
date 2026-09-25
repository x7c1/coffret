import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it } from 'vitest';

import type { Fill, Freeze, Sync } from '@coffret/api';

import {
  NOTHING_DISMISSED,
  putAwayFolders,
  type Dismissed,
  type Flow,
  type Queue,
} from './dismissed';
import type { Trouble } from './retry';
import { StatusBar } from './StatusBar';
import { COLOR } from './theme';

const refused =
  'the folder this device maps "albums" into is not the folder that mapping was recorded ' +
  'against, so nothing was put into it. Open a terminal on the device serving the Library. Run ' +
  '`coffret mappings --library <library>` to inspect the recorded mappings and `coffret map ' +
  '--help` to find the arguments. Use that listing to choose one recovery: reconnect the intended ' +
  'folder if it is elsewhere; if the folder at the recorded location is the intended one, record ' +
  'this mapping again with `coffret map`; or map another folder in its place only as a deliberate ' +
  'choice. If `coffret map` reports a local marker problem, correct the problem and run it again. ' +
  'Then return to the explorer and try the action again';
const renderedRefusal = refused
  .replaceAll('"', '&quot;')
  .replace('<library>', '&lt;library&gt;');

function filling(over: Partial<Fill> = {}): Fill {
  return {
    run: 1,
    waiting: [],
    dropped: [],
    displaced: [],
    folder: 'albums',
    status: 'stopped',
    total: 2,
    done: 0,
    declined: [],
    stopped: { error: 'storage', message: 'Storage did not answer' },
    ...over,
  };
}

function syncing(over: Partial<Sync> = {}): Sync {
  return {
    run: 1,
    step: null,
    status: 'stopped',
    added: 0,
    findings: [],
    stopped: { error: 'storage', message: 'Storage did not answer' },
    ...over,
  };
}

function freezing(over: Partial<Freeze> = {}): Freeze {
  return {
    run: 1,
    step: null,
    waiting: [],
    dropped: [],
    displaced: [],
    folder: 'books/one',
    status: 'stopped',
    packs: 0,
    entries: 0,
    findings: [],
    stopped: { error: 'storage', message: 'Storage did not answer' },
    ...over,
  };
}

/** The lines of those runs put away, and nothing else. */
function read(runs: Partial<Record<Flow, number>>): Dismissed {
  return { ...NOTHING_DISMISSED, runs: { ...NOTHING_DISMISSED.runs, ...runs } };
}

/** The offer of those lost folders put away, and nothing else. */
function forgot(queue: Queue, folders: readonly string[]): Dismissed {
  return putAwayFolders(NOTHING_DISMISSED, queue, folders);
}

function draw({
  fill = null,
  sync = null,
  freeze = null,
  dismissed = NOTHING_DISMISSED,
  trouble = null,
}: {
  fill?: Fill | null;
  sync?: Sync | null;
  freeze?: Freeze | null;
  dismissed?: Dismissed;
  trouble?: Trouble | null;
}): string {
  return renderToStaticMarkup(
    <StatusBar
      library={{
        status: 'ready',
        value: { name: 'Home', library_id: 'library-id', provider: 'Storage' },
      }}
      fetching={null}
      adding={null}
      fill={fill}
      sync={sync}
      freeze={freeze}
      trouble={trouble}
      dismissed={dismissed}
      onDismiss={() => undefined}
      onRetryFill={() => undefined}
      onRetrySync={() => undefined}
      onRetryFreeze={() => undefined}
      onLock={() => undefined}
      locking={false}
      refresh={{ running: false, said: null, refused: null, ask: () => undefined }}
    />,
  );
}

// The failure two books in one session reaches. A freeze Storage stopped is
// followed by the next book off the queue, and the run that stopped is then not
// the one on record — so the line naming it and the offer of a second attempt
// would both go with it, inside a tick and unread.
it('keeps the line and the offer of a book the next one took the record from', () => {
  const html = draw({
    freeze: freezing({
      folder: 'books/two',
      status: 'freezing',
      stopped: null,
      displaced: [freezing({ folder: 'books/one' })],
    }),
  });

  expect(html).toContain('packing books/two');
  expect(html).toContain('pack books/one');
});

// The line of the run now going takes the bar while it is going, because its
// counts are still moving and the stopped one has said all it has to say. What
// the stopped one keeps regardless is its button: a book outside the Library
// nobody can press is a book nobody brings in.
it('gives the line to the run now going and the stopped one its own button', () => {
  const html = draw({
    fill: filling({
      folder: 'letters',
      status: 'filling',
      stopped: null,
      displaced: [filling({ folder: 'albums' })],
    }),
  });

  expect(html).toContain('bringing over 0/2 in letters');
  expect(html).toContain('bring over albums');
  expect(html).not.toContain('bring over again');
});

// And it takes the line once the run on record has nothing to say — a fill that
// finished having placed everything says nothing at all — in the colour every
// refusal on this screen is in, because that is what it is.
it('says what stopped a displaced run once the line is free', () => {
  const html = draw({
    fill: filling({
      folder: 'letters',
      status: 'done',
      done: 2,
      total: 2,
      stopped: null,
      displaced: [filling({ folder: 'albums' })],
    }),
  });

  expect(html).toContain('could not bring over albums — Storage did not answer');
  expect(html).toContain(`color:${COLOR.refused}`);
});

// It is a notice about no run the bar is drawing a line for, so it is put away
// by name — and a dismissal of the running flow's line must not carry it off,
// which putting it away by run number would do: the number a dismissal spends is
// the flow's latest, and a displaced run is never that.
it('puts a displaced run away by name and not with the running line', () => {
  const freeze = freezing({
    folder: 'books/two',
    status: 'done',
    packs: 1,
    entries: 3,
    stopped: null,
    displaced: [freezing({ folder: 'books/one' })],
  });

  expect(draw({ freeze, dismissed: read({ freeze: 9 }) })).toContain('pack books/one');
  expect(draw({ freeze, dismissed: forgot('freeze', ['books/one']) })).not.toContain(
    'pack books/one',
  );
});

// A refusal of one of these presses stands for as long as the offer it answered
// does, and names the button it answered: the bar can hold out several at once,
// and the server's sentence names no button and often no folder either.
it('names the displaced run its refusal answers by that button words', () => {
  const html = draw({
    fill: filling({
      folder: 'letters',
      status: 'filling',
      stopped: null,
      displaced: [filling({ folder: 'albums' })],
    }),
    trouble: { pressed: { flow: 'fill', folder: 'albums' }, said: 'the Library is locked' },
  });

  expect(html).toContain('bring over albums: the Library is locked');
});

// And the same run once the next folder has taken the record from it. Being
// displaced is somebody else's folder starting rather than anything that
// happened to this failure, so the offer is made on the terms it was made on
// while this was the run on record: none. The line stays — the pages are half
// here and the sentence names the gesture that settles it — and only the run
// beside it, which Storage merely did not answer for, is worth a button.
it('makes no second attempt at a displaced run a refused root stopped', () => {
  const html = draw({
    fill: filling({
      folder: 'letters',
      status: 'done',
      done: 2,
      total: 2,
      stopped: null,
      displaced: [
        filling({
          folder: 'albums',
          stopped: { error: 'declined', message: refused, reason: 'refused_root' },
        }),
        filling({ folder: 'books' }),
      ],
    }),
  });

  expect(html).toContain(renderedRefusal);
  expect(html).not.toContain('bring over albums</button>');
  expect(html).toContain('bring over books</button>');
});

it('keeps a refused-root explanation visible without offering the same fill again', () => {
  const html = draw({
    fill: filling({
      stopped: { error: 'declined', message: refused, reason: 'refused_root' },
    }),
  });

  expect(html).toContain(renderedRefusal);
  expect(html).not.toContain('bring over again');
});

it('suppresses retries for refused-root syncs and freezes too', () => {
  const stopped = { error: 'declined' as const, message: refused, reason: 'refused_root' as const };

  expect(draw({ sync: syncing({ stopped }) })).toContain(renderedRefusal);
  expect(draw({ sync: syncing({ stopped }) })).not.toContain('back up again');
  expect(draw({ freeze: freezing({ stopped }) })).toContain(renderedRefusal);
  expect(draw({ freeze: freezing({ stopped }) })).not.toContain('pack again');
});

it('offers retries for ordinary and legacy stopped responses only', () => {
  expect(draw({ fill: filling() })).toContain('bring over again');
  expect(
    draw({ fill: filling({ stopped: { error: 'declined', message: 'an older refusal' } }) }),
  ).toContain('bring over again');

  for (const fill of [
    null,
    filling({ status: 'filling', stopped: null }),
    filling({ status: 'done', stopped: null }),
    filling({ status: 'superseded', stopped: null }),
  ]) {
    expect(draw({ fill })).not.toContain('bring over again');
  }
});

it('restores the fill retry when later activity reports an ordinary stop', () => {
  const refusedHtml = draw({
    fill: filling({
      stopped: { error: 'declined', message: refused, reason: 'refused_root' },
    }),
  });
  const laterHtml = draw({ fill: filling() });

  expect(refusedHtml).not.toContain('bring over again');
  expect(laterHtml).toContain('bring over again');
});

// The state this whole arrangement exists for: a sync that finished carrying
// findings owns the one line the bar has, and every later fill's progress is
// drawn underneath it for the life of the tab. Reading it is what ends it, and
// what stands there afterwards is the line that was waiting behind it.
it('lets the fill through once a finished sync line has been put away', () => {
  const sync = syncing({
    status: 'done',
    added: 1,
    findings: [
      {
        path: 'albums/holiday.jpg',
        message: 'it is inside a Pack',
        reason: 'surfaced',
        surfaced: 'ChangedInPack',
      },
    ],
  });
  const fill = filling({ status: 'filling', done: 1, total: 2, stopped: null });

  const before = draw({ sync, fill });
  expect(before).toContain('albums/holiday.jpg');
  expect(before).not.toContain('bringing over 1/2');

  const after = draw({ sync, fill, dismissed: read({ sync: 1 }) });
  expect(after).not.toContain('albums/holiday.jpg');
  expect(after).toContain('bringing over 1/2 in albums');
});

// Offered from a run that is over and from nowhere else: a line about work
// happening now has counts still moving in it.
it('offers the dismissal only for a run that is over', () => {
  expect(draw({ fill: filling({ status: 'filling', stopped: null }) })).not.toContain(
    '>dismiss<',
  );
  expect(draw({ fill: filling() })).toContain('>dismiss<');
});

// What a fill left behind is the same kind of news as a sync's findings and is
// drawn like it: this line is the one place somebody is told a file they opened
// a folder for was not placed, and in the grey the housekeeping beside it is
// written in it would be read past. It is not a refusal either — the run
// finished — so it must not arrive in the colour a stopped run arrives in.
it('draws what a finished fill left behind as a finding rather than as a refusal', () => {
  const left = draw({
    fill: filling({
      status: 'done',
      done: 1,
      declined: [
        { path: 'albums/holiday.jpg', error: 'declined', message: 'it is inside a Pack' },
      ],
      stopped: null,
    }),
  });

  expect(left).toContain('1 file was not placed');
  expect(left).toContain(`color:${COLOR.warn}`);
  expect(left).not.toContain(`color:${COLOR.refused}`);
  expect(draw({ fill: filling() })).toContain(`color:${COLOR.refused}`);
});

// A worker that ended without an answer threw the queue away. The line and the
// retry beside it both name the folder that died, so the folders that never
// started are said separately and taken up separately — a single "try again"
// would leave them unreachable.
it('names the folders the queue lost and offers each of them', () => {
  const html = draw({
    fill: filling({ status: 'done', done: 2, dropped: ['books/vol-2'] }),
  });

  expect(html).toContain('books/vol-2 was dropped before it was brought over');
  expect(html).toContain('bring over books/vol-2');
});

// A line put away takes its offer with it. A bare "bring over again" standing
// where no sentence says what failed is a button with nothing behind it.
it('takes the retry offer away with the line it was made from', () => {
  const fill = filling();

  expect(draw({ fill })).toContain('bring over again');
  expect(draw({ fill, dismissed: read({ fill: 1 }) })).not.toContain(
    'bring over again',
  );
});

// But the folders the queue lost are not that offer and do not go with it: they
// were never run, nobody has taken them up, and the line about them is not one
// of the three a dismissal is about.
it('keeps the dropped folders on offer after the fill line is put away', () => {
  const fill = filling({ status: 'done', done: 2, dropped: ['books/vol-2'] });

  expect(draw({ fill, dismissed: read({ fill: 1 }) })).toContain(
    'bring over books/vol-2',
  );
});

// A freeze worker that ended without an answer took the books waiting behind it
// with it. They were never run, nothing on record mentions them, and the line
// about the one that died names only that one.
it('names the books the freeze queue lost and offers each of them', () => {
  const freeze = freezing({ status: 'done', packs: 1, entries: 3, dropped: ['books/two'] });

  // The offer stands whatever the bar's one line is saying, because the book it
  // is about is outside the Library whatever became of the one that ran.
  expect(draw({ freeze })).toContain('pack books/two');

  // And the sentence gets the line once the run that died has had its say and
  // been put away — the news outlives the sentence it arrived beside.
  expect(draw({ freeze, dismissed: read({ freeze: 1 }) })).toContain(
    'books/two was dropped before it was packed',
  );
});

// The one kind of notice that used to have no way out. A lost folder is not a
// run, so the dismissal keyed on runs never offered anything for it — and the
// server keeps the offer until that folder is armed, which for somebody who has
// decided not to bring it over is its line and its button for the life of the
// process.
it('offers a dismissal for the folders a queue lost', () => {
  const fill = filling({ status: 'done', done: 2, dropped: ['books/vol-2'] });

  expect(draw({ fill, dismissed: read({ fill: 1 }) })).toContain('>dismiss<');
});

// And putting it away takes the line and the buttons it named together: they
// are one notice, and a bar left holding the offers under a sentence nobody
// wants would have put away nothing.
it('puts the lost folders away with the line that named them', () => {
  const fill = filling({ status: 'done', done: 2, dropped: ['books/vol-2', 'letters'] });
  const after = draw({ fill, dismissed: forgot('fill', ['books/vol-2', 'letters']) });

  expect(after).not.toContain('was dropped before it was brought over');
  expect(after).not.toContain('bring over books/vol-2');
  expect(after).not.toContain('bring over letters');
});

// One queue's notice is not the other's: a freeze that lost a book says nothing
// about the folders a fill lost, and the line that surfaces once the first is
// put away is the second one.
it('keeps the two queues of lost folders apart', () => {
  const fill = filling({ status: 'done', done: 2, dropped: ['albums'] });
  const freeze = freezing({ status: 'done', packs: 1, entries: 3, dropped: ['books/two'] });
  // The freeze's own line leads, so it is read and put away first; what stands
  // where it stood is the book its queue lost, and after that the fill's.
  const after = draw({
    fill,
    freeze,
    dismissed: putAwayFolders(read({ freeze: 1 }), 'freeze', ['books/two']),
  });

  expect(after).not.toContain('pack books/two');
  expect(after).toContain('albums was dropped before it was brought over');
  expect(after).toContain('bring over albums');
});

// The failure this is here for. One Storage outage leaves the bar holding out a
// button per folder either queue lost, side by side and worded alike, and the
// refusal that answers a press is the server's sentence about the operation: it
// names no button, and for a Library that is locked it names no folder either.
// Told only that, a person cannot tell which of the buttons in front of them
// did nothing.
it('says which of two buttons standing together was refused', () => {
  const fill = filling({ status: 'done', done: 2, dropped: ['letters', 'books/vol-2'] });
  const html = draw({
    fill,
    trouble: {
      pressed: { flow: 'fill', folder: 'letters' },
      said: 'the Library is locked on this device',
    },
  });

  // Both offers stand, so the sentence has to carry the one it answers.
  expect(html).toContain('bring over letters');
  expect(html).toContain('bring over books/vol-2');
  expect(html).toContain('bring over letters: the Library is locked on this device');
  expect(html).not.toContain('bring over books/vol-2: the Library is locked');
});

// The second attempt at the run that stopped is a different offer from the
// folders its queue lost, and stands beside them under different words. A
// refusal of it names those words rather than the folder, because the words are
// what the person pressed.
it('names the second attempt by the words its own button stands under', () => {
  const fill = filling({ dropped: ['letters'] });
  const html = draw({
    fill,
    trouble: {
      pressed: { flow: 'fill', folder: 'albums' },
      said: 'Storage did not answer',
    },
  });

  expect(html).toContain('>bring over again<');
  expect(html).toContain('>bring over again: Storage did not answer<');
  // And not by the folder, which here is the wording of no button at all: the
  // one offered for that folder says "bring over again".
  expect(html).not.toContain('>bring over albums:');
});

// Each flow is named in its own verb, the one its buttons are worded in: a
// refused freeze must not read as a refused fill, and the sync's offer names no
// folder because the sync walks the device's mappings.
it('names a refused freeze and a refused sync in their own words', () => {
  expect(
    draw({
      freeze: freezing({ status: 'done', packs: 1, entries: 3, dropped: ['books/two'] }),
      trouble: {
        pressed: { flow: 'freeze', folder: 'books/two' },
        said: 'Storage did not answer',
      },
    }),
  ).toContain('pack books/two: Storage did not answer');

  expect(
    draw({
      sync: syncing(),
      trouble: { pressed: { flow: 'sync' }, said: 'Storage did not answer' },
    }),
  ).toContain('back up again: Storage did not answer');
});
