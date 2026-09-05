import { describe, expect, it } from 'vitest';

import { errorCode } from '../errors.testing.js';
import { MAX_FORMAT_INTEGER } from '../internal/bytes.js';
import { Generation } from '../model/generation.js';
import { ReplicaPosition } from '../model/replicaPosition.js';
import { CONTROL_OBJECT_KINDS, type ControlObjectKind } from '../model/kinds.js';
import {
  controlObjectNamesEqual,
  formatControlObjectName,
  headName,
  indexSnapshotName,
  keyringReplicaName,
  nameAdmitsKind,
  parseControlObjectName,
  successorName,
} from './objectName.js';

function keyring(index: number, count: number) {
  return keyringReplicaName(Generation.of(12n), 'a1b2', ReplicaPosition.of(index, count));
}

describe('control-object names', () => {
  // FM-12: control objects are named `head-<generation>.cfrt`,
  // `idx-<generation>.cfrt`, and
  // `key-<generation>-<set_digest>-r<index>-of-<count>.cfrt`.
  it('spells the forms the rule defines', () => {
    expect(formatControlObjectName(headName(Generation.of(4n)))).toBe('head-4.cfrt');
    expect(formatControlObjectName(indexSnapshotName(Generation.of(4n)))).toBe('idx-4.cfrt');
    expect(formatControlObjectName(keyring(1, 3))).toBe('key-12-a1b2-r1-of-3.cfrt');
  });

  // FM-12, FM-13: the successor of a head takes the head's generation plus 1,
  // whichever kind ends up winning the slot.
  it('names the successor of a head one generation on', () => {
    expect(formatControlObjectName(successorName(Generation.of(4n)))).toBe('head-5.cfrt');
    expect(errorCode(() => successorName(Generation.of(MAX_FORMAT_INTEGER)))).toBe(
      'generation_out_of_range',
    );
  });

  // FM-12: every form round-trips, so a name written by one device is read back
  // to the same values by another.
  it('round-trips every form', () => {
    const names = [
      headName(Generation.FIRST),
      headName(Generation.of(MAX_FORMAT_INTEGER)),
      indexSnapshotName(Generation.of(9n)),
      keyring(0, 1),
      keyring(2, 3),
    ];
    for (const name of names) {
      const parsed = parseControlObjectName(formatControlObjectName(name));
      expect(controlObjectNamesEqual(parsed, name), formatControlObjectName(name)).toBe(true);
    }
  });

  // FM-12: heads and Index Snapshots use replica index 0, count 1.
  it('gives the single-written forms replica zero of one', () => {
    for (const name of [headName(Generation.of(1n)), indexSnapshotName(Generation.of(1n))]) {
      expect(name.replica.index).toBe(0);
      expect(name.replica.count).toBe(1);
      expect(name.setDigest).toBeUndefined();
    }
  });

  // FM-12: the admission table — `head-` admits a Journal record or an
  // activation Index Snapshot, `idx-` only an ordinary Index Snapshot, `key-`
  // only a Keyring replica.
  it('admits exactly the kinds the table lists', () => {
    const table: [ReturnType<typeof headName>, ControlObjectKind[]][] = [
      [headName(Generation.FIRST), ['journal', 'activation-snapshot']],
      [indexSnapshotName(Generation.FIRST), ['index-snapshot']],
      [keyring(0, 1), ['keyring']],
    ];
    for (const [name, admitted] of table) {
      for (const kind of CONTROL_OBJECT_KINDS) {
        expect(
          nameAdmitsKind(name, kind),
          `${formatControlObjectName(name)} and ${kind}`,
        ).toBe(admitted.includes(kind));
      }
    }
  });

  it('carries the digest a Keyring replica name spells', () => {
    expect(keyring(0, 1).setDigest).toBe('a1b2');
  });

  // FM-12: numbers are spelled in decimal without leading zeros, the digest is
  // lowercase hex, and nothing outside the three forms is a control-object name.
  it('rejects names outside the forms', () => {
    const names = [
      'head-4', // no extension
      'head-.cfrt', // no generation
      'head-04.cfrt', // a second spelling of generation 4
      'head-4x.cfrt', // not a number
      'head--4.cfrt', // signed, in effect
      // FM-19: digits spelling a generation this format does not carry name no
      // object, so they are refused the way a leading zero is.
      'head-9223372036854775808.cfrt',
      'log-4.cfrt', // not a role coffret writes
      'key-12-a1b2-r1-of.cfrt', // no replica count
      'key-12-a1b2-r1-to-3.cfrt', // not the `of` separator
      'key-12-a1b2-1-of-3.cfrt', // no `r` on the index
      '0011.cfrt', // a Container, not a control object
    ];
    for (const name of names) {
      expect(errorCode(() => parseControlObjectName(name)), `${name} should not parse`).toBe(
        'malformed_object_name',
      );
    }
  });

  // FM-12: a name whose shape is a replica's but whose digest field is not the
  // lowercase hex token is a Keyring replica with a corrupt field, not an object
  // of some other form. A reader scanning Storage acts differently on the two,
  // so the two refusals stay apart.
  it('rejects a replica name with a bad digest for the digest', () => {
    const names = [
      'key-12-zz-r1-of-3.cfrt', // not hex
      'key-12-A1B2-r1-of-3.cfrt', // not lowercase
      'key-12--r1-of-3.cfrt', // no digest at all
    ];
    for (const name of names) {
      expect(
        errorCode(() => parseControlObjectName(name)),
        `${name} should be refused for its digest`,
      ).toBe('invalid_set_digest');
    }
  });

  // FM-12: a replica index outside its count names no replica, whatever the rest
  // of the name says.
  it('rejects a name with an inconsistent replica position', () => {
    expect(errorCode(() => parseControlObjectName('key-12-a1b2-r3-of-3.cfrt'))).toBe(
      'invalid_replica_position',
    );
  });

  it('rejects a Keyring name with a digest that is not lowercase hex', () => {
    expect(
      errorCode(() => keyringReplicaName(Generation.FIRST, 'ZZ', ReplicaPosition.SINGLE)),
    ).toBe('invalid_set_digest');
  });
});
