// The pages one reader has drawn, and the object URLs they are drawn from.
//
// The bytes arrive with `Cache-Control: private, no-store`, and deliberately:
// the Library's plaintext must never reach the browser's disk cache. So this is
// the only cache there is — object URLs over blobs this tab holds in memory,
// revoked as the reader moves away from them, when the key that made them
// plaintext is given up, and again when the reader closes.
//
// Kept free of DOM and of React so it is unit testable: what it holds is the
// one thing on this screen that outlives a request — plaintext this device
// decrypted — and every rule about when that goes is worth stating once, in a
// place a case can count the revocations at.

/** How the bytes of one page are asked for, which is [`getFile`](@coffret/api). */
export type Fetching = (path: string) => Promise<Blob>;

/** The pages a reader is holding, and every way one of them goes. */
export interface Drawn {
  /** The page at `path` where this reader holds it, and `undefined` where not. */
  held: (path: string) => string | undefined;
  /**
   * The page at `path`: the one held, or the one this asks for.
   *
   * One page asked for twice — the reader turning onto what it was prefetching
   * — is one request, and both callers wait on its answer.
   *
   * `undefined` where the answer came back to a key this device has given up, or
   * to a reader that has gone. Those bytes are revoked as they land, and a
   * revoked URL is not a page: handing it back would draw a broken one where the
   * page being asked for again is going.
   */
  load: (path: string) => Promise<string | undefined>;
  /**
   * Drops every page outside `wanted`, so a long folder does not leave the tab
   * holding every page of it.
   */
  keepOnly: (wanted: ReadonlySet<string>) => void;
  /**
   * Drops every page, because the key that made them plaintext is gone.
   *
   * A request that was already in flight is let go of with them: its bytes were
   * decrypted under the key the lock has just ended, so what it answers with is
   * revoked as it arrives rather than kept — and the next caller asking for that
   * page asks the server rather than being handed what this one brought back.
   * That request is the one whose refusal the reader shows.
   */
  discard: () => void;
  /** The reader went. Every page it held goes, and so does what arrives after. */
  closed: () => void;
  /** The reader is here again, and keeps what it fetches from now on. */
  reopened: () => void;
}

/**
 * A reader's pages, held until something says otherwise.
 *
 * Keyed by Entry Path rather than by position, because a listing refreshed
 * under the reader would otherwise let one page's bytes answer for another's.
 */
export function drawnPages(fetching: Fetching): Drawn {
  const held = new Map<string, string>();
  const running = new Map<string, Promise<string | undefined>>();
  let closed = false;
  // Which round of holding a page belongs to. A discard opens a new one, and
  // what a request of an older round brings back is nobody's: the key it was
  // decrypted under is not this device's any more.
  let round = 0;

  const revokeAll = () => {
    for (const url of held.values()) {
      URL.revokeObjectURL(url);
    }
    held.clear();
  };

  return {
    held: (path) => held.get(path),

    load(path) {
      const drawn = held.get(path);
      if (drawn !== undefined) {
        return Promise.resolve(drawn);
      }
      const started = running.get(path);
      if (started !== undefined) {
        return started;
      }
      const asked = round;
      const fetched = fetching(path).then(
        (blob) => {
          const url = URL.createObjectURL(blob);
          // Only where this is still the request that is running: a discard
          // clears the map, and what a later caller started is not this.
          if (running.get(path) === fetched) {
            running.delete(path);
          }
          if (closed || asked !== round) {
            // The reader went, or the key did, while this was in flight. The
            // blob is nobody's now, and an object URL nothing revokes is held
            // for the life of the tab. Nobody is handed it either — not even
            // the caller waiting on this one, which would otherwise draw a page
            // out of a URL that stopped meaning anything a line ago.
            URL.revokeObjectURL(url);
            return undefined;
          }
          held.set(path, url);
          return url;
        },
        (refused: unknown) => {
          if (running.get(path) === fetched) {
            running.delete(path);
          }
          throw refused;
        },
      );
      running.set(path, fetched);
      return fetched;
    },

    keepOnly(wanted) {
      for (const [path, url] of held) {
        if (!wanted.has(path)) {
          URL.revokeObjectURL(url);
          held.delete(path);
        }
      }
    },

    discard() {
      round += 1;
      revokeAll();
      running.clear();
    },

    closed() {
      closed = true;
      revokeAll();
    },

    reopened() {
      closed = false;
    },
  };
}
