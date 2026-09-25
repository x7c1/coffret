import { useEffect, useRef, useState } from 'react';

import { getFile } from '@coffret/api';

import { drawnPages } from './drawn';
import type { Page } from './pages';
import { stepped } from './pages';
import { prefetchTargets } from './prefetch';
import { COLOR } from './theme';
import { said } from './useAsked';

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
  discarded,
  onNavigate,
  onClose,
  onFetching,
  onFetched,
}: {
  pages: readonly Page[];
  at: number;
  discarded: number;
  onNavigate: (at: number) => void;
  onClose: () => void;
  onFetching: (name: string | null) => void;
  onFetched: () => void;
}) {
  // Every page this reader has drawn, by Entry Path — see [`drawn`](./drawn),
  // which holds them and is the one place they are revoked from.
  const [drawn] = useState(() => drawnPages(getFile));

  const [shown, setShown] = useState<Shown>({ status: 'loading' });
  const [attempt, setAttempt] = useState(0);

  const page: Page | undefined = pages[at];
  const path = page?.path;
  const name = page?.name;
  const remote = page?.remote ?? false;

  useEffect(() => {
    if (path === undefined || name === undefined) {
      return;
    }
    const held = drawn.held(path);
    if (held !== undefined) {
      setShown({ status: 'ready', url: held });
      return;
    }
    let live = true;
    setShown({ status: 'loading' });
    onFetching(name);
    void drawn.load(path).then(
      (url) => {
        // Nothing to show where the answer came back to a key this device has
        // given up: it was revoked as it landed, and the page it was is already
        // being asked for again by the attempt the discard started.
        if (!live || url === undefined) {
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
  }, [path, name, remote, attempt, drawn, onFetching, onFetched]);

  useEffect(() => {
    const wanted = new Set<string>();
    const here = pages[at]?.path;
    if (here !== undefined) {
      wanted.add(here);
    }
    for (const target of prefetchTargets(at, pages.length, PREFETCH_RADIUS)) {
      const ahead = pages[target].path;
      wanted.add(ahead);
      // A page that would not prefetch is not this page's problem. It is asked
      // for again — and answered for, on the screen — when it is turned to.
      void drawn.load(ahead).catch(() => undefined);
    }
    drawn.keepOnly(wanted);
  }, [pages, at, drawn]);

  // What a lock takes back. The pages this reader is holding are plaintext the
  // Master Key made, and a lock ends this server's hold on that key — so they
  // go with it: the one on the screen and every one prefetched around it.
  const gone = useRef(discarded);
  useEffect(() => {
    if (gone.current === discarded) {
      return;
    }
    gone.current = discarded;
    drawn.discard();
    // The page on the screen was drawn from a URL that has just been revoked, so
    // it comes off in the same breath rather than one render later — and what
    // replaces it is the asking, which is what this screen shows while a page is
    // on its way. The answer to it is the refusal, while the server is locked,
    // and the page itself if it is not.
    setShown({ status: 'loading' });
    setAttempt((made) => made + 1);
  }, [discarded, drawn]);

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
    // React mounts an effect twice in development, and what holds the pages
    // outlives both mounts, so the first one's teardown must not leave the
    // second refusing to keep what it fetches. Both run in one synchronous
    // batch, well before any request of the first can have answered.
    drawn.reopened();
    return () => drawn.closed();
  }, [drawn]);

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
