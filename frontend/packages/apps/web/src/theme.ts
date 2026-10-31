// The colours the explorer is drawn in, named once.
//
// Inline styles and no framework: the screen is three regions and a reader, and
// a stylesheet for that would be a second place to look — index.html holds only
// the ground and the hover states, which React would have to re-render a row to
// write. What a name buys is that "the dim one" is one colour rather than four
// hex codes that drifted, and that the explorer is drawn in two sets — a light
// one and a dark one — by a change of values here rather than a hunt through
// every component. `theme.test.ts` holds the components to that: no colour
// literal outside this file, shadows included.
//
// A component draws in `COLOR`, whose every value is a CSS custom property
// rather than a colour: the page's root carries the values of whichever set is
// showing (see [`paintScheme`](./appearance)), so switching sets is one write
// to the root and not a re-render of every row. `index.html` paints with the
// same properties.

/** The explorer's colours, light: what the page is drawn in unless the dark set is showing. */
export const LIGHT = {
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
  text: '#16181d',
  /** Sizes, times, the location, headings: everything that is not the name. */
  dim: '#5b616e',
  /** Fainter than `dim`: what stands in for a value, the marks around one, and a folder's size, a total rather than a file's own. */
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
  /**
   * The accent under the pointer, where something is drawn on the accent
   * itself: the "⋮" of the focused row, as `index.html` paints it. A shade
   * darker, so the white mark on it stays white on blue.
   */
  accentHover: '#1d4ed8',
  /** Text and icons on the accent itself: the focused row, a primary button. */
  onAccent: '#ffffff',
  /** A row selected beside the focused one: a tint of the accent, so the focused row still stands out. */
  picked: '#dbe6fd',
  /** A fainter tint of the accent than `picked`: the row a drop lands in, and what a banner about files coming in stands on. */
  accentTint: '#e8effd',
  /** Inside a folder's icon, wherever one is drawn on the ground: a pale blue in the light set, a muted one in the dark. */
  folderFill: '#cddcfb',
  /** A folder icon's outline around `folderFill`: a blue a step off the fill — deeper in the light set, lighter in the dark — so the shape holds on the ground. */
  folderStroke: '#5b8def',
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

/** One name of the explorer's colours. */
type ColorName = keyof typeof LIGHT;

/**
 * The explorer's colours, dark: a value for every name of the light set, each
 * playing the same part on a dark ground.
 *
 * Text keeps 4.5:1 or better on the ground, on the panel and on the rows it is
 * drawn over, and the accent drawn as a mark — an icon, a ring, an outline —
 * keeps 3:1 on each of them. The rows a person tells apart stand on grounds of
 * different lightness and not only of different hue: the ground darkest, a drop
 * target a step above it (and ringed in the accent), a selected row above that,
 * and the focused row on the accent itself. Where the light set darkens the
 * accent for text — `accentDeep` on a tint of it — this one lightens it, which
 * is what the same part asks for on a dark ground.
 */
export const DARK: Readonly<Record<ColorName, string>> = {
  ground: '#15171c',
  panel: '#1d2026',
  border: '#363b45',
  rowRule: '#262a31',
  text: '#e6e8eb',
  dim: '#a6acb8',
  faint: '#959ca8',
  accent: '#3271e3',
  accentDeep: '#93b8fb',
  accentHover: '#2559bb',
  onAccent: '#ffffff',
  picked: '#162a51',
  accentTint: '#142240',
  folderFill: '#2b4a80',
  folderStroke: '#7aa5f5',
  hover: '#262a32',
  refused: '#f97066',
  refusedGround: '#3a1c1b',
  warn: '#e0b13e',
  warnDeep: '#f2d27a',
  warnGround: '#33290f',
  success: '#4ac26b',
  present: '#c3c8d1',
  backdrop: 'rgba(0, 0, 0, 0.6)',
};

/** The custom property the page's root holds the colour named `name` in. */
export function colorProperty(name: ColorName): string {
  return `--color-${name}`;
}

/**
 * The explorer's colours, by name: what every component draws in. Each is the
 * custom property of that name, which holds the light value or the dark one
 * according to the set showing.
 */
export const COLOR = Object.fromEntries(
  Object.keys(LIGHT).map((name) => [name, `var(${colorProperty(name as ColorName)})`]),
) as Readonly<Record<ColorName, string>>;

/**
 * The reader's sheet, which stays dark whichever set the screen is drawn in: a
 * picture is read best on a dark ground, and the sheet reads as a layer over
 * the tree rather than a fourth region of it.
 */
export const SHEET_COLOR = {
  ground: '#111214',
  text: '#e6e8eb',
  /** Its secondary text: where the row stands, the position, captions. */
  dim: '#a3a8b0',
  /** Its lines and frames: a step off the ground, as `border` is on the screen. */
  border: '#2a2d33',
  /** What stands on its ground — its buttons, the sheets fanned behind a cover — as `panel` does on the screen. */
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
