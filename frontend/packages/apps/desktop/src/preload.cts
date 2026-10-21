// The explorer window's preload, and the one preload of the app.
//
// The explorer runs in the app's own window, so a drop onto it can be handed
// over as the paths of what was let go of rather than read into the page and
// sent as bytes. This exposes one function to the page, `coffretDesktop.
// takeDrop`: given the dropped `File`s, it maps each to its path on this device
// with `webUtils.getPathForFile` and asks the main process to walk them over
// the one channel the app listens on. The page is answered with a one-time
// drop handle and each file's path relative to the folder it was dropped from;
// the paths themselves never reach the page.
//
// It runs sandboxed and isolated like every page of the app, so it is a
// CommonJS script that reaches nothing but Electron's renderer modules: the
// channel's name is written out here rather than imported, and `drops.ts` holds
// the main process's copy of it, which its cases hold to this one.

// eslint-disable-next-line @typescript-eslint/no-require-imports -- a sandboxed preload is CommonJS and can require only Electron's own modules
import electron = require('electron');

const { contextBridge, ipcRenderer, webUtils } = electron;

/** The one channel the main process takes a drop on. */
const TAKE_DROP = 'coffret:take-drop';

/**
 * What Electron puts before the main process's own sentence when a call over a
 * channel is refused: `Error invoking remote method '<channel>': Error: `.
 */
const INVOKE_REFUSED = /^Error invoking remote method '[^']*': (?:Error: )?/;

contextBridge.exposeInMainWorld('coffretDesktop', {
  async takeDrop(files: readonly File[]): Promise<unknown> {
    const paths = Array.from(files, (file) => webUtils.getPathForFile(file));
    try {
      return await ipcRenderer.invoke(TAKE_DROP, paths);
    } catch (refused: unknown) {
      // The page shows a refusal's message as the reason nothing was added, so
      // it is handed the sentence the app refused with — the addon's, such as
      // that no Library is open — rather than one naming the channel.
      const message = refused instanceof Error ? refused.message : String(refused);
      throw new Error(message.replace(INVOKE_REFUSED, ''));
    }
  },
});
