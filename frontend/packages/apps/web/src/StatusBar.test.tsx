import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it } from 'vitest';

import type { Fill, Freeze, Sync } from '@coffret/api';

import { StatusBar } from './StatusBar';

const refused =
  'the folder this device maps "albums" into is not the folder that mapping was recorded ' +
  'against, so nothing was put into it. Open a terminal on the device serving the Library. Run ' +
  '`coffret mappings --library <library>` to inspect the recorded mappings and `coffret map ' +
  '--help` to find the arguments. Use that listing to choose one recovery: reconnect the intended ' +
  'folder if it is elsewhere; if the folder at the recorded location is the intended one, record ' +
  'this mapping again with `coffret map`; or map another folder in its place only as a deliberate ' +
  'choice. If `coffret map` reports a local marker problem, correct the problem and run it again. ' +
  'Then return to the explorer and try the action again';
const renderedRefusal = refused
  .replaceAll('"', '&quot;')
  .replace('<library>', '&lt;library&gt;');

function filling(over: Partial<Fill> = {}): Fill {
  return {
    folder: 'albums',
    status: 'stopped',
    total: 2,
    done: 0,
    declined: [],
    stopped: { error: 'storage', message: 'Storage did not answer' },
    ...over,
  };
}

function syncing(over: Partial<Sync> = {}): Sync {
  return {
    status: 'stopped',
    added: 0,
    noted: [],
    stopped: { error: 'storage', message: 'Storage did not answer' },
    ...over,
  };
}

function freezing(over: Partial<Freeze> = {}): Freeze {
  return {
    folder: 'books/one',
    status: 'stopped',
    packs: 0,
    entries: 0,
    noted: [],
    stopped: { error: 'storage', message: 'Storage did not answer' },
    ...over,
  };
}

function draw({
  fill = null,
  sync = null,
  freeze = null,
}: {
  fill?: Fill | null;
  sync?: Sync | null;
  freeze?: Freeze | null;
}): string {
  return renderToStaticMarkup(
    <StatusBar
      library={{
        status: 'ready',
        value: { name: 'Home', library_id: 'library-id', provider: 'Storage' },
      }}
      fetching={null}
      adding={null}
      fill={fill}
      sync={sync}
      freeze={freeze}
      trouble={null}
      onRetryFill={() => undefined}
      onRetrySync={() => undefined}
      onRetryFreeze={() => undefined}
      onLock={() => undefined}
      locking={false}
      refresh={{ running: false, said: null, refused: null, ask: () => undefined }}
    />,
  );
}

it('keeps a refused-root explanation visible without offering the same fill again', () => {
  const html = draw({
    fill: filling({
      stopped: { error: 'declined', message: refused, reason: 'refused_root' },
    }),
  });

  expect(html).toContain(renderedRefusal);
  expect(html).not.toContain('bring over again');
});

it('suppresses retries for refused-root syncs and freezes too', () => {
  const stopped = { error: 'declined' as const, message: refused, reason: 'refused_root' as const };

  expect(draw({ sync: syncing({ stopped }) })).toContain(renderedRefusal);
  expect(draw({ sync: syncing({ stopped }) })).not.toContain('back up again');
  expect(draw({ freeze: freezing({ stopped }) })).toContain(renderedRefusal);
  expect(draw({ freeze: freezing({ stopped }) })).not.toContain('pack again');
});

it('offers retries for ordinary and legacy stopped responses only', () => {
  expect(draw({ fill: filling() })).toContain('bring over again');
  expect(
    draw({ fill: filling({ stopped: { error: 'declined', message: 'an older refusal' } }) }),
  ).toContain('bring over again');

  for (const fill of [
    null,
    filling({ status: 'filling', stopped: null }),
    filling({ status: 'done', stopped: null }),
    filling({ status: 'superseded', stopped: null }),
  ]) {
    expect(draw({ fill })).not.toContain('bring over again');
  }
});

it('restores the fill retry when later activity reports an ordinary stop', () => {
  const refusedHtml = draw({
    fill: filling({
      stopped: { error: 'declined', message: refused, reason: 'refused_root' },
    }),
  });
  const laterHtml = draw({ fill: filling() });

  expect(refusedHtml).not.toContain('bring over again');
  expect(laterHtml).toContain('bring over again');
});
