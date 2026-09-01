import type { Fill, Library, Sync } from '@coffret/api';

import { fillLine, syncLine } from './fill';
import { COLOR } from './theme';
import type { Remote } from './useRemote';

/**
 * Which Library this is, along the bottom.
 *
 * The name and the provider, and a line while something is happening — a file
 * being brought over, files going up, the sync carrying them in — with the offer
 * of a second attempt where one of those stopped.
 *
 * And one control that is not about work already running: asking the Library
 * what is new. It is here because it is about the Library as a whole rather than
 * about the folder on the screen, which is what the status bar names — and
 * because nothing else on this page would ever ask: there is no polling of the
 * remote head, so without a press this device never hears of what another device
 * committed.
 *
 * Nothing else. There is no management screen in this release, and a status bar
 * that grew one would be the place it happened by accident.
 */
export function StatusBar({
  library,
  fetching,
  adding,
  fill,
  sync,
  trouble,
  onRetryFill,
  onRetrySync,
  refresh,
}: {
  library: Remote<Library>;
  fetching: string | null;
  /** The drop whose files are still going up, if one is. */
  adding: string | null;
  /** What the server is bringing over on its own, if anything. */
  fill: Fill | null;
  /** What the server is carrying into the Library on its own, if anything. */
  sync: Sync | null;
  /** What a retry was refused with, if one was. */
  trouble: string | null;
  onRetryFill: (folder: string) => void;
  onRetrySync: () => void;
  /**
   * Asking the Library what is new, and what the last asking came to.
   *
   * One value rather than four props, because the four are one control: the
   * button, whether it is busy, what it found, and what refused it.
   */
  refresh: {
    /** Whether a refresh is running right now. */
    running: boolean;
    /** What the last one came to, and `null` before any has run. */
    said: string | null;
    /** What refused the last one, and `null` where none was. */
    refused: string | null;
    ask: () => void;
  };
}) {
  // One line for what is in flight, and there is an order to who takes it. The
  // drop's own line comes first, because it is the only one about a request this
  // page is still making; the sync it arms takes over from it, and is put above
  // the fill because it is what somebody just asked for by dropping.
  //
  // Each candidate carries the colour it is drawn in, because the colour belongs
  // to whoever the line belongs to: a line about the sync drawn in red because a
  // fill stopped some minutes ago would be the bar colouring one thing by the
  // state of another — and it has room to say only the one.
  const line =
    shown(adding, COLOR.text) ??
    shown(syncLine(sync), toneOfSync(sync)) ??
    shown(fillLine(fill), fill?.status === 'stopped' ? COLOR.refused : COLOR.text);
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
      {/* The per-file line is what the candidates above fall through to, and not
          something shown beside them: it is the reader waiting on the page in
          front of it — the browser's own request lifecycle, which is all the
          server used to have a word for — while the fill is the folder being
          brought over behind it, and it is the larger thing happening. Both at
          once would be the bar reporting one fetch twice in two vocabularies. */}
      {line !== null ? (
        <span style={{ color: line.colour }}>{line.text}</span>
      ) : (
        fetching !== null && <span style={{ color: COLOR.text }}>fetching {fetching}…</span>
      )}
      {/* The same offer for the sync, from the same state and for the same
          reason: a Storage that came back should not have to be met by adding a
          file that is already sitting in the folder.

          Each button says what it would do again rather than both saying "try
          again": one Storage outage stops the fill and the sync together, and
          two identical buttons side by side would leave a person guessing which
          of the two the one line beside them belongs to. It stays an offer made
          from a stopped state and from nowhere else — nothing here is a "sync
          now". */}
      {sync !== null && sync.status === 'stopped' && (
        <button onClick={onRetrySync} style={RETRY}>
          back up again
        </button>
      )}
      {/* Offered from the failed state and from nowhere else. It is not a
          download button: what brings a folder over is opening a file in it, and
          this is here so that a Storage that came back does not have to be met
          by opening a file that is already open. */}
      {fill !== null && fill.status === 'stopped' && (
        <button onClick={() => onRetryFill(fill.folder)} style={RETRY}>
          bring over again
        </button>
      )}
      {trouble !== null && <span style={{ color: COLOR.refused }}>{trouble}</span>}
      {/* The control that asks what is new stands at the far end, apart from the
          two offers of a second attempt — those are made from a failure and go
          away with it, and this is always there.

          Its answer is said beside it rather than in the line above, because a
          refresh that is over has nothing to stop saying: in the line it would
          sit on top of the next fill's progress until something else happened. */}
      <span
        style={{ marginLeft: 'auto', display: 'flex', gap: 12, alignItems: 'center' }}
      >
        {refresh.refused !== null ? (
          <span style={{ color: COLOR.refused }}>{refresh.refused}</span>
        ) : (
          refresh.said !== null && <span>{refresh.said}</span>
        )}
        <button
          onClick={refresh.ask}
          disabled={refresh.running}
          style={{ ...RETRY, cursor: refresh.running ? 'default' : 'pointer' }}
        >
          {refresh.running ? 'looking…' : 'look for what is new'}
        </button>
      </span>
    </footer>
  );
}

/** One line and the colour it is drawn in, where there is a line to draw. */
function shown(text: string | null, colour: string): { text: string; colour: string } | null {
  return text === null ? null : { text, colour };
}

/**
 * What a sync's line is drawn in.
 *
 * A run that stopped is a refusal, in the colour every refusal on this screen is
 * in. A run that finished and still has a line is one that found something — a
 * file sitting in the folder that this run did not carry in — and that is the
 * warn colour for the reason the rows use it: something to be told rather than
 * shown, and not the failure of anything anybody asked for. Drawn as ordinary
 * text it would read as the housekeeping beside it and be looked past, which for
 * the one sentence saying a dropped file is not backed up is the whole loss.
 */
function toneOfSync(sync: Sync | null): string {
  switch (sync?.status) {
    case 'stopped':
      return COLOR.refused;
    case 'done':
      return COLOR.warn;
    default:
      return COLOR.text;
  }
}

/**
 * What every button along this bar is drawn as: the two offers of a second
 * attempt, and the one that asks the Library what is new.
 */
const RETRY = {
  border: `1px solid ${COLOR.border}`,
  background: COLOR.panel,
  color: COLOR.text,
  font: 'inherit',
  padding: '1px 10px',
  borderRadius: 4,
  cursor: 'pointer',
} as const;

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
