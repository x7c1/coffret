import { describe, expect, it, vi } from 'vitest';

import type { Added } from '@coffret/api';

import {
  choiceLabels,
  chooseAndAdd,
  existsLine,
  folderLine,
  looseLine,
  questionOf,
  summarize,
  type Choice,
  type Choosing,
} from './packChoice';

/** One dropped file of `size` bytes at `path`, relative to the folder dropped onto. */
function added(path: string, size = 10): Added {
  // Only the size is read of a file before anything is sent, so a stand-in
  // holding just that is the file as far as this module is concerned — which
  // is what lets a case speak of a gigabyte without allocating one.
  return { path, file: { size } as File };
}

/** A book of `pages` pages of `size` bytes each under `folder`, some a level down. */
function book(folder: string, pages: number, size = 10): Added[] {
  return Array.from({ length: pages }, (_, at) =>
    added(at % 2 === 0 ? `${folder}/page-${at}.jpg` : `${folder}/scans/page-${at}.jpg`, size),
  );
}

describe('summarize', () => {
  // The count is every file at every depth under the dropped folder, so the
  // number shown is the number of parts the request will carry.
  it('counts every file under a dropped folder, however deep', () => {
    const files = [...book('BookX', 80, 725_000), added('BookX/a/b/c/deep.jpg', 0)];
    const summary = summarize(files, []);
    expect(summary).not.toBeNull();
    expect(summary?.folders).toEqual([
      { name: 'BookX', files: 81, bytes: 58_000_000, exists: false },
    ]);
    expect(summary?.files).toBe(files.length);
    expect(folderLine(summary!.folders[0])).toBe('📁 BookX — 81 files, 58.0 MB');
  });

  it('asks nothing of a drop of files alone', () => {
    expect(summarize([added('cover.png'), added('notes.txt')], [])).toBeNull();
  });

  // Several folders are listed each with its own totals, and loose files beside
  // them are counted into the drop the answer applies to.
  it('lists each folder of a drop and the loose files beside them', () => {
    const files = [...book('vol-1', 3), ...book('vol-2', 2), added('cover.png')];
    const summary = summarize(files, ['vol-2']);
    expect(summary?.folders.map((folder) => [folder.name, folder.files, folder.exists])).toEqual([
      ['vol-1', 3, false],
      ['vol-2', 2, true],
    ]);
    expect(summary?.loose).toBe(1);
    expect(summary?.files).toBe(6);
    expect(existsLine(summary!.folders[1])).toMatch(/^vol-2 already exists here — /);
    expect(questionOf(summary!)).toBe('this drop holds 2 folders — how should it go into the Library?');
    expect(looseLine(summary!)).toBe('and 1 file beside them, added the same way');
    expect(choiceLabels(summary!)).toEqual({
      pack: 'Add as a Pack (recommended)',
      one_by_one: 'Add the 6 files one by one',
      cancel: 'Cancel',
    });
  });
});

describe('chooseAndAdd', () => {
  /** A drop that answers the question with `choice`, recording what it was asked. */
  function choosing(files: Added[], choice: Choice = 'pack') {
    const order: string[] = [];
    const asking: Choosing = {
      files,
      existing: [],
      ask: vi.fn(async () => {
        order.push('asked');
        return choice;
      }),
      send: vi.fn(async (freeze: boolean) => {
        order.push(freeze ? 'sent as a Pack' : 'sent one by one');
      }),
      refuse: vi.fn(),
    };
    return { asking, order };
  }

  it('asks about a drop holding a folder before anything is sent', async () => {
    const { asking, order } = choosing(book('BookX', 4));
    expect(await chooseAndAdd(asking)).toBe('sent');
    expect(order).toEqual(['asked', 'sent as a Pack']);
    expect(asking.ask).toHaveBeenCalledWith(
      expect.objectContaining({ files: 4, folders: [expect.objectContaining({ files: 4 })] }),
    );
  });

  it('sends a drop of files alone one by one without asking', async () => {
    const { asking, order } = choosing([added('cover.png')]);
    expect(await chooseAndAdd(asking)).toBe('sent');
    expect(asking.ask).not.toHaveBeenCalled();
    expect(order).toEqual(['sent one by one']);
  });

  // Choosing Pack asks the server for a freeze; choosing one by one asks for
  // the sync, as a drop of files does.
  it('sends as a Pack or one by one as the person chose', async () => {
    const pack = choosing(book('BookX', 2), 'pack');
    await chooseAndAdd(pack.asking);
    expect(pack.asking.send).toHaveBeenCalledWith(true);

    const loose = choosing(book('BookX', 2), 'one_by_one');
    await chooseAndAdd(loose.asking);
    expect(loose.asking.send).toHaveBeenCalledWith(false);
  });

  it('sends nothing when the question is cancelled', async () => {
    const { asking } = choosing(book('BookX', 2), 'cancel');
    expect(await chooseAndAdd(asking)).toBe('cancelled');
    expect(asking.send).not.toHaveBeenCalled();
    expect(asking.refuse).not.toHaveBeenCalled();
  });

  // LA-9: a drop the server is certain to refuse is refused here, before the
  // question — a choice about how to add what cannot be added is no choice.
  it('refuses a drop past a budget without asking about it', async () => {
    const { asking } = choosing([...book('BookX', 2), added('BookX/huge.tif', 2 ** 31)]);
    expect(await chooseAndAdd(asking)).toBe('refused');
    expect(asking.ask).not.toHaveBeenCalled();
    expect(asking.send).not.toHaveBeenCalled();
    expect(asking.refuse).toHaveBeenCalledWith(
      expect.objectContaining({ budget: 'part', name: 'BookX/huge.tif' }),
    );
  });
});
