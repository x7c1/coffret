import { describe, expect, it, vi } from 'vitest';

import type { Delete, DeletePreview } from '@coffret/api';

import {
  askToDelete,
  deletedLine,
  deleteOffered,
  deletingLine,
  questionOf,
  type DeleteQuestion,
  type Deleting,
} from './deleteEntries';

/** What the server answers for a folder of twelve files, two inside Packs. */
function preview(over: Partial<DeletePreview> = {}): DeletePreview {
  return {
    folder: 'albums',
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
    folder: 'albums',
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
    expect(question.title).toBe('Delete 📁 albums?');
    expect(question.removes).toBe('12 files, 340.0 MB will be removed from the Library');
    expect(question.rebuilds).toBe(
      '2 Packs holding other files will be rebuilt — reads 1.8 GB, writes 1.5 GB',
    );
    expect(question.refused).toEqual([]);
    expect(question.deletes).toBe(true);
    expect(question.target).toEqual({ folder: 'albums', paths: [] });
  });

  it('names one file by its name, and says nothing of a rebuild where there is none', () => {
    const question = questionOf(
      preview({ folder: null, paths: ['albums/cover.png'], entries: 1, bytes: 5, rebuilt: 0 }),
    );
    expect(question.title).toBe('Delete cover.png?');
    expect(question.removes).toBe('1 file, 5 B will be removed from the Library');
    expect(question.rebuilds).toBeNull();
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
      target: { folder: 'albums', paths: [] },
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
    expect(yes.steps.start).toHaveBeenCalledWith({ folder: 'albums', paths: [] });

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
    ).toBe(
      '📁 albums — 12 files, 340.0 MB removed from the Library; 2 Packs rebuilt; ' +
        'page-002.jpg stays in the Library — the Library has no key for the Pack holding them; ' +
        'not in the Library: gone.jpg',
    );
  });

  it('says a stopped one in the server’s words, a conflict among them', () => {
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
    ).toBe('📁 albums was not deleted — another device changed the Library meanwhile');
  });
});
