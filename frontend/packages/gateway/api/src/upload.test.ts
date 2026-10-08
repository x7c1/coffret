import { afterEach, expect, it, vi } from 'vitest';

import { isRefusal } from './refusal';
import { addFiles, isOverBudget, uploadOf, type Added } from './upload';
import uploadBudget from './upload-budget.json';

afterEach(() => {
  vi.unstubAllGlobals();
  FakeRequest.made = [];
});

type Listener = (event: unknown) => void;

/** Listeners by event type, the part of an `EventTarget` the upload uses. */
class Listeners {
  private readonly byType = new Map<string, Listener[]>();

  addEventListener(type: string, listener: Listener): void {
    this.byType.set(type, [...(this.byType.get(type) ?? []), listener]);
  }

  fire(type: string, event: unknown = { type }): void {
    for (const listener of this.byType.get(type) ?? []) {
      listener(event);
    }
  }
}

/**
 * An `XMLHttpRequest` that sends nothing, standing where the browser's would.
 *
 * The test environment has no `XMLHttpRequest`, and one that did would not
 * report upload progress for a body going nowhere — so this records what it was
 * asked to send, and a case drives what the browser would have said back:
 * the body going up, an answer, a transfer that broke.
 */
class FakeRequest extends Listeners {
  static made: FakeRequest[] = [];

  readonly upload = new Listeners();
  readonly headers: Record<string, string> = {};
  method: string | null = null;
  url: string | null = null;
  body: unknown = undefined;
  responseType = '';
  status = 0;
  responseText = '';
  aborted = false;

  open(method: string, url: string): void {
    this.method = method;
    this.url = url;
  }

  setRequestHeader(name: string, value: string): void {
    this.headers[name] = value;
  }

  send(body: unknown): void {
    this.body = body;
    FakeRequest.made.push(this);
  }

  abort(): void {
    this.aborted = true;
    this.fire('abort');
  }

  /** The browser saying how much of the body has gone. */
  sent(loaded: number, total: number, lengthComputable = true): void {
    this.upload.fire('progress', { loaded, total, lengthComputable });
  }

  /** An answer that arrived whole. */
  answer(status: number, text: string): void {
    this.status = status;
    this.responseText = text;
    this.fire('load');
  }

  /** The one request made since the case began. */
  static only(): FakeRequest {
    expect(FakeRequest.made).toHaveLength(1);
    return FakeRequest.made[0];
  }
}

/** A drop of two real pages, which is what the transport is handed. */
function pages(): Added[] {
  return [
    { path: 'page-001.jpg', file: new File(['one'], 'page-001.jpg', { lastModified: 1_444_000_000_999 }) },
    { path: 'page-002.jpg', file: new File(['two'], 'page-002.jpg', { lastModified: -1_500 }) },
  ];
}

/** Lets the answer's reading, which is asynchronous, run to its end. */
function settled<T>(promise: Promise<T>): Promise<T | unknown> {
  return promise.catch((thrown: unknown) => thrown);
}

/**
 * A file that says it is `size` bytes long and holds none of them.
 *
 * Which is all a drop past the budget has to be to be refused before it is
 * sent: what is weighed is what the files say they come to, and a case that
 * had to hold 64 GiB to say so would be the case that could not be run.
 */
function claiming(path: string, size: number): Added {
  return { path, file: { size, name: path } as File };
}

/** What `addFiles` threw, or the answer it resolved to where it threw nothing. */
function thrownBy(files: Added[]): Promise<unknown> {
  return addFiles('books', files).catch((refusal: unknown) => refusal);
}

// LA-9, LA-10: a drop whose files already come to more than the request budget
// is refused before anything is sent (`addFiles` says why), saying which budget
// and by how much, and nothing landed.
it('refuses a drop past the request budget before sending it', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);

  // As many files as one part may be large as come to the budget, and one byte
  // more: no file in it passes a budget of its own.
  const files = Array.from(
    { length: uploadBudget.request_bytes / uploadBudget.part_bytes },
    (_, at) => claiming(`page-${at}.jpg`, uploadBudget.part_bytes),
  );
  files.push(claiming('colophon.txt', 1));
  const thrown = await thrownBy(files);

  expect(FakeRequest.made).toEqual([]);
  expect(isOverBudget(thrown)).toBe(true);
  if (isOverBudget(thrown)) {
    expect(thrown.kind).toBe('bad_request');
    expect(thrown.status).toBe(413);
    expect(thrown.written).toEqual([]);
    expect(thrown.overdrawn).toEqual({
      budget: 'request',
      carried: uploadBudget.request_bytes + 1,
      limit: uploadBudget.request_bytes,
    });
  }
});

// LA-9, LA-10: one file larger than one part may be is refused before anything
// is sent, naming that file — the server would have refused it part way, after
// the files ahead of it had landed.
it('refuses a drop holding one file past the part budget before sending it, naming it', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);

  const thrown = await thrownBy([
    claiming('page-001.jpg', 3),
    claiming('film/holiday.mov', uploadBudget.part_bytes + 1),
  ]);

  expect(FakeRequest.made).toEqual([]);
  expect(isOverBudget(thrown)).toBe(true);
  if (isOverBudget(thrown)) {
    expect(thrown.written).toEqual([]);
    expect(thrown.overdrawn).toEqual({
      budget: 'part',
      name: 'film/holiday.mov',
      size: uploadBudget.part_bytes + 1,
      limit: uploadBudget.part_bytes,
    });
    expect(thrown.message).toContain('film/holiday.mov');
  }
});

// LA-9, LA-10: more files than one request may carry parts for is refused
// before anything is sent, whatever they come to.
it('refuses a drop of more files than the parts budget before sending it', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);

  const files = Array.from({ length: uploadBudget.parts + 1 }, (_, at) =>
    claiming(`page-${at}.jpg`, 1),
  );
  const thrown = await thrownBy(files);

  expect(FakeRequest.made).toEqual([]);
  expect(isOverBudget(thrown)).toBe(true);
  if (isOverBudget(thrown)) {
    expect(thrown.written).toEqual([]);
    expect(thrown.overdrawn).toEqual({
      budget: 'parts',
      count: uploadBudget.parts + 1,
      limit: uploadBudget.parts,
    });
  }
});

// The file to take out is said before the count to halve: halving a drop that
// still holds a file no part can carry is halving one refused again.
it('names the file past the part budget before the count past the parts budget', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);

  const files = Array.from({ length: uploadBudget.parts + 1 }, (_, at) =>
    claiming(`page-${at}.jpg`, 1),
  );
  files.push(claiming('film/holiday.mov', uploadBudget.part_bytes + 1));
  const thrown = await thrownBy(files);

  expect(isOverBudget(thrown) && thrown.overdrawn.budget).toBe('part');
});

// Both per-file budgets are boundaries the server holds inclusively: a file of
// exactly one part's budget, among exactly as many files as there may be parts,
// is the server's to take.
it('sends a drop at exactly the part and parts budgets', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);
  vi.stubGlobal(
    'FormData',
    class {
      append() {}
    },
  );

  const files = Array.from({ length: uploadBudget.parts - 1 }, (_, at) =>
    claiming(`page-${at}.jpg`, 1),
  );
  files.push(claiming('film/holiday.mov', uploadBudget.part_bytes));
  const added = addFiles('books', files);
  FakeRequest.only().answer(200, JSON.stringify({ written: [], refused: [] }));

  await expect(added).resolves.toEqual({ written: [], refused: [] });
});

// The request budget is a boundary: files that come to exactly it are the
// server's to weigh, because the framing on top is what decides, and only the
// server sees the body the browser makes. Each file is no more than one part
// may be, so it is the request budget alone being weighed.
it('sends a drop whose files come to no more than the request budget', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);
  vi.stubGlobal(
    'FormData',
    class {
      append() {}
    },
  );

  const files = Array.from(
    { length: uploadBudget.request_bytes / uploadBudget.part_bytes },
    (_, at) => claiming(`page-${at}.jpg`, uploadBudget.part_bytes),
  );
  const added = addFiles('books', files);
  FakeRequest.only().answer(200, JSON.stringify({ written: [], refused: [] }));

  await expect(added).resolves.toEqual({ written: [], refused: [] });
});

// The request is the one `fetch` made before it: the route with the folder and
// the freeze in the query, the files as a multipart body, and no header of the
// page's own — the server's key is added by whatever serves the page.
it('sends the drop as it always did, and says how much of it has gone', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);
  const progress = vi.fn();

  const added = addFiles('books/vol-1', pages(), { freeze: true, onProgress: progress });
  const request = FakeRequest.only();
  expect(request.method).toBe('POST');
  expect(request.url).toBe('/api/upload?path=books%2Fvol-1&freeze=true');
  expect(request.headers).toEqual({});
  expect(request.body).toBeInstanceOf(FormData);
  const body = request.body as FormData;
  // Each file under its own modification time, which the server stamps the
  // file it writes with — negative before 1970.
  expect([...body.entries()].map(([field, part]) => [field, (part as File).name])).toEqual([
    ['1444000000999', 'page-001.jpg'],
    ['-1500', 'page-002.jpg'],
  ]);

  request.sent(100, 400);
  // A browser that does not know the whole length has nothing to say about
  // how far through it is, and a share of an unknown total is not passed on.
  request.sent(150, 0, false);
  request.sent(400, 400);
  request.answer(200, JSON.stringify({ written: ['books/vol-1/page-001.jpg'], refused: [] }));

  await expect(added).resolves.toEqual({ written: ['books/vol-1/page-001.jpg'], refused: [] });
  expect(progress.mock.calls).toEqual([
    [100, 400],
    [400, 400],
  ]);
});

// The Library root is the route with no `path` at all, as before.
it('sends a drop onto the Library root without a path', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);

  const added = addFiles('', pages());
  const request = FakeRequest.only();
  expect(request.url).toBe('/api/upload');
  request.answer(200, JSON.stringify({ written: [], refused: [] }));
  await added;
});

// A refusal of the whole drop is read as every refused request is — kind,
// reason, the server's sentence, and what had landed before it stopped.
it('reads a refused drop as every refused request is read', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);

  const added = settled(addFiles('albums', pages()));
  FakeRequest.only().answer(
    409,
    JSON.stringify({
      error: 'refused_placement',
      message: 'no folder on this device holds this part of the Library',
      reason: 'unmapped',
      written: ['albums/page-001.jpg'],
    }),
  );

  const thrown = await added;
  expect(isRefusal(thrown)).toBe(true);
  if (isRefusal(thrown)) {
    expect(thrown.kind).toBe('refused_placement');
    expect(thrown.status).toBe(409);
    expect(thrown.reason).toBe('unmapped');
    expect(thrown.message).toBe('no folder on this device holds this part of the Library');
    expect(thrown.written).toEqual(['albums/page-001.jpg']);
  }
});

// Something other than the server answering in its place is said to be, with
// the status it replied with.
it('says an answer that is not the server’s is not', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);

  const refused = settled(addFiles('albums', pages()));
  FakeRequest.only().answer(502, '<html>bad gateway</html>');
  const thrown = await refused;
  expect(isRefusal(thrown) && thrown.kind).toBe('unrecognized');
  expect(isRefusal(thrown) && thrown.status).toBe(502);

  FakeRequest.made = [];
  const garbled = settled(addFiles('albums', pages()));
  FakeRequest.only().answer(200, 'not json');
  const read = await garbled;
  expect(isRefusal(read) && read.kind).toBe('unrecognized');
});

// A transfer that broke is `unreachable`, in the words that do not claim the
// server is gone: it may have answered while the body was still going up.
it('says a transfer that broke off as one that broke off', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);

  const added = settled(addFiles('albums', pages()));
  FakeRequest.only().fire('error');

  const thrown = await added;
  expect(isRefusal(thrown)).toBe(true);
  if (isRefusal(thrown)) {
    expect(thrown.kind).toBe('unreachable');
    expect(thrown.status).toBe(0);
    expect(thrown.message).toMatch(/broke off while it was being sent/);
  }
});

// An abort is the caller's own tidying up and passes through as itself, as it
// does out of `fetch`: the signal's reason, and no refusal.
it('passes an abort through as the abort', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);
  const controller = new AbortController();

  const added = settled(addFiles('albums', pages(), { signal: controller.signal }));
  const request = FakeRequest.only();
  controller.abort();

  const thrown = await added;
  expect(request.aborted).toBe(true);
  expect(isRefusal(thrown)).toBe(false);
  expect(thrown).toBe(controller.signal.reason);
});

// And a signal aborted before the drop was made sends nothing at all.
it('sends nothing for a drop already aborted', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);
  const controller = new AbortController();
  controller.abort();

  const thrown = await settled(addFiles('albums', pages(), { signal: controller.signal }));
  expect(FakeRequest.made).toEqual([]);
  expect(thrown).toBe(controller.signal.reason);
});

// A part the drop refused is read as a refused request is: a kind this client
// has not heard of is `unrecognized`, and a reason or a finding name it has not
// heard of is `null`, rather than a string claiming the union.
it('narrows each refused part as a refused request is narrowed', () => {
  expect(
    uploadOf({
      written: ['books/one.jpg'],
      refused: [
        { name: 'two.jpg', error: 'quota', message: 'a kind this page has never heard of' },
        {
          name: 'three.jpg',
          error: 'declined',
          message: 'a reason this page has never heard of',
          reason: 'elsewhere',
          surfaced: 'SomethingNew',
        },
        {
          name: '.coffret/four.jpg',
          error: 'declined',
          message: 'a name coffret keeps',
          reason: 'reserved',
        },
      ],
    }),
  ).toEqual({
    written: ['books/one.jpg'],
    refused: [
      {
        name: 'two.jpg',
        kind: 'unrecognized',
        message: 'a kind this page has never heard of',
        reason: null,
        surfaced: null,
      },
      {
        name: 'three.jpg',
        kind: 'declined',
        message: 'a reason this page has never heard of',
        reason: null,
        surfaced: null,
      },
      {
        name: '.coffret/four.jpg',
        kind: 'declined',
        message: 'a name coffret keeps',
        reason: 'reserved',
        surfaced: null,
      },
    ],
  });
});
