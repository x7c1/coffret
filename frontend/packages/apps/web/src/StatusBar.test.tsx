import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it } from 'vitest';

import type { DisplacedFill, DisplacedFreeze, Fill, Freeze, Refused, Sync } from '@coffret/api';

import {
  NOTHING_DISMISSED,
  putAwayFolders,
  type Dismissed,
  type Flow,
  type Queue,
} from './dismissed';
import { RAN_OUT } from './reconnect';
import type { Offer } from './ReconnectOffer';
import type { Trouble } from './retry';
import { LOCKED, StatusBar, UNLOCK, type UnlockOffer } from './StatusBar';
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

/** A mapped root refused, as what stopped a run. */
const refusedRoot: Refused = {
  kind: 'refused_placement',
  message: refused,
  reason: 'refused_root',
  surfaced: null,
};

/** What stops a run in these cases unless a case says otherwise. */
const STORAGE: Refused = {
  kind: 'storage',
  message: 'Storage did not answer',
  reason: null,
  surfaced: null,
};

/**
 * A run's own fields, with its status and its refusal in one of the two pairs
 * they come in. Left out, the run is stopped by Storage; a refusal alone is a
 * different reason for stopping, and any other status carries none.
 */
type Over<Run extends { status: string }> = Partial<Omit<Run, 'status' | 'stopped'>> &
  (
    | { status?: 'stopped'; stopped?: Refused }
    | { status: Exclude<Run['status'], 'stopped'>; stopped: null }
  );

function filling(over: Over<Fill> = {}): Fill {
  return {
    run: 1,
    waiting: [],
    discarded: [],
    displaced: [],
    folder: 'albums',
    status: 'stopped',
    total: 2,
    done: 0,
    declined: [],
    findings: [],
    stopped: STORAGE,
    ...over,
  };
}

/** A fill that stopped and that a later one took the record from. */
function displacedFill(over: Partial<Omit<DisplacedFill, 'status'>> = {}): DisplacedFill {
  return {
    run: 1,
    folder: 'albums',
    total: 2,
    done: 0,
    declined: [],
    findings: [],
    status: 'stopped',
    stopped: STORAGE,
    ...over,
  };
}

function syncing(over: Over<Sync> = {}): Sync {
  return {
    run: 1,
    step: null,
    status: 'stopped',
    added: 0,
    findings: [],
    stopped: STORAGE,
    ...over,
  };
}

function freezing(over: Over<Freeze> = {}): Freeze {
  return {
    run: 1,
    step: null,
    waiting: [],
    discarded: [],
    displaced: [],
    folder: 'books/one',
    status: 'stopped',
    packs: 0,
    entries: 0,
    findings: [],
    stopped: STORAGE,
    ...over,
  };
}

/** A freeze that stopped and that a later one took the record from. */
function displacedFreeze(over: Partial<Omit<DisplacedFreeze, 'status'>> = {}): DisplacedFreeze {
  return {
    run: 1,
    step: null,
    folder: 'books/one',
    packs: 0,
    entries: 0,
    findings: [],
    status: 'stopped',
    stopped: STORAGE,
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
  reconnect = null,
  locked = null,
}: {
  fill?: Fill | null;
  sync?: Sync | null;
  freeze?: Freeze | null;
  dismissed?: Dismissed;
  trouble?: Trouble | null;
  reconnect?: Offer | null;
  locked?: UnlockOffer | null;
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
      refresh={{ running: false, said: null, refused: null, ask: () => undefined }}
      reconnect={reconnect}
      locked={locked}
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
      displaced: [displacedFreeze({ folder: 'books/one' })],
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
      displaced: [displacedFill({ folder: 'albums' })],
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
      displaced: [displacedFill({ folder: 'albums' })],
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
    displaced: [displacedFreeze({ folder: 'books/one' })],
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
      displaced: [displacedFill({ folder: 'albums' })],
    }),
    trouble: { pressed: { flow: 'fill', folder: 'albums' }, said: 'the Library is locked' },
  });

  expect(html).toContain('bring over albums: the Library is locked');
});

// And the same run once the next folder has taken the record from it. Being
// displaced is somebody else's folder starting rather than anything that
// happened to this failure, so the offer is made on the terms it was made on
// while this was the run on record: none. The line stays — the pages are half
// here and the sentence names the gesture that remedies it — and only the run
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
        displacedFill({
          folder: 'albums',
          stopped: refusedRoot,
        }),
        displacedFill({ folder: 'books' }),
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
      stopped: refusedRoot,
    }),
  });

  expect(html).toContain(renderedRefusal);
  expect(html).not.toContain('bring over again');
});

// A device that has to be enrolled again, and a server that needs its
// Passphrase, are both remedied at a terminal and by nothing a page can press:
// the line says so and stands on its own, with no button beside it that could
// only meet the same refusal.
it('offers no second attempt at a run an epoch or a lock stopped', () => {
  for (const error of ['epoch', 'locked'] as const) {
    const stopped: Refused = {
      kind: error,
      message: 'what remedies this is at a terminal',
      reason: null,
      surfaced: null,
    };

    const fill = draw({ fill: filling({ stopped }) });
    expect(fill, error).toContain('what remedies this is at a terminal');
    expect(fill, error).not.toContain('bring over again');
    expect(draw({ sync: syncing({ stopped }) }), error).not.toContain('back up again');
    expect(draw({ freeze: freezing({ stopped }) }), error).not.toContain('pack again');
  }
  expect(draw({ fill: filling() }), 'Storage that did not answer').toContain('bring over again');
});

it('suppresses retries for refused-root syncs and freezes too', () => {
  const stopped = refusedRoot;

  expect(draw({ sync: syncing({ stopped }) })).toContain(renderedRefusal);
  expect(draw({ sync: syncing({ stopped }) })).not.toContain('back up again');
  expect(draw({ freeze: freezing({ stopped }) })).toContain(renderedRefusal);
  expect(draw({ freeze: freezing({ stopped }) })).not.toContain('pack again');
});

it('offers retries for ordinary and legacy stopped responses only', () => {
  expect(draw({ fill: filling() })).toContain('bring over again');
  expect(
    draw({
      fill: filling({
        stopped: { kind: 'declined', message: 'an older refusal', reason: null, surfaced: null },
      }),
    }),
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

it('restores the fill retry when a later work answer reports an ordinary stop', () => {
  const refusedHtml = draw({
    fill: filling({
      stopped: refusedRoot,
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
    stopped: null,
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
        {
          path: 'albums/holiday.jpg',
          kind: 'declined',
          message: 'it is inside a Pack',
          reason: null,
          surfaced: null,
        },
      ],
      stopped: null,
    }),
  });

  expect(left).toContain('1 file was not placed');
  expect(left).toContain(`color:${COLOR.warn}`);
  expect(left).not.toContain(`color:${COLOR.refused}`);
  expect(draw({ fill: filling() })).toContain(`color:${COLOR.refused}`);
});

/** What a fill's reads found of the Library's Keyring, as the server says it. */
const degradedKeyring = {
  path: null,
  message:
    "the Library's Keyring is degraded: some of its replicas are missing or unreadable. " +
    'Files still open, and the next run that writes to the Library repairs it',
  reason: 'keyring_degraded',
  surfaced: null,
} as const;

// KL-15: replica loss is never silent, and the person it matters most for is
// one who only opens files — whose only run is the fill that brings a folder
// over. So a fill that finished with the Keyring degraded has a line, drawn as
// a sync's and a freeze's findings are: in the warn colour, and never as a
// stopped run. The files still opened, and nothing is offered again.
it("shows a finished fill's degraded Keyring as a finding without stopping it", () => {
  const html = draw({
    fill: filling({
      status: 'done',
      done: 2,
      findings: [degradedKeyring],
      stopped: null,
    }),
  });

  expect(html).toContain("the Library&#x27;s Keyring is degraded");
  expect(html).toContain(`color:${COLOR.warn}`);
  expect(html).not.toContain(`color:${COLOR.refused}`);
  expect(html).not.toContain('could not bring over');
  expect(html).not.toContain('bring over again');
  expect(html).toContain('>dismiss<');
});

// Beside the Entries it left behind, rather than instead of them: the rows
// carry those, and nothing but this line carries the finding.
it("says a fill's degraded Keyring after the files it did not place", () => {
  const html = draw({
    fill: filling({
      status: 'done',
      done: 1,
      declined: [
        {
          path: 'albums/holiday.jpg',
          kind: 'declined',
          message: 'it is inside a Pack',
          reason: null,
          surfaced: null,
        },
      ],
      findings: [degradedKeyring],
      stopped: null,
    }),
  });

  expect(html).toContain('1 file was not placed: albums/holiday.jpg');
  expect(html).toContain("; the Library&#x27;s Keyring is degraded");
  expect(html).toContain(`color:${COLOR.warn}`);
});

/** A Keyring repair a sync's commit made, as the server says it. */
const keyringRepaired = {
  path: null,
  message:
    'repaired the Keyring: 1 replica of generation 4 was missing or unreadable, ' +
    'and was rewritten from a surviving one',
  reason: 'keyring_repaired',
  surfaced: null,
} as const;

// KL-15: a repair performed is never silent. A sync that repaired the Keyring
// and finished says so in the warn colour at most: the set is whole again, so
// the run is not stopped over it and nothing is offered again.
it("shows a finished sync's Keyring repair as a finding without stopping it", () => {
  const html = draw({
    sync: syncing({ status: 'done', added: 1, findings: [keyringRepaired], stopped: null }),
  });

  expect(html).toContain('repaired the Keyring: 1 replica of generation 4');
  expect(html).toContain(`color:${COLOR.warn}`);
  expect(html).not.toContain(`color:${COLOR.refused}`);
  expect(html).not.toContain('could not back up');
  expect(html).not.toContain('back up again');
  expect(html).toContain('>dismiss<');
});

// And a sync whose commit failed after the repair says the repair after what
// stopped it: the line is the stop's, and the repair is not why it stopped.
it('says the Keyring repair a stopped sync made after what stopped it', () => {
  const html = draw({ sync: syncing({ findings: [keyringRepaired] }) });

  expect(html).toContain(
    'could not back up what was added — Storage did not answer; repaired the Keyring: ',
  );
  expect(html).toContain('back up again');
});

// A worker that ended without an answer threw the queue away. The line and the
// retry beside it both name the folder that died, so the folders that never
// started are said separately and taken up separately — a single "try again"
// would leave them unreachable.
it('names the folders the queue lost and offers each of them', () => {
  const html = draw({
    fill: filling({ status: 'done', stopped: null, done: 2, discarded: ['books/vol-2'] }),
  });

  expect(html).toContain('books/vol-2 was discarded before it was brought over');
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
it('keeps the discarded folders on offer after the fill line is put away', () => {
  const fill = filling({ status: 'done', stopped: null, done: 2, discarded: ['books/vol-2'] });

  expect(draw({ fill, dismissed: read({ fill: 1 }) })).toContain(
    'bring over books/vol-2',
  );
});

// A freeze worker that ended without an answer took the books waiting behind it
// with it. They were never run, nothing on record mentions them, and the line
// about the one that died names only that one.
it('names the books the freeze queue lost and offers each of them', () => {
  const freeze = freezing({
    status: 'done',
    stopped: null,
    packs: 1,
    entries: 3,
    discarded: ['books/two'],
  });

  // The offer stands whatever the bar's one line is saying, because the book it
  // is about is outside the Library whatever became of the one that ran.
  expect(draw({ freeze })).toContain('pack books/two');

  // And the sentence gets the line once the run that died has had its say and
  // been put away — the news outlives the sentence it arrived beside.
  expect(draw({ freeze, dismissed: read({ freeze: 1 }) })).toContain(
    'books/two was discarded before it was packed',
  );
});

// The one kind of notice that used to have no way out. A lost folder is not a
// run, so the dismissal keyed on runs never offered anything for it — and the
// server keeps the offer until that folder is armed, which for somebody who has
// decided not to bring it over is its line and its button for the life of the
// process.
it('offers a dismissal for the folders a queue lost', () => {
  const fill = filling({ status: 'done', stopped: null, done: 2, discarded: ['books/vol-2'] });

  expect(draw({ fill, dismissed: read({ fill: 1 }) })).toContain('>dismiss<');
});

// And putting it away takes the line and the buttons it named together: they
// are one notice, and a bar left holding the offers under a sentence nobody
// wants would have put away nothing.
it('puts the lost folders away with the line that named them', () => {
  const fill = filling({
    status: 'done',
    stopped: null,
    done: 2,
    discarded: ['books/vol-2', 'letters'],
  });
  const after = draw({ fill, dismissed: forgot('fill', ['books/vol-2', 'letters']) });

  expect(after).not.toContain('was discarded before it was brought over');
  expect(after).not.toContain('bring over books/vol-2');
  expect(after).not.toContain('bring over letters');
});

// One queue's notice is not the other's: a freeze that lost a book says nothing
// about the folders a fill lost, and the line that surfaces once the first is
// put away is the second one.
it('keeps the two queues of lost folders apart', () => {
  const fill = filling({ status: 'done', stopped: null, done: 2, discarded: ['albums'] });
  const freeze = freezing({
    status: 'done',
    stopped: null,
    packs: 1,
    entries: 3,
    discarded: ['books/two'],
  });
  // The freeze's own line leads, so it is read and put away first; what stands
  // where it stood is the book its queue lost, and after that the fill's.
  const after = draw({
    fill,
    freeze,
    dismissed: putAwayFolders(read({ freeze: 1 }), 'freeze', ['books/two']),
  });

  expect(after).not.toContain('pack books/two');
  expect(after).toContain('albums was discarded before it was brought over');
  expect(after).toContain('bring over albums');
});

// The failure this is here for. One Storage outage leaves the bar holding out a
// button per folder either queue lost, side by side and worded alike, and the
// refusal that answers a press is the server's sentence about the operation: it
// names no button, and for a Library that is locked it names no folder either.
// Told only that, a person cannot tell which of the buttons in front of them
// did nothing.
it('says which of two buttons standing together was refused', () => {
  const fill = filling({
    status: 'done',
    stopped: null,
    done: 2,
    discarded: ['letters', 'books/vol-2'],
  });
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
  const fill = filling({ discarded: ['letters'] });
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
      freeze: freezing({
        status: 'done',
        stopped: null,
        packs: 1,
        entries: 3,
        discarded: ['books/two'],
      }),
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

/** A grant Storage no longer takes, as what stopped a run. */
const RAN_OUT_REFUSAL: Refused = {
  kind: 'storage',
  message: "the Library's Storage no longer accepts this device's grant",
  reason: 'unauthenticated',
  surfaced: null,
};

/** The reconnect a Library on Google Drive offers, with nothing pressed yet. */
function offered(over: Partial<Offer> = {}): Offer {
  return {
    reconnect: null,
    said: null,
    refused: null,
    consentPage: null,
    ask: () => undefined,
    ...over,
  };
}

// A run the permission running out stopped is offered the one gesture that
// clears it, beside its line: the sentence saying why, and the button.
it('offers a reconnect beside a run a permission that ran out stopped', () => {
  const html = draw({ fill: filling({ stopped: RAN_OUT_REFUSAL }), reconnect: offered() });

  expect(html).toContain(RAN_OUT.replaceAll("'", '&#x27;'));
  expect(html).toContain('>reconnect</button>');
});

// And the press of a second attempt the permission refused is the same: the
// refusal line stands, and the reconnect after it.
it('offers a reconnect beside a press the permission refused', () => {
  const html = draw({
    sync: { run: 1, added: 0, findings: [], step: null, status: 'done', stopped: null },
    trouble: {
      pressed: { flow: 'sync' },
      said: RAN_OUT_REFUSAL.message,
      ranOut: true,
    },
    reconnect: offered(),
  });

  expect(html).toContain('>reconnect</button>');
});

// Storage not answering is not the permission, and a Library a consent page
// cannot help — `null` — is offered nothing however its run was stopped.
it('offers no reconnect where the refusal is not a permission, or none can be renewed', () => {
  expect(draw({ fill: filling(), reconnect: offered() })).not.toContain('reconnect</button>');
  expect(draw({ fill: filling({ stopped: RAN_OUT_REFUSAL }), reconnect: null })).not.toContain(
    'reconnect</button>',
  );
});

// While the consent page waits there is nothing to press: the page is open, and
// the line says to answer it. Once it ends without a grant, the button is back
// with the reason beside it.
it('withholds the button while a consent page waits and offers it again after', () => {
  const stopped = filling({ stopped: RAN_OUT_REFUSAL });
  const waiting = draw({
    fill: stopped,
    reconnect: offered({
      reconnect: { state: 'waiting', message: 'waiting for the consent page' },
      consentPage: 'https://consent.example/',
    }),
  });
  expect(waiting).not.toContain('>reconnect</button>');
  expect(waiting).toContain('waiting for the consent page');
  // And the page itself as a link, for a browser that blocked the tab.
  expect(waiting).toContain('href="https://consent.example/"');

  const refused = draw({
    fill: stopped,
    reconnect: offered({
      reconnect: { state: 'refused', message: 'the consent page was declined' },
    }),
  });
  expect(refused).toContain('>reconnect</button>');
  expect(refused).toContain('the consent page was declined');
});

/** The unlock as the bar offers it, with nothing pressed yet unless a case says. */
function unlockOffer(over: Partial<UnlockOffer> = {}): UnlockOffer {
  return { asking: false, said: null, refused: null, ask: () => undefined, ...over };
}

// The Library locked under a bar that was offering second attempts. Every one of
// them would meet the same locked refusal until the Passphrase is given, so the
// bar says the Library is locked, once, with the one button that can change it
// — and none of the others.
it('shows the locked line and the unlock in place of the offers of a second attempt', () => {
  const html = draw({
    fill: filling(),
    sync: syncing(),
    freeze: freezing(),
    locked: unlockOffer(),
  });

  expect(html).toContain(LOCKED);
  expect(html).toContain(`>${UNLOCK}</button>`);
  expect(html).toContain('Home — on Storage');
  for (const offer of ['bring over again', 'back up again', 'pack again', 'dismiss']) {
    expect(html, offer).not.toContain(offer);
  }
});

// And says it once: the same bar open shows none of it.
it('shows no locked line while the Library is open', () => {
  const html = draw({ fill: filling() });

  expect(html).not.toContain(LOCKED);
  expect(html).not.toContain(`>${UNLOCK}</button>`);
  expect(html).toContain('bring over again');
});

// What the press came to stands beside the button: the app asking for the
// Passphrase in its own window, or — under the command line — the server's
// sentence saying to start it again, drawn as the refusal it is.
it('shows what the server answered the unlock with', () => {
  const asked = draw({
    locked: unlockOffer({
      said: 'the Coffret app is asking for the Passphrase in its own window',
    }),
  });
  expect(asked).toContain('asking for the Passphrase in its own window');

  const sentence = 'it is unlocked by starting it again with the Passphrase';
  const refused = draw({ locked: unlockOffer({ refused: sentence }) });
  expect(refused).toContain(`<span style="color:${COLOR.refused}">${sentence}</span>`);
});

// A press waiting for its answer is not pressed again.
it('holds the unlock while a press waits for its answer', () => {
  const html = draw({ locked: unlockOffer({ asking: true }) });

  expect(html).toMatch(/<button disabled=""[^>]*>unlock<\/button>/);
});
