import type { Library } from '@coffret/api';

import { COLOR } from './theme';
import type { Remote } from './useRemote';

/**
 * Which Library this is, along the bottom.
 *
 * The name and the provider, and a line while a fetch is running. Nothing else:
 * there is no management screen in this release, and a status bar that grew one
 * would be the place it happened by accident.
 */
export function StatusBar({
  library,
  fetching,
}: {
  library: Remote<Library>;
  fetching: string | null;
}) {
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
      {/* The browser's own request lifecycle and nothing more: the server says
          `present` or `remote` about an Entry and never `fetching`, because
          nothing on this device changes between asking for one and getting it. */}
      {fetching !== null && <span style={{ color: COLOR.text }}>fetching {fetching}…</span>}
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
