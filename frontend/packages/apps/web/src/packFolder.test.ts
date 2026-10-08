import { describe, expect, it, vi } from 'vitest';

import type { FreezePreview } from '@coffret/api';

import { askToPack, packOffered, questionOf, type Packing, type PackQuestion } from './packFolder';

/** What the server answers for a folder of eighty one-file pages. */
function preview(over: Partial<FreezePreview> = {}): FreezePreview {
  return {
    folder: 'books/BookX',
    files: 80,
    bytes: 58_000_000,
    in_pack: 0,
    changed_in_pack: 0,
    not_here: 0,
    unavailable: 0,
    after_current: false,
    already_packing: false,
    ...over,
  };
}

describe('packOffered', () => {
  // EP-9: an unmapped folder has no file here for a freeze to read. And the
  // Library root's freeze would be the whole Library.
  it('offers a mapped folder, and neither an unmapped one nor the Library root', () => {
    expect(packOffered({ path: 'books/BookX', mapped: true })).toBe(true);
    expect(packOffered({ path: 'books/BookX', mapped: false })).toBe(false);
    expect(packOffered({ path: '', mapped: true })).toBe(false);
  });
});

describe('questionOf', () => {
  it('names the folder with what will be packed, as the drop names a dropped one', () => {
    const question = questionOf(preview());
    expect(question.line).toBe('📁 BookX — 80 files, 58.0 MB will be packed into Packs');
    expect(question.packs).toBe(true);
    expect(question.leftOut).toBeNull();
    expect(question.waits).toBeNull();
  });

  // A count smaller than the folder is not a surprise when the rest is named.
  it('says what is left as it is, in its broad groups', () => {
    const question = questionOf(preview({ in_pack: 3, changed_in_pack: 1, not_here: 2 }));
    expect(question.leftOut).toBe(
      'left as they are: 3 files already in a Pack, 1 file in a Pack and changed here, ' +
        '2 files not on this device',
    );
  });

  it('says a freeze already running goes first', () => {
    expect(questionOf(preview({ after_current: true })).waits).toBe(
      'a book is being packed already — this folder is packed after it',
    );
  });

  // The folder's own freeze is running or waiting: Pack would arm nothing, so
  // there is no later run to promise and nothing to press but Close.
  it('says a folder being packed already is, instead of offering Pack', () => {
    const question = questionOf(preview({ after_current: true, already_packing: true }));
    expect(question.line).toBe('📁 BookX — this folder is being packed already');
    expect(question.packs).toBe(false);
    expect(question.waits).toBeNull();
  });

  // Nothing selected: say so and why, and offer no Pack.
  it('says why there is nothing to pack instead of offering Pack', () => {
    const packed = questionOf(preview({ files: 0, bytes: 0, in_pack: 80 }));
    expect(packed.packs).toBe(false);
    expect(packed.line).toBe('📁 BookX — nothing to pack: 80 files already in a Pack');

    const remote = questionOf(preview({ files: 0, bytes: 0, not_here: 12 }));
    expect(remote.line).toBe('📁 BookX — nothing to pack: 12 files not on this device');

    const empty = questionOf(preview({ files: 0, bytes: 0 }));
    expect(empty.line).toBe('📁 BookX — nothing to pack: it holds no files');

    const unreachable = questionOf(preview({ files: 0, bytes: 0, unavailable: 1, not_here: 3 }));
    expect(unreachable.line).toBe(
      '📁 BookX — nothing to pack: the folder on this device it is mapped to cannot be reached right now',
    );
  });
});

describe('askToPack', () => {
  function packing(answer: boolean, over: Partial<Packing> = {}) {
    const asked: PackQuestion[] = [];
    const start = vi.fn(() => Promise.resolve());
    const refuse = vi.fn();
    const given: Packing = {
      folder: 'books/BookX',
      preview: () => Promise.resolve(preview()),
      ask: (question) => {
        asked.push(question);
        return Promise.resolve(answer);
      },
      start,
      refuse,
      ...over,
    };
    return { given, asked, start, refuse };
  }

  it('arms the freeze of the folder once Pack is chosen', async () => {
    const { given, asked, start } = packing(true);
    expect(await askToPack(given)).toBe('armed');
    expect(asked.map((question) => question.line)).toEqual([
      '📁 BookX — 80 files, 58.0 MB will be packed into Packs',
    ]);
    expect(start).toHaveBeenCalledWith('books/BookX');
  });

  it('arms nothing on Cancel', async () => {
    const { given, start } = packing(false);
    expect(await askToPack(given)).toBe('cancelled');
    expect(start).not.toHaveBeenCalled();
  });

  // Shown why, and nothing armed whatever is pressed.
  it('arms nothing for a folder with nothing to pack', async () => {
    const { given, asked, start } = packing(true, {
      preview: () => Promise.resolve(preview({ files: 0, bytes: 0, in_pack: 80 })),
    });
    expect(await askToPack(given)).toBe('cancelled');
    expect(asked[0].packs).toBe(false);
    expect(start).not.toHaveBeenCalled();
  });

  it('arms nothing for a folder being packed already', async () => {
    const { given, asked, start } = packing(true, {
      preview: () => Promise.resolve(preview({ after_current: true, already_packing: true })),
    });
    expect(await askToPack(given)).toBe('cancelled');
    expect(asked[0].packs).toBe(false);
    expect(start).not.toHaveBeenCalled();
  });

  it('says why the preview was refused, and asks nothing', async () => {
    const refusal = new Error('the folder is not mapped');
    const { given, asked, start, refuse } = packing(true, {
      preview: () => Promise.reject(refusal),
    });
    expect(await askToPack(given)).toBe('refused');
    expect(asked).toEqual([]);
    expect(start).not.toHaveBeenCalled();
    expect(refuse).toHaveBeenCalledWith(refusal);
  });

  it('says why the freeze was refused', async () => {
    const refusal = new Error('locked');
    const { given, refuse } = packing(true, { start: () => Promise.reject(refusal) });
    expect(await askToPack(given)).toBe('refused');
    expect(refuse).toHaveBeenCalledWith(refusal);
  });
});
