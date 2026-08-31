import type { Fill, Library } from '@coffret/api';

import { fillLine } from './fill';
import { COLOR } from './theme';
import type { Remote } from './useRemote';

/**
 * Which Library this is, along the bottom.
 *
 * The name and the provider, and a line while something is being brought over.
 * Nothing else: there is no management screen in this release, and a status bar
 * that grew one would be the place it happened by accident.
 */
export function StatusBar({
  library,
  fetching,
  fill,
  trouble,
  onRetryFill,
}: {
  library: Remote<Library>;
  fetching: string | null;
  /** What the server is bringing over on its own, if anything. */
  fill: Fill | null;
  /** What a retry was refused with, if one was. */
  trouble: string | null;
  onRetryFill: (folder: string) => void;
}) {
  const line = fillLine(fill);
  return (
    <footer
      style={{
        flex: '0 0 auto',
        display: 'flex',
        gap: 16,
        alignItems: 'center',
        padding: '5px 12px',
        background: COLOR.panel,
        borderTop: `1px solid ${COLOR.border}`,
        fontSize: 12,
        color: COLOR.dim,
      }}
    >
      {/* A refusal standing where the Library's name goes is not another dim
          line of housekeeping: it is the sentence saying why the screen above
          is empty, and it is coloured like the ones up there. */}
      <span style={library.status === 'failed' ? { color: COLOR.refused } : undefined}>
        {named(library)}
      </span>
      {/* One line for what is in flight, and the fill takes it whenever there is
          one. The per-file line is the reader waiting on the page in front of
          it — the browser's own request lifecycle, which is all the server used
          to have a word for; the fill is the folder being brought over behind
          it, and it is the larger thing happening. Both at once would be the bar
          reporting one fetch twice in two vocabularies. */}
      {line !== null ? (
        <span style={{ color: fill?.status === 'stopped' ? COLOR.refused : COLOR.text }}>
          {line}
        </span>
      ) : (
        fetching !== null && <span style={{ color: COLOR.text }}>fetching {fetching}…</span>
      )}
      {/* Offered from the failed state and from nowhere else. It is not a
          download button: what brings a folder over is opening a file in it, and
          this is here so that a Storage that came back does not have to be met
          by opening a file that is already open. */}
      {fill !== null && fill.status === 'stopped' && (
        <button
          onClick={() => onRetryFill(fill.folder)}
          style={{
            border: `1px solid ${COLOR.border}`,
            background: COLOR.panel,
            color: COLOR.text,
            font: 'inherit',
            padding: '1px 10px',
            borderRadius: 4,
            cursor: 'pointer',
          }}
        >
          try again
        </button>
      )}
      {trouble !== null && <span style={{ color: COLOR.refused }}>{trouble}</span>}
    </footer>
  );
}

function named(library: Remote<Library>): string {
  switch (library.status) {
    case 'loading':
      return 'opening the Library…';
    case 'ready':
      return `${library.value.name} — on ${library.value.provider}`;
    case 'failed':
      return library.message;
  }
}
