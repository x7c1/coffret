import { afterEach, expect, it, vi } from 'vitest';

import { isRefusal } from './refusal';
import { addFiles, uploadOf, type Added } from './upload';
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
    { path: 'page-001.jpg', file: new File(['one'], 'page-001.jpg') },
    { path: 'page-002.jpg', file: new File(['two'], 'page-002.jpg') },
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

// LA-9, LA-10: a drop whose files already come to more than the budget is
// refused before anything is sent (`addFiles` says why), in the server's own
// sentence, and nothing landed.
it('refuses a drop past the request budget before sending it, in the server’s words', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);

  const half = uploadBudget.request_bytes / 2;
  const thrown: unknown = await addFiles('books', [
    claiming('page-001.jpg', half),
    claiming('page-002.jpg', half + 1),
  ]).catch((refusal: unknown) => refusal);

  expect(FakeRequest.made).toEqual([]);
  expect(isRefusal(thrown)).toBe(true);
  if (isRefusal(thrown)) {
    expect(thrown.kind).toBe('bad_request');
    expect(thrown.status).toBe(413);
    expect(thrown.message).toBe(uploadBudget.request_too_large);
    expect(thrown.written).toEqual([]);
  }
});

// The budget is a boundary: files that come to exactly it are the server's to
// weigh, because the framing on top is what decides, and only the server sees
// the body the browser makes.
it('sends a drop whose files come to no more than the budget', async () => {
  vi.stubGlobal('XMLHttpRequest', FakeRequest);
  vi.stubGlobal(
    'FormData',
    class {
      append() {}
    },
  );

  const added = addFiles('books', [claiming('page-001.jpg', uploadBudget.request_bytes)]);
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
  expect(body.getAll('file').map((part) => (part as File).name)).toEqual([
    'page-001.jpg',
    'page-002.jpg',
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
