// @vitest-environment node
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

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
