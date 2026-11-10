import { describe, expect, it, vi } from 'vitest';

import type { ListedFile, ListedFolder, Listing, Logged, Move, MoveAccepted, MovePreview, PendingMove } from '@coffret/api';

import type { Described } from './details';
import {
  askToMove,
  drawnMoves,
  followedMoves,
  grabbed,
  leavingLine,
  moveLandingOver,
  moveLine,
  movedOf,
  movedPath,
  moveQuestionOf,
  movesInto,
  moveTargets,
  moveTitle,
  movedRowsFrom,
  movingLine,
  NOTHING_TO_MOVE,
  notMovedLine,
  openedFor,
  pairsOf,
  renameDefect,
  renamePair,
  renameStepOf,
} from './moving';
import { earlier, inProgress } from './activity';
import { NOTHING_SELECTED, type Picked } from './selection';
import { treeRows, type FileRow, type FolderRow, type TreeRow } from './tree';

function file(path: string, over: Partial<ListedFile> = {}): ListedFile {
  return {
    name: path.slice(path.lastIndexOf('/') + 1),
    path,
    size: 10,
    mtime: null,
    deposited_at: null,
    state: 'remote',
    container: 'one-file',
    reserved: false,
    openable: true,
    content_type: 'image/jpeg',
    ...over,
  };
}

function folder(path: string): ListedFolder {
  return {
    name: path.slice(path.lastIndexOf('/') + 1),
    path,
    mapped: true,
    present_throughout: false,
    files_total: 1,
    files_present: 0,
    size: 0,
    mtime: null,
  };
}

function listing(path: string, over: Partial<Listing> = {}): Listing {
  return {
    path,
    mapped: true,
    held: true,
    folders: [],
    files: [],
    folders_on_disk: [],
    empty_mapped_prefixes: [],
    ...over,
  };
}

// The Library these rows are drawn from:
//
//   series/            series/vol-1/  series/vol-1/p1.jpg
//                      series/vol-2/
//                      series/cover.jpg
//   albums/
const SERIES = listing('series', {
  folders: [folder('series/vol-1'), folder('series/vol-2')],
  files: [file('series/cover.jpg')],
});
const VOL_1 = listing('series/vol-1', { files: [file('series/vol-1/p1.jpg')] });
const ROOT = listing('', { folders: [folder('series'), folder('albums')] });

function folderRow(path: string, over: Partial<FolderRow> = {}, from: Listing = SERIES): FolderRow {
  return {
    kind: 'folder',
    key: `folder:${path}`,
    path,
    name: path.slice(path.lastIndexOf('/') + 1),
    depth: path.split('/').length - 1,
    folder: folder(path),
    in: from,
    expanded: false,
    expandable: true,
    prospective: false,
    stranded: false,
    ...over,
  };
}

function fileRow(path: string, over: Partial<ListedFile> = {}, from: Listing = SERIES): FileRow {
  const listed = file(path, over);
  return { kind: 'file', key: `file:${path}`, path, name: listed.name, depth: path.split('/').length - 1, file: listed, in: from };
}

const SERIES_ROW = folderRow('series', {}, ROOT);
const ALBUMS = folderRow('albums', {}, ROOT);
const VOL1 = folderRow('series/vol-1', { expanded: true });
const PAGE = fileRow('series/vol-1/p1.jpg', {}, VOL_1);
const VOL2 = folderRow('series/vol-2');
const COVER = fileRow('series/cover.jpg');

const ROWS: TreeRow[] = [ALBUMS, SERIES_ROW, VOL1, PAGE, VOL2, COVER];

function selected(...rows: Described[]): Picked {
  return { focus: rows[0].key, selection: { keys: new Set(rows.map((row) => row.key)), anchor: rows[0].key } };
}

function preview(over: Partial<MovePreview> = {}): MovePreview {
  return {
    pairs: [{ from: 'series/vol-1', to: 'albums/vol-1' }],
    entries: 12,
    following: 12,
    leaving: 0,
    leaving_bytes: 0,
    edited: [],
    uploading: [],
    refused: [],
    ...over,
  };
}

describe('which rows a drag moves', () => {
  it('drags the selection where the row grabbed is in one of several, and the row alone otherwise', () => {
    const picked = selected(VOL2, COVER);
    expect(grabbed(ROWS, picked, COVER.key).moved).toEqual([VOL2, COVER]);
    expect(grabbed(ROWS, picked, COVER.key).picked).toBe(picked);

    const alone = grabbed(ROWS, picked, ALBUMS.key);
    expect(alone.moved).toEqual([ALBUMS]);
    expect(alone.picked.focus).toBe(ALBUMS.key);
    expect(alone.picked.selection.keys.size).toBe(0);

    const nothing: Picked = { focus: null, selection: NOTHING_SELECTED };
    expect(grabbed(ROWS, nothing, PAGE.key).moved).toEqual([PAGE]);
  });

  it('moves a folder and not the rows under it selected with it', () => {
    expect(grabbed(ROWS, selected(VOL1, PAGE, COVER), PAGE.key).moved).toEqual([VOL1, COVER]);
    expect(movedOf([SERIES_ROW, VOL1, PAGE, COVER])).toEqual([SERIES_ROW]);
  });

  it('leaves out what the Library does not hold', () => {
    const prepared = folderRow('series/new', { prospective: true });
    const stranded = folderRow('series/packing', { stranded: true });
    const added = fileRow('series/added.jpg', { state: 'added', container: null });
    expect(movedOf([prepared, stranded, added, COVER])).toEqual([COVER]);
    expect(grabbed(ROWS, { focus: null, selection: NOTHING_SELECTED }, prepared.key).moved).toEqual([]);
  });
});

describe('where a drag lands', () => {
  it('lands in a folder row, and in the folder a file row stands in', () => {
    expect(moveLandingOver(ALBUMS.key, [COVER])).toBe('albums');
    // vol-1's page stands in vol-1: the cover moves in there.
    expect(moveLandingOver(PAGE.key, [COVER])).toBe('series/vol-1');
  });

  it('takes a move to the root on the header’s Library, and says so', () => {
    expect(movesInto('', [VOL1])).toBe(true);
    expect(moveLine('')).toBe('Move to Library');
    expect(moveLine('albums')).toBe('Move to albums');
  });

  it('lands nowhere over no row, over the folder the rows stand in, over a dragged row, or under a dragged folder', () => {
    expect(moveLandingOver(null, [COVER])).toBeNull();
    // The folder the cover stands in, by its row and by a file row beside it.
    expect(moveLandingOver(SERIES_ROW.key, [COVER])).toBeNull();
    expect(moveLandingOver(COVER.key, [VOL2])).toBeNull();
    // A dragged row itself.
    expect(moveLandingOver(VOL2.key, [VOL2, COVER])).toBeNull();
    expect(moveLandingOver(VOL1.key, [VOL1])).toBeNull();
    // Under a dragged folder: its own rows, and what is deeper.
    expect(moveLandingOver(PAGE.key, [VOL1])).toBeNull();
    expect(moveLandingOver(VOL1.key, [SERIES_ROW])).toBeNull();
    // A top-level row is at the root already, where the header takes nothing.
    expect(movesInto('', [ALBUMS])).toBe(false);
    expect(movesInto('albums', [])).toBe(false);
  });

  it('asks for each row under its own name where it lands', () => {
    expect(pairsOf([VOL2, COVER], 'albums')).toEqual([
      { from: 'series/vol-2', to: 'albums/vol-2' },
      { from: 'series/cover.jpg', to: 'albums/cover.jpg' },
    ]);
    expect(pairsOf([VOL1], '')).toEqual([{ from: 'series/vol-1', to: 'vol-1' }]);
  });

  it('follows a path to where a pair moved it, or the folder above it', () => {
    const pairs = [{ from: 'series/vol-1', to: 'albums/vol-1' }];
    expect(movedPath('series/vol-1', pairs)).toBe('albums/vol-1');
    expect(movedPath('series/vol-1/p1.jpg', pairs)).toBe('albums/vol-1/p1.jpg');
    expect(movedPath('series/vol-10', pairs)).toBe('series/vol-10');
  });
});

describe('what the confirmation says', () => {
  it('names one row, or a count, and where it goes', () => {
    expect(moveTitle(['Vol. 03'], 'comics/Series A')).toBe('Move “Vol. 03” to “Series A”?');
    expect(moveTitle(['a', 'b', 'c'], '')).toBe('Move 3 items to the Library?');
  });

  it('says what a plain move does and offers it', () => {
    const question = moveQuestionOf(preview(), ['vol-1'], 'albums');
    expect(question.title).toBe('Move “vol-1” to “albums”?');
    expect(question.lines).toEqual([
      '12 files will be given a new path in the Library — nothing is uploaded',
      '12 files on this device follow along',
    ]);
    expect(question.leaving).toBeNull();
    expect(question.refused).toEqual([]);
    expect(question.moves).toBe(true);
    expect(moveQuestionOf(preview({ following: 0 }), ['vol-1'], 'albums').lines).toHaveLength(1);
  });

  it('says how many of this device’s files leave it, and their size', () => {
    const question = moveQuestionOf(preview({ following: 0, leaving: 196, leaving_bytes: 1_200_000_000 }), ['vol-1'], '');
    expect(question.leaving).toBe(
      '196 files (1.2 GB) on this device will be removed from it; opening them fetches them again',
    );
    expect(question.moves).toBe(true);
    expect(leavingLine({ leaving: 1, leaving_bytes: 900 })).toBe(
      '1 file (900 B) on this device will be removed from it; opening it fetches it again',
    );
  });

  it('says why a refused move is refused, and offers Cancel alone', () => {
    const refusal = (message: string) => ({ kind: 'refused_move' as const, message, reason: null, surfaced: null });
    const question = moveQuestionOf(
      preview({
        entries: 0,
        refused: [refusal('albums/vol-1 is there already'), refusal('albums/vol-1 is there already'), refusal('p1.jpg changed')],
      }),
      ['vol-1'],
      'albums',
    );
    expect(question.refused).toEqual(['albums/vol-1 is there already', 'p1.jpg changed']);
    expect(question.lines).toEqual([]);
    expect(question.moves).toBe(false);
    // Nothing refused and nothing to move is not a move either.
    expect(moveQuestionOf(preview({ entries: 0 }), ['vol-1'], 'albums').refused).toEqual([NOTHING_TO_MOVE]);
  });
});

describe('a move asked about', () => {
  const ran = { run: 1, work: { moves: [] } } as unknown as MoveAccepted;

  function moving(answer: boolean, over: Partial<Parameters<typeof askToMove>[0]> = {}) {
    return {
      pairs: preview().pairs,
      names: ['vol-1'],
      landing: 'albums',
      preview: vi.fn(async () => preview()),
      ask: vi.fn(async () => answer),
      move: vi.fn(async () => ran),
      moved: vi.fn(),
      refuse: vi.fn(),
      ...over,
    };
  }

  it('moves only on Move', async () => {
    const yes = moving(true);
    expect(await askToMove(yes)).toBe('moved');
    expect(yes.move).toHaveBeenCalledWith(preview().pairs);
    expect(yes.moved).toHaveBeenCalledWith(ran, preview().pairs);

    const no = moving(false);
    expect(await askToMove(no)).toBe('cancelled');
    expect(no.move).not.toHaveBeenCalled();
  });

  it('moves nothing the preview refused, and says a refused request', async () => {
    const refused = moving(true, {
      preview: vi.fn(async () =>
        preview({ refused: [{ kind: 'refused_move', message: 'taken', reason: null, surfaced: null }] }),
      ),
    });
    expect(await askToMove(refused)).toBe('cancelled');
    expect(refused.move).not.toHaveBeenCalled();

    const failing = moving(true, { move: vi.fn(async () => Promise.reject(new Error('gone'))) });
    expect(await askToMove(failing)).toBe('refused');
    expect(failing.refuse).toHaveBeenCalled();
    expect(failing.moved).not.toHaveBeenCalled();
  });

  it('says a refusal of one row by its name, and of several as a count', () => {
    expect(notMovedLine(['vol-1'], 'gone')).toBe('“vol-1” was not moved — gone');
    expect(notMovedLine(['vol-1', 'vol-2', 'vol-3'], 'gone')).toBe('3 items were not moved — gone');
  });
});

describe('what Rename… refuses', () => {
  it('refuses a name that cannot be one', () => {
    expect(renameDefect('', VOL2, [])).toBe('a name is needed');
    expect(renameDefect('   ', VOL2, [])).toBe('a name is needed');
    expect(renameDefect('a/b', VOL2, [])).toBe('a name cannot hold a “/”');
    expect(renameDefect('.', VOL2, [])).toBe('“.” and “..” are not names');
    expect(renameDefect('..', VOL2, [])).toBe('“.” and “..” are not names');
    expect(renameDefect('.coffret', VOL2, [])).toMatch(/coffret keeps its own records/);
  });

  it('refuses the name of another row in the same folder, a folder or a file or a prepared one', () => {
    expect(renameDefect('vol-1', VOL2, [])).toBe('there is already something called vol-1 here');
    expect(renameDefect('cover.jpg', VOL2, [])).toBe('there is already something called cover.jpg here');
    expect(renameDefect('vol-9', VOL2, ['series/vol-9'])).toBe('there is already something called vol-9 here');
    expect(renameDefect('vol-3', VOL2, [])).toBeNull();
  });

  it('renames to nothing where the name is the one it has', () => {
    expect(renameDefect('vol-2', VOL2, [])).toBeNull();
    expect(renamePair(VOL2, ' vol-2 ')).toBeNull();
    expect(renamePair(VOL2, ' vol-3 ')).toEqual({ from: 'series/vol-2', to: 'series/vol-3' });
    expect(renamePair(SERIES_ROW, 'shelf')).toEqual({ from: 'series', to: 'shelf' });
  });

  it('says first the files that would leave this device, and goes ahead once said', () => {
    const leaves = preview({ leaving: 3, leaving_bytes: 3000 });
    expect(renameStepOf(leaves, false)).toEqual({
      leaving: '3 files (3.0 kB) on this device will be removed from it; opening them fetches them again',
    });
    expect(renameStepOf(leaves, true)).toBe('go');
    expect(renameStepOf(preview(), false)).toBe('go');
    expect(
      renameStepOf(preview({ refused: [{ kind: 'refused_move', message: 'edited', reason: null, surfaced: null }] }), true),
    ).toEqual({ blocked: 'edited' });
  });
});

describe('the folder tree Move to… offers', () => {
  const FOLDERS = ['albums', 'series', 'series/vol-1', 'series/vol-1/extras', 'series/vol-2'];

  function drawn(expanded: string[], moved: Described[]) {
    return moveTargets(FOLDERS, new Set(expanded), moved).map(
      (row) => `${'  '.repeat(row.depth)}${row.name}${row.expandable ? (row.expanded ? ' v' : ' >') : ''}${row.takes ? '' : ' (x)'}`,
    );
  }

  it('starts with the Library, then the open folders in the Library’s order', () => {
    expect(drawn([], [COVER])).toEqual(['Library', '  albums', '  series > (x)']);
    expect(drawn(['series'], [COVER])).toEqual([
      'Library',
      '  albums',
      '  series v (x)',
      '    vol-1 >',
      '    vol-2',
    ]);
  });

  it('takes nothing where the move would do nothing, and does not open a folder being moved', () => {
    expect(drawn(['series', 'series/vol-1'], [VOL1])).toEqual([
      'Library',
      '  albums',
      '  series v (x)',
      '    vol-1 (x)',
      '    vol-2',
    ]);
    expect(drawn([], [ALBUMS])).toEqual(['Library (x)', '  albums (x)', '  series >']);
  });

  it('names a folder held only by its descendants, and opens to where the rows stand', () => {
    expect(moveTargets(['deep/down'], new Set(['deep']), [COVER]).map((row) => row.path)).toEqual([
      '',
      'deep',
      'deep/down',
    ]);
    expect([...openedFor([PAGE])].sort()).toEqual(['series', 'series/vol-1']);
  });
});

describe('a move under way, drawn at its new place (spec: LA-31)', () => {
  const pairs = [{ from: 'series/vol-1', to: 'albums/vol-1' }];
  const pending: PendingMove = { run: 3, pairs, waits: false };
  const ALBUM = listing('albums', { files: [file('albums/a.jpg')] });
  const held = new Map<string, Listing>([
    ['', ROOT],
    ['series', SERIES],
    ['albums', ALBUM],
  ]);
  const on = (listings: ReadonlyMap<string, Listing>) => (folder: string) => listings.get(folder) ?? null;

  /** The tree's rows as `{ key, moving }`, with `series` and `albums` open. */
  function drawn(followed: Parameters<typeof drawnMoves>[0], listings: ReadonlyMap<string, Listing> = held) {
    return treeRows({
      listing: (folder) => {
        const value = listings.get(folder);
        return value === undefined ? undefined : { status: 'ready', value };
      },
      expanded: new Set(['series', 'albums']),
      prospective: [],
      stranded: [],
      moves: drawnMoves(followed),
    }).map((row) => ({ key: row.key, moving: row.kind !== 'note' && row.moving === true }));
  }

  function ended(status: 'done' | 'stopped'): Logged {
    const run: Move =
      status === 'done'
        ? { run: 3, pairs, entries: 1, relocated: 1, left: 0, unrelocated: null, findings: [], status, stopped: null }
        : {
            run: 3,
            pairs,
            entries: 0,
            relocated: 0,
            left: 0,
            unrelocated: null,
            findings: [],
            status,
            stopped: {
              kind: 'refused_move',
              reason: 'edited',
              surfaced: null,
              message: 'this device\'s file "series/vol-1/p1.jpg" changed on this device since it was placed',
            },
          };
    return { id: 9, at: '2026-11-10T10:00:00Z', settled: false, kind: 'move', run };
  }

  /** The move as this page follows it from Move pressed, with the rows it drew. */
  const pressed = [{ run: 3, pairs, rows: movedRowsFrom([VOL1]), state: 'moving' as const }];

  it('draws the moved rows at their new place as being moved, and not at their old place, while it is pending', () => {
    const followed = followedMoves(pressed, [pending], [], on(held));
    expect(followed).toBe(pressed);
    const rows = drawn(followed);
    expect(rows).toContainEqual({ key: 'folder:albums/vol-1', moving: true });
    expect(rows.some((row) => row.key === 'folder:series/vol-1')).toBe(false);
    // Closed while it is being moved: nothing under it is at the new place yet.
    expect(rows.some((row) => row.key === 'file:albums/vol-1/p1.jpg')).toBe(false);
  });

  it('draws a move asked for elsewhere out of the listing of where it stood', () => {
    const followed = followedMoves([], [pending], [], on(held));
    expect(drawn(followed)).toContainEqual({ key: 'folder:albums/vol-1', moving: true });
  });

  it('draws them as ordinary rows once it has committed, until the listings say so themselves', () => {
    const committed = followedMoves(pressed, [], [ended('done')], on(held));
    expect(committed[0].state).toBe('moved');
    const rows = drawn(committed);
    expect(rows).toContainEqual({ key: 'folder:albums/vol-1', moving: false });
    expect(rows.some((row) => row.key === 'folder:series/vol-1')).toBe(false);

    // The listings answered again: the Library has it at the new place, and
    // the move is not drawn any longer.
    const answered = new Map(held)
      .set('series', listing('series', { folders: [folder('series/vol-2')], files: [file('series/cover.jpg')] }))
      .set('albums', listing('albums', { folders: [folder('albums/vol-1')], files: [file('albums/a.jpg')] }));
    const settled = followedMoves(committed, [], [ended('done')], on(answered));
    expect(settled).toEqual([]);
    const after = drawn(settled, answered);
    expect(after).toContainEqual({ key: 'folder:albums/vol-1', moving: false });
    expect(after.some((row) => row.key === 'folder:series/vol-1')).toBe(false);
  });

  it('puts them back where they were, and the Activity panel says why, where it was refused before its commit', () => {
    const refused = followedMoves(pressed, [], [ended('stopped')], on(held));
    expect(refused).toEqual([]);
    const rows = drawn(refused);
    expect(rows).toContainEqual({ key: 'folder:series/vol-1', moving: false });
    expect(rows.some((row) => row.key === 'folder:albums/vol-1')).toBe(false);

    const [entry] = earlier([ended('stopped')], [], false);
    expect(entry.outcome).toBe('failure');
    expect(entry.text).toBe(
      'could not move series/vol-1 → albums/vol-1 — this device\'s file "series/vol-1/p1.jpg" changed on this device since it was placed',
    );
  });

  it('says on its line in progress what it moves and where to, and what it waits for', () => {
    expect(movingLine(pending, 'move')).toBe('moving series/vol-1 → albums/vol-1…');
    expect(movingLine({ ...pending, waits: true }, 'freeze')).toBe(
      'moving series/vol-1 → albums/vol-1 — waiting for the packing under way to finish',
    );
    expect(movingLine({ run: 4, pairs: [...pairs, { from: 'x', to: 'y' }], waits: true }, 'move')).toBe(
      'moving 2 items — waiting for the move under way to finish',
    );
    const lines = inProgress({
      adding: null,
      addingAs: null,
      adds: [],
      moves: [pending],
      sent: null,
      turn: 'move',
      fill: null,
      sync: null,
      freeze: null,
      deletion: null,
      fetching: null,
    });
    expect(lines).toEqual([
      { key: 'move:3', text: 'moving series/vol-1 → albums/vol-1…', progress: null, stop: null, ending: null },
    ]);
  });
});
