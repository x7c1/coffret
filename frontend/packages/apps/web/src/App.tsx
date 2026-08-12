import { useEffect, useState } from 'react';

import { GridView } from './GridView';
import type { Entry } from './layout';
import { ReaderView } from './ReaderView';

type View = { kind: 'grid' } | { kind: 'reader'; index: number };

export interface Stats {
  entriesLoadedMs: number | null;
  lastPageTurnMs: number | null;
}

export function App() {
  const [entries, setEntries] = useState<Entry[] | null>(null);
  const [view, setView] = useState<View>({ kind: 'grid' });
  const [stats, setStats] = useState<Stats>({ entriesLoadedMs: null, lastPageTurnMs: null });

  useEffect(() => {
    let cancelled = false;
    const started = performance.now();
    fetch('/api/entries')
      .then((res) => {
        if (!res.ok) {
          throw new Error(`GET /api/entries -> ${res.status}`);
        }
        return res.json() as Promise<Entry[]>;
      })
      .then((loaded) => {
        if (cancelled) {
          return;
        }
        setEntries(loaded);
        setStats((s) => ({ ...s, entriesLoadedMs: performance.now() - started }));
      })
      .catch((e: unknown) => {
        console.error('failed to load entries', e);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (entries === null) {
    return <p style={{ padding: 16 }}>loading library…</p>;
  }

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      {view.kind === 'grid' ? (
        <GridView entries={entries} onOpen={(index) => setView({ kind: 'reader', index })} />
      ) : (
        <ReaderView
          entries={entries}
          index={view.index}
          onNavigate={(index) => setView({ kind: 'reader', index })}
          onClose={() => setView({ kind: 'grid' })}
          onPageTurnMeasured={(ms) => setStats((s) => ({ ...s, lastPageTurnMs: ms }))}
        />
      )}
      <Hud entryCount={entries.length} stats={stats} />
    </div>
  );
}

function Hud({ entryCount, stats }: { entryCount: number; stats: Stats }) {
  const format = (ms: number | null) => (ms === null ? '—' : `${ms.toFixed(0)} ms`);
  return (
    <div
      style={{
        position: 'fixed',
        right: 8,
        bottom: 8,
        padding: '6px 10px',
        background: 'rgba(0,0,0,0.7)',
        border: '1px solid #444',
        borderRadius: 6,
        fontSize: 12,
        lineHeight: 1.6,
        pointerEvents: 'none',
      }}
    >
      <div>entries: {entryCount}</div>
      <div>list loaded: {format(stats.entriesLoadedMs)}</div>
      <div>page turn: {format(stats.lastPageTurnMs)}</div>
    </div>
  );
}
