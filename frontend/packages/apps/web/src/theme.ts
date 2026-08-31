// The few colours the explorer is drawn in, named once.
//
// Inline styles and no framework: the screen is three regions and a reader, and
// a stylesheet for that would be a second place to look — index.html holds only
// the ground and the hover states, which React would have to re-render a row to
// write, so neither colour is named here. What a name buys is that "the dim one"
// is one colour rather than four hex codes that drifted.

export const COLOR = {
  /** The tree and the status bar, a shade off the ground. */
  panel: '#181818',
  /** The folder being listed, and the row the reader was last opened at. */
  selected: '#2b3d52',
  border: '#333',
  text: '#eee',
  /** Sizes, times, and everything else that is not the name. */
  dim: '#8b8b8b',
  /** This device has the file. */
  present: '#5b9a63',
  /** The Library has it and this device does not. */
  remote: '#6d7f96',
  /** It is being brought over right now, whether or not anybody asked for it. */
  fetching: '#5f9ea0',
  /** Something the reader has to be told rather than shown. */
  warn: '#c9a227',
  /** A refusal. */
  refused: '#c96a5a',
} as const;
