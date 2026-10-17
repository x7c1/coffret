/**
 * Test support: building wire maps by hand.
 *
 * Excluded from the package build — nothing here ships.
 */

import { type MapFields } from './fieldNumbers.js';

/**
 * {@link toWire} for a case that builds a map by hand: a name `kind` declares
 * becomes its field number, and any other map key — an unknown field number, a
 * text key — is kept as the case wrote it, so a case can put one beside the
 * fields it names.
 */
export function toWireLenient(value: unknown, kind: MapFields): unknown {
  if (value instanceof Map) {
    const wire = new Map<unknown, unknown>();
    for (const [name, inner] of value) {
      const field =
        typeof name === 'string'
          ? kind.fields.find((candidate) => candidate.retired !== true && candidate.name === name)
          : undefined;
      if (field === undefined) {
        wire.set(name, inner);
      } else {
        wire.set(
          field.number,
          field.holds === undefined ? inner : toWireLenient(inner, field.holds()),
        );
      }
    }
    return wire;
  }
  if (Array.isArray(value)) {
    return value.map((item) => toWireLenient(item, kind));
  }
  return value;
}
