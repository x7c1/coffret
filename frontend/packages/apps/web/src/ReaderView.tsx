import { useCallback, useEffect, useRef, useState } from 'react';

import { getFile } from '@coffret/api';

import type { Page } from './pages';
import { stepped } from './pages';
import { prefetchTargets } from './prefetch';
import { COLOR } from './theme';
import { said } from './useRemote';

/** How far ahead and behind the reader keeps pages ready. */
const PREFETCH_RADIUS = 3;

/** What the reader has to show for the page it is on. */
type Shown =
  | { status: 'loading' }
  | { status: 'ready'; url: string }
  | { status: 'failed'; message: string };

/**
 * One page, large, over the list.
 *
 * `←` and `→` move through the openable files of the folder — the rest are
 * stepped over, since there is nothing to show for them — and `Escape` or a
 * click goes back to the list with the file that was open still selected.
 *
 * A page this device does not have yet is fetched by the same request as any
 * other; what it costs is a placeholder while it is in flight, and a sentence
 * and a retry where it was refused. Neither is a screen a reader is stuck on.
 */
export function ReaderView({
  pages,
  at,
  onNavigate,
  onClose,
  onFetching,
  onFetched,
}: {
  pages: readonly Page[];
  at: number;
  onNavigate: (at: number) => void;
  onClose: () => void;
  onFetching: (name: string | null) => void;
  onFetched: () => void;
}) {
  // Every page this reader has drawn, by Entry Path.
  //
  // The bytes arrive with `Cache-Control: private, no-store`, and deliberately:
  // the Library's plaintext must never reach the browser's disk cache. So this
  // map is the only cache there is — object URLs over blobs this tab holds in
  // memory, revoked as the reader moves away from them and again when it
  // closes, and gone with the tab either way.
  //
  // Keyed by path rather than by position, because a listing refreshed under
  // the reader would otherwise let one page's bytes answer for another's.
  const drawn = useRef(new Map<string, string>());
  const running = useRef(new Map<string, Promise<string>>());
  const closed = useRef(false);

  const [shown, setShown] = useState<Shown>({ status: 'loading' });
  const [attempt, setAttempt] = useState(0);

  const load = useCallback((path: string): Promise<string> => {
    const held = drawn.current.get(path);
    if (held !== undefined) {
      return Promise.resolve(held);
    }
    // One page asked for twice — the reader turning onto what it was
    // prefetching — is one request, and both callers wait on its answer.
    const started = running.current.get(path);
    if (started !== undefined) {
      return started;
    }
    const fetching = getFile(path).then(
      (blob) => {
        const url = URL.createObjectURL(blob);
        running.current.delete(path);
        if (closed.current) {
          // The reader went while this was in flight. The blob is nobody's now,
          // and an object URL nothing revokes is held for the life of the tab.
          URL.revokeObjectURL(url);
          return url;
        }
        drawn.current.set(path, url);
        return url;
      },
      (refused: unknown) => {
        running.current.delete(path);
        throw refused;
      },
    );
    running.current.set(path, fetching);
    return fetching;
  }, []);

  const page: Page | undefined = pages[at];
  const path = page?.path;
  const name = page?.name;
  const remote = page?.remote ?? false;

  useEffect(() => {
    if (path === undefined || name === undefined) {
      return;
    }
    const held = drawn.current.get(path);
    if (held !== undefined) {
      setShown({ status: 'ready', url: held });
      return;
    }
    let live = true;
    setShown({ status: 'loading' });
    onFetching(name);
    void load(path).then(
      (url) => {
        if (!live) {
          return;
        }
        // The request is over either way, and the status bar's line is about a
        // request in flight: cleared here rather than only when the reader moves
        // on, or the bar would go on saying "fetching" over a page on the screen.
        onFetching(null);
        setShown({ status: 'ready', url });
        if (remote) {
          // The file is on this device now, so a row still saying `remote` is
          // stale. Which rows changed is the server's to say, so the listing is
          // asked again rather than edited here.
          onFetched();
        }
      },
      (refused: unknown) => {
        if (live) {
          onFetching(null);
          setShown({ status: 'failed', message: said(refused) });
        }
      },
    );
    return () => {
      live = false;
      onFetching(null);
    };
  }, [path, name, remote, attempt, load, onFetching, onFetched]);

  useEffect(() => {
    const wanted = new Set<string | undefined>([pages[at]?.path]);
    for (const target of prefetchTargets(at, pages.length, PREFETCH_RADIUS)) {
      const ahead = pages[target].path;
      wanted.add(ahead);
      // A page that would not prefetch is not this page's problem. It is asked
      // for again — and answered for, on the screen — when it is turned to.
      void load(ahead).catch(() => undefined);
    }
    // Everything outside the window goes, so a long folder does not leave the
    // tab holding every page of it.
    for (const [held, url] of drawn.current) {
      if (!wanted.has(held)) {
        URL.revokeObjectURL(url);
        drawn.current.delete(held);
      }
    }
  }, [pages, at, load]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'ArrowRight') {
        onNavigate(stepped(pages, at, 1));
      } else if (event.key === 'ArrowLeft') {
        onNavigate(stepped(pages, at, -1));
      } else if (event.key === 'Escape') {
        onClose();
      } else {
        return;
      }
      event.preventDefault();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [pages, at, onNavigate, onClose]);

  // Closing the reader is this tab dropping every page it held.
  useEffect(() => {
    const held = drawn.current;
    // React mounts an effect twice in development, and the refs outlive both
    // mounts, so the first one's teardown must not leave the second refusing to
    // keep what it fetches. Both run in one synchronous batch, well before any
    // request of the first can have answered.
    closed.current = false;
    return () => {
      closed.current = true;
      for (const url of held.values()) {
        URL.revokeObjectURL(url);
      }
      held.clear();
    };
  }, []);

  if (page === undefined) {
    return null;
  }

  return (
    <div
      onClick={onClose}
      style={{
        position: 'absolute',
        inset: 0,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'rgba(0, 0, 0, 0.94)',
        zIndex: 10,
      }}
    >
      <Shows
        shown={shown}
        name={page.name}
        remote={page.remote}
        onRetry={() => setAttempt((made) => made + 1)}
      />
      <div style={{ padding: '10px 12px', fontSize: 12, color: COLOR.dim }}>
        {page.name} ({at + 1}/{pages.length}) — ←/→ to turn, Esc to close
      </div>
    </div>
  );
}

function Shows({
  shown,
  name,
  remote,
  onRetry,
}: {
  shown: Shown;
  name: string;
  remote: boolean;
  onRetry: () => void;
}) {
  switch (shown.status) {
    case 'loading':
      // The wait is the same request either way, and only one of the two waits
      // has anything to explain: a page this device already has is read off its
      // own disk, and saying it was not there would be saying the opposite of
      // what the row it was opened from says (spec: EP-10).
      return (
        <p style={{ color: COLOR.dim, textAlign: 'center' }}>
          fetching {name}…
          {remote && (
            <>
              <br />
              <span style={{ fontSize: 12 }}>
                this device does not have it yet, so it is being brought over
              </span>
            </>
          )}
        </p>
      );
    case 'failed':
      return (
        <div
          onClick={(event) => event.stopPropagation()}
          style={{ textAlign: 'center', maxWidth: 520, padding: 16 }}
        >
          <p style={{ color: COLOR.refused }}>{shown.message}</p>
          <button
            onClick={onRetry}
            style={{
              border: `1px solid ${COLOR.border}`,
              background: COLOR.panel,
              color: COLOR.text,
              font: 'inherit',
              padding: '5px 14px',
              borderRadius: 4,
              cursor: 'pointer',
            }}
          >
            try again
          </button>
        </div>
      );
    case 'ready':
      return (
        <img
          src={shown.url}
          alt={name}
          style={{ flex: 1, minHeight: 0, maxWidth: '100%', objectFit: 'contain' }}
        />
      );
  }
}
