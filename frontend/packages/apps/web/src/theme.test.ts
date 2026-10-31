// @vitest-environment node
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { COLOR, colorProperty, DARK, LIGHT } from './theme';

/** This directory: the explorer's components, and `theme.ts` beside them. */
const HERE = fileURLToPath(new URL('.', import.meta.url));

/**
 * A colour written out: a hex code — three, four, six or eight digits, after a
 * `#` that does not follow a word character or a `&` (an `&#123;` entity, an
 * `a#b` fragment) — or an `rgb(`/`rgba(` call.
 */
const LITERAL = /(?<![\w&])#(?:[0-9a-f]{8}|[0-9a-f]{6}|[0-9a-f]{3,4})\b|\brgba?\(/gi;

/** Each literal in `source`, as `line: text`. */
function literalsIn(source: string): string[] {
  return source.split('\n').flatMap((line, at) => [...line.matchAll(LITERAL)].map((found) => `${at + 1}: ${found[0]}`));
}

// The components draw in names only — `COLOR`, `SHEET_COLOR` and `SHADOW` of
// `theme.ts` — so that a second set of values is a change there and nowhere
// else. A shadow is a colour too, and is held to the same.
describe('the explorer’s colours', () => {
  it('are written out nowhere but theme.ts', () => {
    const components = readdirSync(HERE).filter((name) => name.endsWith('.tsx') && !/\.test\.tsx$/.test(name));
    expect(components.length).toBeGreaterThan(0);
    const found = Object.fromEntries(
      components
        .map((name) => [name, literalsIn(readFileSync(join(HERE, name), 'utf8'))] as const)
        .filter(([, literals]) => literals.length > 0),
    );
    expect(found).toEqual({});
  });

  // The pattern itself, so that the case above failing to find anything is
  // the components being clean rather than the pattern matching nothing.
  it('are found where they are written out', () => {
    expect(literalsIn("color: '#16181d',\nfill: '#FFF', ring: '#2563ebcc'")).toEqual(['1: #16181d', '2: #FFF', '2: #2563ebcc']);
    expect(literalsIn("boxShadow: '0 4px 16px rgba(0, 0, 0, 0.5)', c: 'rgb(1,2,3)'")).toEqual(['1: rgba(', '1: rgb(']);
    expect(literalsIn("&#123; a#fff 'M14 3H6'")).toEqual([]);
  });
});

/** The page itself, whose styles paint the ground and the hover states. */
const PAGE = readFileSync(join(HERE, '..', 'index.html'), 'utf8');

// `index.html` paints with the same custom properties as the components, since
// a pseudo-element such as a placeholder takes no inline style: the one a
// field's placeholder is drawn in is the faint one, what stands in for a value.
describe('the page’s own styles', () => {
  it('draw a field’s placeholder in the faint colour', () => {
    const rule = /input::placeholder[^{]*\{([^}]*)\}/.exec(PAGE)?.[1] ?? '';
    expect(rule).toContain(`color: ${COLOR.faint};`);
  });

  // So that the ground and the hovers follow the set shown, as the rest does.
  it('write no colour out, and name only colours the explorer has', () => {
    expect(literalsIn(PAGE)).toEqual([]);
    const named = [...PAGE.matchAll(/var\(--color-(\w+)\)/g)].map((found) => found[1]);
    expect(named.length).toBeGreaterThan(0);
    expect(named.filter((name) => !(name in COLOR))).toEqual([]);
  });
});

/** A colour's relative luminance, as WCAG reckons contrast from it. */
function luminance(hex: string): number {
  const value = Number.parseInt(hex.slice(1), 16);
  const [red, green, blue] = [value >> 16, (value >> 8) & 0xff, value & 0xff].map((channel) => {
    const linear = channel / 255;
    return linear <= 0.03928 ? linear / 12.92 : ((linear + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}

/** How far apart two colours stand, by WCAG's ratio. */
function contrast(one: string, other: string): number {
  const [lighter, darker] = [luminance(one), luminance(other)].sort((a, b) => b - a);
  return (lighter + 0.05) / (darker + 0.05);
}

type Name = keyof typeof LIGHT;

/**
 * Text, and what it is drawn on, in the dark set: every pair a person reads.
 * The light set is not held here to the same, since its faint colour — what
 * stands in for a value — stands at about 3:1 on its ground.
 */
const READ: readonly (readonly [Name, readonly Name[]])[] = [
  ['text', ['ground', 'panel', 'hover', 'picked', 'accentTint', 'refusedGround', 'warnGround']],
  ['dim', ['ground', 'panel', 'hover', 'picked', 'accentTint']],
  ['faint', ['ground', 'panel', 'hover', 'picked']],
  ['onAccent', ['accent', 'accentHover']],
  ['accentDeep', ['accentTint', 'ground', 'panel', 'picked']],
  ['refused', ['ground', 'panel', 'refusedGround', 'hover']],
  ['warn', ['ground', 'panel', 'warnGround']],
  ['warnDeep', ['warnGround']],
];

// The two sets name the same colours: a component draws in a name, and a name
// the dark set had no value for would draw nothing at all.
describe('the two sets', () => {
  it('give every name a value in each', () => {
    expect(Object.keys(DARK).sort()).toEqual(Object.keys(LIGHT).sort());
    expect(Object.keys(COLOR).sort()).toEqual(Object.keys(LIGHT).sort());
    const valid = /^#[0-9a-f]{6}$|^rgba\(\d+, \d+, \d+, (0|1|0?\.\d+)\)$/;
    for (const set of [LIGHT, DARK]) {
      expect(Object.entries(set).filter(([, value]) => !valid.test(value))).toEqual([]);
    }
  });

  it('are drawn through one custom property per name', () => {
    for (const name of Object.keys(LIGHT) as Name[]) {
      expect(COLOR[name]).toBe(`var(${colorProperty(name)})`);
    }
  });

  it('keep the dark set’s text at 4.5:1 or better on what it is drawn on', () => {
    const short = READ.flatMap(([ink, grounds]) =>
      grounds
        .map((ground) => [`${ink} on ${ground}`, contrast(DARK[ink], DARK[ground])] as const)
        .filter(([, ratio]) => ratio < 4.5),
    );
    expect(short).toEqual([]);
  });

  // The accent drawn as a mark rather than as text — a file's icon, the ring
  // of a file on its way, a dialog's outline — holds 3:1, as a mark has to be
  // made out, on everything it is drawn on: the ground, the panel, a row under
  // the pointer, a selected row and a drop target. In both sets.
  it('keep the accent as a mark at 3:1 or better on what it is drawn on', () => {
    const grounds = ['ground', 'panel', 'hover', 'picked', 'accentTint'] as const;
    for (const [set, colors] of [['light', LIGHT], ['dark', DARK]] as const) {
      const short = grounds
        .map((ground) => [`${set}: accent on ${ground}`, contrast(colors.accent, colors[ground])] as const)
        .filter(([, ratio]) => ratio < 3);
      expect(short).toEqual([]);
    }
  });

  // The focused row is the accent itself, and its name is read on it.
  it('keep the focused row’s text at 4.5:1 or better on the accent, in both sets', () => {
    for (const colors of [LIGHT, DARK]) {
      expect(contrast(colors.onAccent, colors.accent)).toBeGreaterThanOrEqual(4.5);
    }
  });

  // The rows a person tells apart stand on grounds of different lightness,
  // not only of a different hue: the ground, then a drop target, then a
  // selected row, then the focused row on the accent.
  it('tell the drop target, the selected row and the focused row apart by lightness', () => {
    for (const set of [LIGHT, DARK]) {
      const rows = (['accentTint', 'picked', 'accent'] as const).map((name) => luminance(set[name]));
      const ground = luminance(set.ground);
      const order = set === DARK ? [ground, ...rows] : [ground, ...rows].reverse();
      expect(order).toEqual([...order].sort((a, b) => a - b));
      for (let at = 1; at < order.length; at += 1) {
        expect((order[at] + 0.05) / (order[at - 1] + 0.05)).toBeGreaterThan(1.05);
      }
    }
  });
});
