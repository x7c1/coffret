// The explorer window's preload, and the one preload of the app.
//
// The explorer runs in the app's own window, so a drop onto it can be handed
// over as the paths of what was let go of rather than read into the page and
// sent as bytes. This exposes `coffretDesktop.takeDrop` to the page: given the
// dropped `File`s, it maps each to its path on this device with
// `webUtils.getPathForFile` and asks the main process to walk them over the one
// channel the app answers. Files picked with the explorer's **+**
// go the same way; a folder picked whole arrives as every file under it, each
// carrying its way down from the folder (`webkitRelativePath`), and is handed
// over as the folder itself, so that the walk names its files relative to
// where it was picked from as it does for a folder dropped. The page is answered with a one-time
// drop handle and each file's path relative to the folder it was dropped from;
// the paths themselves never reach the page.
//
// It also carries the explorer's session, one way in each direction and with
// no answer either way. `coffretDesktop.onTicket` registers a listener the
// main process's fresh tickets are relayed to: the main process sends one on
// its own timer ahead of the page's token's expiry, and the page exchanges it
// for a new token (spec: LA-18). `coffretDesktop.sessionEnded` tells the main
// process that the page holds no session, which loads the window afresh where
// it can (spec: LA-21). The page has no call that asks for a ticket.
//
// `coffretDesktop.appearanceChanged` tells the main process, one way and with
// nothing in it, that the server took a new appearance for the explorer; the
// main process reads the appearance afresh and draws the windows' chrome in it.
//
// It runs sandboxed and isolated like every page of the app, so it is a
// CommonJS script that reaches nothing but Electron's renderer modules: the
// channels' names are written out here rather than imported, and `drops.ts`,
// `renewal.ts` and `appearance.ts` hold the main process's copies of them,
// which their cases hold to these.

// eslint-disable-next-line @typescript-eslint/no-require-imports -- a sandboxed preload is CommonJS and can require only Electron's own modules
import electron = require('electron');

const { contextBridge, ipcRenderer, webUtils } = electron;

/** The one channel the main process takes a drop on. */
const TAKE_DROP = 'coffret:take-drop';

/** The channel the main process hands a fresh ticket over on. */
const FRESH_TICKET = 'coffret:fresh-ticket';

/** The channel the page says over that it holds no session. */
const SESSION_ENDED = 'coffret:session-ended';

/** The channel the page says over that the server took a new appearance. */
const APPEARANCE_CHANGED = 'coffret:appearance-changed';

/**
 * What Electron puts before the main process's own sentence when a call over a
 * channel is refused: `Error invoking remote method '<channel>': Error: `.
 */
const INVOKE_REFUSED = /^Error invoking remote method '[^']*': (?:Error: )?/;

/**
 * The path one file stands for: its own, or — for a file of a folder picked
 * whole — the picked folder's, which is its path with the way down from that
 * folder taken off and the folder's own name put back. The two ways down are
 * the same length whichever separator each is written with.
 */
function pickedPath(file: File): string {
  const path = webUtils.getPathForFile(file);
  const relative = file.webkitRelativePath;
  if (relative === '') {
    return path;
  }
  return path.slice(0, path.length - relative.length) + relative.split('/')[0];
}

contextBridge.exposeInMainWorld('coffretDesktop', {
  async takeDrop(files: readonly File[]): Promise<unknown> {
    const paths = [...new Set(Array.from(files, pickedPath))];
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
  onTicket(listener: (ticket: string) => void): () => void {
    // The page is handed the ticket alone, never the event it came in.
    const relay = (_event: unknown, ticket: unknown) => {
      if (typeof ticket === 'string') {
        listener(ticket);
      }
    };
    ipcRenderer.on(FRESH_TICKET, relay);
    return () => {
      ipcRenderer.removeListener(FRESH_TICKET, relay);
    };
  },
  sessionEnded(): void {
    ipcRenderer.send(SESSION_ENDED);
  },
  appearanceChanged(): void {
    ipcRenderer.send(APPEARANCE_CHANGED);
  },
});
