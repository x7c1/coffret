import { useEffect, useRef } from 'react';

import type { Entry } from './layout';
import { prefetchTargets } from './layout';

const PREFETCH_RADIUS = 3;

export function ReaderView({
  entries,
  index,
  onNavigate,
  onClose,
  onPageTurnMeasured,
}: {
  entries: Entry[];
  index: number;
  onNavigate: (index: number) => void;
  onClose: () => void;
  onPageTurnMeasured: (ms: number) => void;
}) {
  const turnStartedAt = useRef<number | null>(null);
  // Cache prefetched Image objects so the browser keeps them decoded; keyed by
  // entry id, pruned implicitly when the reader unmounts.
  const prefetched = useRef(new Map<number, HTMLImageElement>());

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'ArrowRight' && index + 1 < entries.length) {
        turnStartedAt.current = performance.now();
        onNavigate(index + 1);
      } else if (event.key === 'ArrowLeft' && index > 0) {
        turnStartedAt.current = performance.now();
        onNavigate(index - 1);
      } else if (event.key === 'Escape') {
        onClose();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [entries.length, index, onNavigate, onClose]);

  useEffect(() => {
    for (const target of prefetchTargets(index, entries.length, PREFETCH_RADIUS)) {
      const id = entries[target].id;
      if (!prefetched.current.has(id)) {
        const image = new Image();
        image.src = `/api/image/${id}`;
        prefetched.current.set(id, image);
      }
    }
  }, [entries, index]);

  return (
    <div
      style={{
        flex: 1,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        minHeight: 0,
      }}
      onClick={onClose}
    >
      <img
        src={`/api/image/${entries[index].id}`}
        onLoad={() => {
          if (turnStartedAt.current !== null) {
            onPageTurnMeasured(performance.now() - turnStartedAt.current);
            turnStartedAt.current = null;
          }
        }}
        style={{ maxWidth: '100%', maxHeight: '100%', objectFit: 'contain' }}
      />
      <div
        style={{
          position: 'fixed',
          left: 8,
          bottom: 8,
          fontSize: 12,
          color: '#aaa',
          pointerEvents: 'none',
        }}
      >
        {entries[index].path} ({index + 1}/{entries.length}) — ←/→ to turn, Esc to close
      </div>
    </div>
  );
}
