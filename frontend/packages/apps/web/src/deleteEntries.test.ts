import { describe, expect, it, vi } from 'vitest';

import { Refusal, type Delete, type DeletePreview, type Finding } from '@coffret/api';

import {
  askToDelete,
  deletedLine,
  deleteOffered,
  deletingLine,
  questionOf,
  refusedNotice,
  type DeleteQuestion,
  type Deleting,
} from './deleteEntries';

/** What the server answers for a folder of twelve files, two inside Packs. */
function preview(over: Partial<DeletePreview> = {}): DeletePreview {
  return {
    folders: ['albums'],
    paths: [],
    entries: 12,
    bytes: 340_000_000,
    removed: 10,
    rebuilt: 2,
    rebuild_read: 1_800_000_000,
    rebuild_written: 1_500_000_000,
    refused: [],
    missing: [],
    after_current: false,
    ...over,
  };
}

/** A deletion of `albums`, in whichever state `over` says. */
function run(over: Partial<Delete> = {}): Delete {
  return {
    run: 1,
    folders: ['albums'],
    paths: [],
    entries: 0,
    bytes: 0,
    removed: 0,
    rebuilt: 0,
    rebuild_read: 0,
    rebuild_written: 0,
    refused: [],
    missing: [],
    findings: [],
    step: null,
    waiting: 0,
    status: 'deleting',
    stopped: null,
    ...over,
  } as Delete;
}

const KEY_LOST = {
  spared: ['books/vol-1/page-002.jpg'],
  kept: 2,
  reason: 'key_lost' as const,
  message: 'the Library has no key for the Pack holding them',
};

describe('deleteOffered', () => {
  it('offers a file the Library holds, and not one only added here', () => {
    expect(deleteOffered({ container: 'one-file' })).toBe(true);
    expect(deleteOffered({ container: 'pack' })).toBe(true);
    expect(deleteOffered({ container: null })).toBe(false);
  });
});

describe('questionOf', () => {
  it('says what leaves the Library and what a rebuild costs', () => {
    const question = questionOf(preview());
    expect(question.title).toBe('Delete “albums”?');
    expect(question.removes).toBe('12 files, 340.0 MB will be removed from the Library');
    expect(question.rebuilds).toEqual({
      line: '2 Packs holding other files will be rebuilt',
      cost: 'Reads 1.8 GB, writes 1.5 GB',
    });
    expect(question.refused).toEqual([]);
    expect(question.deletes).toBe(true);
    expect(question.target).toEqual({ folders: ['albums'], paths: [] });
  });

  it('names one file by its name, and several rows as a count of items', () => {
    const one = questionOf(
      preview({ folders: [], paths: ['albums/cover.png'], entries: 1, bytes: 5, rebuilt: 0 }),
    );
    expect(one.title).toBe('Delete “cover.png”?');
    expect(one.removes).toBe('1 file, 5 B will be removed from the Library');

    const several = questionOf(preview({ folders: ['albums', 'books'], paths: ['scans/a.jpg'] }));
    expect(several.title).toBe('Delete 3 items?');
    expect(several.target).toEqual({ folders: ['albums', 'books'], paths: ['scans/a.jpg'] });
  });

  // PK-9, PK-10: only a Pack that keeps files the deletion does not name is
  // rebuilt — read whole and uploaded again — and only that is worth a note.
  // A Pack whose every file goes, and a one-file Container, are removed
  // outright, which costs nothing to speak of.
  it('notes a rebuild only where a Pack keeps other files', () => {
    expect(questionOf(preview({ rebuilt: 1, rebuild_read: 2_000_000, rebuild_written: 1_000_000 })).rebuilds).toEqual({
      line: '1 Pack holding other files will be rebuilt',
      cost: 'Reads 2.0 MB, writes 1.0 MB',
    });
    // Whole Packs and one-file Containers removed: nothing rebuilt.
    expect(
      questionOf(preview({ removed: 12, rebuilt: 0, rebuild_read: 0, rebuild_written: 0 })).rebuilds,
    ).toBeNull();
  });

  it('says what would be refused and why, and the files the Library does not hold', () => {
    const question = questionOf(
      preview({ refused: [KEY_LOST], missing: ['albums/gone.jpg'], after_current: true }),
    );
    expect(question.refused).toEqual([
      'page-002.jpg stays in the Library — the Library has no key for the Pack holding them',
    ]);
    expect(question.missing).toBe('not in the Library, so nothing to remove: gone.jpg');
    expect(question.waits).toBe('a deletion is running already — this one runs after it');
  });

  it('offers no Delete where nothing would be removed', () => {
    const question = questionOf(preview({ entries: 0, bytes: 0, refused: [KEY_LOST] }));
    expect(question.deletes).toBe(false);
    expect(question.removes).toBe('nothing will be removed from the Library');
  });
});

describe('askToDelete', () => {
  function deleting(answer: boolean, over: Partial<Deleting> = {}) {
    const asked: DeleteQuestion[] = [];
    const steps = {
      target: { folders: ['albums'], paths: [] },
      preview: vi.fn(() => Promise.resolve(preview())),
      ask: vi.fn((question: DeleteQuestion) => {
        asked.push(question);
        return Promise.resolve(answer);
      }),
      start: vi.fn(() => Promise.resolve()),
      refuse: vi.fn(),
      ...over,
    };
    return { steps, asked };
  }

  it('arms the deletion only on Delete', async () => {
    const yes = deleting(true);
    expect(await askToDelete(yes.steps)).toBe('armed');
    expect(yes.steps.start).toHaveBeenCalledWith({ folders: ['albums'], paths: [] });

    const no = deleting(false);
    expect(await askToDelete(no.steps)).toBe('cancelled');
    expect(no.steps.start).not.toHaveBeenCalled();
    expect(no.asked).toHaveLength(1);
  });

  it('arms nothing where there is nothing to delete, whatever is pressed', async () => {
    const nothing = deleting(true, {
      preview: vi.fn(() => Promise.resolve(preview({ entries: 0 }))),
    });
    expect(await askToDelete(nothing.steps)).toBe('cancelled');
    expect(nothing.steps.start).not.toHaveBeenCalled();
  });

  it('says a refused preview or a refused arming, and shows no question for the first', async () => {
    const refusal = new Error('the server is locked');
    const previewRefused = deleting(true, { preview: vi.fn(() => Promise.reject(refusal)) });
    expect(await askToDelete(previewRefused.steps)).toBe('refused');
    expect(previewRefused.asked).toHaveLength(0);
    expect(previewRefused.steps.refuse).toHaveBeenCalledWith(refusal);

    const startRefused = deleting(true, { start: vi.fn(() => Promise.reject(refusal)) });
    expect(await askToDelete(startRefused.steps)).toBe('refused');
    expect(startRefused.steps.refuse).toHaveBeenCalledWith(refusal);
  });
});

describe('the lines a deletion stands under', () => {
  it('says how far a running one has got', () => {
    expect(deletingLine(run())).toBe('deleting 📁 albums…');
    expect(
      deletingLine(
        run({ step: { phase: 'packing', done: 1, total: 2, bytes: null }, waiting: 1 }),
      ),
    ).toBe('deleting 📁 albums — rebuilding Packs 1/2 (1 more after it)');
    expect(deletedLine(run())).toBeNull();
  });

  // Several rows deleted together go by a count: of files where only files
  // were named, and of items where folders were too.
  it('names several rows by a count', () => {
    expect(deletingLine(run({ folders: [], paths: ['albums/a.jpg', 'albums/b.jpg'] }))).toBe('deleting 2 files…');
    expect(deletingLine(run({ folders: ['albums', 'books'], paths: ['c.jpg'] }))).toBe('deleting 3 items…');
  });

  it('says it is waiting while a book is being packed or a backup runs', () => {
    expect(deletingLine(run(), 'packing')).toBe(
      'deleting 📁 albums — waiting for the packing under way to finish',
    );
    expect(deletingLine(run(), 'backup')).toBe(
      'deleting 📁 albums — waiting for the backup under way to finish',
    );
    expect(deletingLine(run({ waiting: 1 }), 'deletion')).toBe(
      'deleting 📁 albums — waiting for the deletion under way to finish (1 more after it)',
    );
    const started = run({ step: { phase: 'committing', done: 0, total: null, bytes: null } });
    expect(deletingLine(started, 'packing')).toBe('deleting 📁 albums — committing');
  });

  // Once the commit has landed, what the deletion still does is on this
  // device, and the line says so rather than standing at `committing`.
  it('says it is removing the copies on this device after the commit', () => {
    const removing = run({
      step: { phase: 'removing_copies', done: 0, total: null, bytes: null },
    });
    expect(deletingLine(removing)).toBe("deleting 📁 albums — discarding this device's copies into the trash");
  });

  it('names what was deleted, and anything refused', () => {
    expect(
      deletedLine(
        run({
          status: 'done',
          entries: 12,
          bytes: 340_000_000,
          rebuilt: 2,
          refused: [KEY_LOST],
          missing: ['albums/gone.jpg'],
        }),
      ),
    ).toEqual({
      text:
        '📁 albums — 12 files, 340.0 MB removed from the Library; 2 Packs rebuilt; ' +
        'page-002.jpg stays in the Library — the Library has no key for the Pack holding them; ' +
        'not in the Library: gone.jpg',
      retry: null,
    });
  });

  it('says what became of this device’s copies: counted when discarded, named when kept', () => {
    const finding = (path: string | null, surfaced: Finding['surfaced'], message: string) =>
      ({ path, message, reason: surfaced === null ? 'root_missing' : 'surfaced', surfaced }) as Finding;
    expect(
      deletedLine(
        run({
          status: 'done',
          entries: 4,
          bytes: 4_000,
          findings: [
            finding('albums/a.jpg', 'Discarded', 'discarded'),
            finding('albums/b.jpg', 'Discarded', 'discarded'),
            finding('albums/c.jpg', 'KeptEdited', 'kept'),
            finding('albums/d.jpg', 'DiscardRefused', 'discarding it into the trash was refused'),
            finding(null, null, 'a folder this device maps is not there'),
          ],
        }),
      ),
    ).toEqual({
      text:
        '📁 albums — 4 files, 4.0 kB removed from the Library; ' +
        '2 files discarded into the trash on this device; ' +
        'kept on this device because it changed: c.jpg; ' +
        'd.jpg — discarding it into the trash was refused (and 1 more)',
      retry: null,
    });
  });

  it('says a stopped one in the server’s words, and offers a conflict again', () => {
    expect(
      deletedLine(
        run({
          status: 'stopped',
          stopped: {
            kind: 'conflict',
            message: 'another device changed the Library meanwhile',
            reason: null,
            surfaced: null,
          },
        }),
      ),
    ).toEqual({
      text: '📁 albums was not deleted — another device changed the Library meanwhile',
      retry: { folders: ['albums'], paths: [] },
    });
  });

  it('offers nothing again for a stop that asking again would only meet once more', () => {
    expect(
      deletedLine(
        run({
          status: 'stopped',
          stopped: {
            kind: 'epoch',
            message: 'enroll this device again',
            reason: null,
            surfaced: null,
          },
        }),
      ),
    ).toEqual({ text: '📁 albums was not deleted — enroll this device again', retry: null });
  });
});

describe('refusedNotice', () => {
  const target = { folders: [], paths: ['albums/a.jpg'] };

  it('offers a conflict again, for the same target', () => {
    const conflict = new Refusal('conflict', 409, 'another device changed the Library meanwhile');
    expect(refusedNotice(target, conflict, conflict.message)).toEqual({
      text: 'a.jpg was not deleted — another device changed the Library meanwhile',
      retry: { folders: [], paths: ['albums/a.jpg'] },
    });
  });

  it('offers nothing again for any other refusal', () => {
    const server = new Refusal('server', 500, 'the server could not finish that');
    expect(refusedNotice(target, server, server.message)).toEqual({
      text: 'a.jpg was not deleted — the server could not finish that',
      retry: null,
    });
    const stray = new Error('not a refusal');
    expect(refusedNotice(target, stray, 'the explorer could not finish that').retry).toBeNull();
  });
});
