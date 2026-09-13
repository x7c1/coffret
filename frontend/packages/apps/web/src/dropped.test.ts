import { expect, it, vi } from 'vitest';

import { Refusal, type Upload } from '@coffret/api';

import { askToAdd, brokeOff } from './dropped';

/** One drop, with everything it reaches out to recorded in the order it went. */
function dropping(ask: () => Promise<Upload>) {
  const order: string[] = [];
  const notice: (string | null)[] = [];
  return {
    order,
    notice,
    run: () =>
      askToAdd({
        ask,
        notice: (line) => notice.push(line),
        reload: () => order.push('reload'),
        follow: () => order.push('follow'),
      }),
  };
}

/** What the client mints for a `fetch` that rejected without being aborted. */
function broken(): Refusal {
  return new Refusal('unreachable', 0, 'the coffret server did not answer');
}

// The ordinary drop. The listing is what puts the new rows on the screen, and
// the activity is followed because the server armed a sync or a freeze as it
// answered — nothing on this page has asked for the activity since, so it is
// told there is something to follow.
it('asks the folder again and follows what a drop that landed armed', async () => {
  const run = dropping(() => Promise.resolve({ written: ['albums/one.jpg'], refused: [] }));

  await run.run();

  expect(run.order).toEqual(['follow', 'reload']);
  expect(run.notice).toEqual([null]);
});

// An answer that refused every part of it. The folder is still asked again —
// this answer says nothing landed, but the question costs one local read and is
// the same question the two paths beside it ask — and nothing is followed,
// because nothing was armed.
it('says what was refused, names how many more, and follows nothing', async () => {
  const run = dropping(() =>
    Promise.resolve({
      written: [],
      refused: [
        {
          name: 'page-001.jpg',
          error: 'declined',
          message: 'the Library holds this inside a Pack',
          reason: 'pack_resident',
        },
        {
          name: 'page-002.jpg',
          error: 'declined',
          message: 'the Library holds this inside a Pack',
          reason: 'pack_resident',
        },
      ],
    }),
  );

  await run.run();

  expect(run.order).toEqual(['reload']);
  expect(run.notice).toEqual([
    null,
    'page-001.jpg — the Library holds this inside a Pack (and 1 more)',
  ]);
});

// One answer carrying both halves, which is the ordinary shape of a drop of
// many files: they are separate questions, so what landed and what was refused
// come back together. Both halves are acted on — the refusal is said, and the
// activity is followed all the same, because something did land and the server
// armed a flow behind it. An answer read as one or the other would leave the
// files that landed with nothing following them in.
it('says what was refused and still follows what landed beside it', async () => {
  const run = dropping(() =>
    Promise.resolve({
      written: ['albums/page-001.jpg'],
      refused: [
        {
          name: 'page-002.jpg',
          error: 'declined',
          message: 'the Library holds this inside a Pack',
          reason: 'pack_resident',
        },
      ],
    }),
  );

  await run.run();

  expect(run.order).toEqual(['follow', 'reload']);
  // And one refused part is named on its own, with no count of a rest there
  // is none of.
  expect(run.notice).toEqual([null, 'page-002.jpg — the Library holds this inside a Pack']);
});

// The one this module exists for: a transfer that breaks is not proof that
// nothing arrived. Those files are in the folder, and the listing is the only
// thing that will show them; a screen that asked again only where the request
// answered leaves somebody dropping the same files twice.
it('asks the folder again when the request itself was never answered', async () => {
  const run = dropping(() => Promise.reject(broken()));

  await run.run();

  expect(run.order).toEqual(['follow', 'reload']);
});

// And says so without claiming either of the two things it does not know: that
// the server is gone, or that the files were refused. What it knows is that the
// drop did not finish and that the folder is the thing to look at.
it('does not report a broken transfer as a server that did not answer', async () => {
  const line = brokeOff(broken());

  expect(line).not.toContain('did not answer');
  expect(line).not.toContain('refused');
  expect(line).toContain('did not finish sending');
});

// And it does not stop at the bad news. Saying only that the outcome is unknown
// is true and leaves a person who has just lost twenty minutes of sending with
// nothing to do next, which is the state the sentence before it left them in.
it('tells the person what to do about a drop that broke', async () => {
  const line = brokeOff(broken());

  expect(line).toContain('the rows below are the folder as it stands now');
  expect(line).toContain('can be dropped again');
});

// A refusal read off an answer that did arrive is the server's own sentence,
// written to be read, and is shown as it is written.
it('shows a refusal the server did answer with as the server wrote it', async () => {
  const run = dropping(() =>
    Promise.reject(
      new Refusal('bad_request', 413, 'that is more than this route takes: page-001.jpg'),
    ),
  );

  await run.run();

  expect(run.notice.at(-1)).toBe('that is more than this route takes: page-001.jpg');
  expect(run.order).toEqual(['follow', 'reload']);
});

// Anything that is not a refusal at all is this client's own mistake, and says
// so rather than showing whatever a stray value stringifies to.
it('says the explorer failed where what was thrown is not a refusal', async () => {
  const logged = vi.spyOn(console, 'error').mockImplementation(() => {});

  expect(brokeOff(new TypeError('files is not iterable'))).toBe(
    'the explorer could not finish that',
  );

  expect(logged).toHaveBeenCalledTimes(1);
  logged.mockRestore();
});
