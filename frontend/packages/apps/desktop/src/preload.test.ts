// The explorer preload, run as Electron would run it: the script itself,
// compiled to the CommonJS it is built as, in a context of its own whose
// `require` hands it stand-ins for Electron's renderer modules. What is
// asserted is what crosses the channel — the paths a set of picked or dropped
// `File`s is handed over as.

import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

import ts from 'typescript';
import { describe, expect, it, vi } from 'vitest';

import { TAKE_DROP } from './drops.js';

/** A `File` as the preload reads one: where it is on this device, and its way down from a picked folder. */
interface Picked {
  /** What `webUtils.getPathForFile` answers for it. */
  path: string;
  /** Empty for a file picked or dropped on its own. */
  webkitRelativePath: string;
}

/** A file picked or dropped on its own. */
function alone(path: string): Picked {
  return { path, webkitRelativePath: '' };
}

/** A file of a folder picked whole, at `path`, reached from the folder by `relative`. */
function inFolder(path: string, relative: string): Picked {
  return { path, webkitRelativePath: relative };
}

type TakeDrop = (files: readonly Picked[]) => Promise<unknown>;

/** The preload, loaded once per case, and what it asked the main process. */
function loaded(answer: () => Promise<unknown> = async () => 'answered') {
  const source = readFileSync(new URL('./preload.cts', import.meta.url), 'utf8');
  const compiled = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;

  const invoke = vi.fn<(channel: string, paths: string[]) => Promise<unknown>>(async () => answer());
  const exposed = new Map<string, unknown>();
  const electron = {
    contextBridge: {
      exposeInMainWorld: (name: string, api: unknown) => exposed.set(name, api),
    },
    ipcRenderer: { invoke },
    webUtils: { getPathForFile: (file: Picked) => file.path },
  };
  const required = (name: string) => {
    if (name !== 'electron') {
      throw new Error(`a sandboxed preload can require only Electron's own modules, not ${name}`);
    }
    return electron;
  };
  const module = { exports: {} };
  runInNewContext(compiled, { require: required, module, exports: module.exports, Error, Set, Array, String });

  const api = exposed.get('coffretDesktop') as { takeDrop: TakeDrop } | undefined;
  if (api === undefined) {
    throw new Error('the preload exposed nothing as coffretDesktop');
  }
  const handedOver = () => invoke.mock.calls.map(([channel, paths]) => ({ channel, paths }));
  return { takeDrop: api.takeDrop, handedOver };
}

describe('the explorer preload, handing picked files over by path', () => {
  it('hands a file picked on its own over as its own path', async () => {
    const { takeDrop, handedOver } = loaded();
    await takeDrop([alone('/home/someone/Pictures/a.jpg'), alone('/home/someone/b.jpg')]);
    expect(handedOver()).toEqual([
      { channel: TAKE_DROP, paths: ['/home/someone/Pictures/a.jpg', '/home/someone/b.jpg'] },
    ]);
  });

  it('hands every file of a folder picked whole over as that folder, once', async () => {
    const { takeDrop, handedOver } = loaded();
    await takeDrop([
      inFolder('/home/someone/books/vol-1/page-001.jpg', 'vol-1/page-001.jpg'),
      inFolder('/home/someone/books/vol-1/page-002.jpg', 'vol-1/page-002.jpg'),
      inFolder('/home/someone/books/vol-1/extras/cover.jpg', 'vol-1/extras/cover.jpg'),
    ]);
    expect(handedOver()).toEqual([{ channel: TAKE_DROP, paths: ['/home/someone/books/vol-1'] }]);
  });

  it('takes the way down off a Windows path, whose separators differ from it', async () => {
    const { takeDrop, handedOver } = loaded();
    await takeDrop([inFolder('C:\\Users\\someone\\vol-1\\sub\\page-001.jpg', 'vol-1/sub/page-001.jpg')]);
    expect(handedOver()).toEqual([{ channel: TAKE_DROP, paths: ['C:\\Users\\someone\\vol-1'] }]);
  });

  it('hands the same path over once however many times it was given, in the order first given', async () => {
    const { takeDrop, handedOver } = loaded();
    await takeDrop([
      alone('/home/someone/b.jpg'),
      inFolder('/home/someone/vol-1/page-001.jpg', 'vol-1/page-001.jpg'),
      alone('/home/someone/a.jpg'),
      alone('/home/someone/b.jpg'),
      inFolder('/home/someone/vol-1/page-002.jpg', 'vol-1/page-002.jpg'),
    ]);
    expect(handedOver()).toEqual([
      {
        channel: TAKE_DROP,
        paths: ['/home/someone/b.jpg', '/home/someone/vol-1', '/home/someone/a.jpg'],
      },
    ]);
  });

  it('answers the page with what the main process answered', async () => {
    const { takeDrop } = loaded(async () => ({ handle: 'h' }));
    await expect(takeDrop([alone('/a.jpg')])).resolves.toEqual({ handle: 'h' });
  });

  it("gives a refusal the main process's own sentence, without the channel's name before it", async () => {
    const { takeDrop } = loaded(async () => {
      throw new Error(`Error invoking remote method '${TAKE_DROP}': Error: no Library is open`);
    });
    await expect(takeDrop([alone('/a.jpg')])).rejects.toThrow(/^no Library is open$/);
  });
});
