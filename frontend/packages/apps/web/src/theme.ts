// The colours the explorer is drawn in, named once.
//
// Inline styles and no framework: the screen is three regions and a reader, and
// a stylesheet for that would be a second place to look — index.html holds only
// the ground and the hover states, which React would have to re-render a row to
// write, and repeats the few values of these it paints. What a name buys is that
// "the dim one" is one colour rather than four hex codes that drifted, and that
// a second set — a dark one beside this light one — is a change of values here
// rather than a hunt through every component. `theme.test.ts` holds the
// components to that: no colour literal outside this file, shadows included.

/** The explorer's colours, light. */
export const COLOR = {
  /**
   * The ground, as `index.html` paints the page, and the tree and the details
   * pane on it: named here for what is drawn over rows scrolling under it —
   * the tree's column headings — which has to be filled with it rather than
   * left clear.
   */
  ground: '#f7f8fa',
  /** What stands on the ground: the header, the Activity panel, menus, dialogs. */
  panel: '#ffffff',
  border: '#e3e6ea',
  /** The line between one row and the next, and between a menu's groups: fainter than a border. */
  rowRule: '#eef0f3',
  /** The frame of a round control that stands alone: the **?**. A shade firmer than a border. */
  control: '#d5d9df',
  text: '#16181d',
  /** Sizes, times, the location, headings: everything that is not the name. */
  dim: '#5b616e',
  /** Fainter than `dim`: what stands in for a value, and the marks around one. */
  faint: '#8a919c',
  /**
   * The one colour that acts: the focused row, the buttons that go ahead, a
   * file's icon, progress — and the sync icon while there is something newer
   * to bring in.
   */
  accent: '#2563eb',
  /**
   * The accent, darker: text on a tint of it — what is on its way into the
   * Library, which the drop target and the banners about it are tinted for.
   */
  accentDeep: '#1d4ed8',
  /** Text and icons on the accent itself: the focused row, a primary button. */
  onAccent: '#ffffff',
  /** A row selected beside the focused one: a tint of the accent, so the focused row still stands out. */
  picked: '#dbe6fd',
  /** A lighter tint of the accent: the row a drop lands in, and what a banner about files coming in stands on. */
  accentTint: '#e8effd',
  /** What the pointer is over: a menu's item, a row, a row's "⋮". */
  hover: '#eef1f5',
  /** A refusal, and a failure. */
  refused: '#b42318',
  /** Under a refusal: and what a destructive menu item is shaded with. */
  refusedGround: '#fdecea',
  /** Something the reader has to be told rather than shown. */
  warn: '#8a6100',
  /** A warning's text inside a notice box, where `warn` would be too light to read as a paragraph. */
  warnDeep: '#5c4400',
  /** Under a warning. */
  warnGround: '#fff8e6',
  /** It went well: the dot of a finished entry that succeeded. */
  success: '#15803d',
  /** This device has the file: the laptop in the tree's narrow column. */
  present: '#3d424d',
  /** What dims the screen behind a dialog. */
  backdrop: 'rgba(16, 24, 40, 0.32)',
} as const;

/**
 * The reader's sheet, which stays dark over the light screen: a picture is
 * read best on a dark ground, and the sheet reads as a layer over the tree
 * rather than a fourth region of it.
 */
export const SHEET_COLOR = {
  ground: '#111214',
  text: '#e6e8eb',
  /** Its secondary text: where the row stands, the position, captions. */
  dim: '#a3a8b0',
  /** Its lines and frames: a step off the ground, as `border` is on the light screen. */
  border: '#2a2d33',
  /** What stands on its ground — its buttons, the sheets fanned behind a cover — as `panel` does on the light screen. */
  raised: '#1b1d21',
  /** A refusal said on the sheet: `refused` lightened to be read on its ground. */
  refused: '#f97066',
  /**
   * The two sheets fanned behind a folder's cover, the farther the darker:
   * light enough against the ground to read as paper under the cover rather
   * than as part of the dark.
   */
  stackFar: '#4a4f58',
  stackNear: '#646a74',
} as const;

/** The shadows, named for what casts them: each one is a colour too. */
export const SHADOW = {
  /** Under the actions menu, the pill a drag brings up, and the Activity panel's shortcuts tooltip. */
  menu: '0 12px 32px rgba(0, 0, 0, 0.16)',
  /** Under a dialog. */
  dialog: '0 24px 64px rgba(0, 0, 0, 0.24)',
  /** Along the reader's sheet's left edge, over the rows it covers. */
  sheet: '-16px 0 40px rgba(0, 0, 0, 0.28)',
} as const;

/** The explorer's typeface, as `index.html` sets it on the page and the tree's column measure takes it. */
export const FONT = 'Inter, system-ui, sans-serif';
