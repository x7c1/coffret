// The few colours the explorer is drawn in, named once.
//
// Inline styles and no framework: the screen is three regions and a reader, and
// a stylesheet for that would be a second place to look — index.html holds only
// the ground and the hover states, which React would have to re-render a row to
// write, so neither colour is named here. What a name buys is that "the dim one"
// is one colour rather than four hex codes that drifted.

export const COLOR = {
  /**
   * The ground, as `index.html` paints the page: named here for what is drawn
   * over rows scrolling under it — the tree's column headings — which has
   * to be filled with it rather than left clear.
   */
  ground: '#111',
  /** The bar over the tree and the Activity panel, a shade off the ground. */
  panel: '#181818',
  /** The row of the tree focused by a click or the arrow keys. */
  selected: '#2b3d52',
  /**
   * A row selected beside the focused one: a light tint of `selected`, so the
   * focused row still stands out among them.
   */
  picked: '#1d2733',
  /**
   * The focused row where others are selected and it is not, after a
   * ⌘/Ctrl+click took it out: a ring in a brighter shade of `selected`, so the
   * keys still show where they stand without the row reading as selected.
   */
  focusRing: '#4f6f94',
  border: '#333',
  /** The line between one row of the tree and the next, fainter than a border. */
  rowRule: '#202020',
  text: '#eee',
  /** Sizes, times, and everything else that is not the name. */
  dim: '#8b8b8b',
  /** This device has the file. */
  present: '#5b9a63',
  /** The Library has it and this device does not. */
  remote: '#6d7f96',
  /** It is being brought over right now, whether or not anybody asked for it. */
  fetching: '#5f9ea0',
  /** It is in the folder and the Library does not have it. */
  added: '#8a7cc0',
  /** Something the reader has to be told rather than shown. */
  warn: '#c9a227',
  /** A refusal. */
  refused: '#c96a5a',
  /**
   * The sync icon while the Library has a head this device has not caught up
   * with: brighter than `remote`, since what it says is that a press would
   * bring something in.
   */
  newer: '#4fa3e6',
  /** Under a banner drawn in `added`: about files on their way into the Library. */
  addedGround: '#12222a',
  /** Under a banner drawn in `warn`. */
  warnGround: '#2a2413',
  /** Under a banner drawn in `refused`: a gesture that would be turned away. */
  refusedGround: '#2a1714',
} as const;
