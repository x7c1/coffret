import { describe, expect, it } from 'vitest';

import { errorCode } from '../errors.testing.js';
import { Generation } from '../model/generation.js';
import { ReplicaPosition } from '../model/replicaPosition.js';
import {
  controlObjectNamesEqual,
  formatControlObjectName,
  indexSnapshotName,
  journalName,
  keyringReplicaName,
  parseControlObjectName,
} from './objectName.js';

function keyring(index: number, count: number) {
  return keyringReplicaName(Generation.of(12n), 'a1b2', ReplicaPosition.of(index, count));
}

describe('control-object names', () => {
  // FM-12: control objects are named `jrn-<generation>.cfrt`,
  // `idx-<generation>.cfrt`, and
  // `key-<generation>-<set_digest>-r<index>-of-<count>.cfrt`.
  it('spells the forms the rule defines', () => {
    expect(formatControlObjectName(journalName(Generation.of(4n)))).toBe('jrn-4.cfrt');
    expect(formatControlObjectName(indexSnapshotName(Generation.of(4n)))).toBe('idx-4.cfrt');
    expect(formatControlObjectName(keyring(1, 3))).toBe('key-12-a1b2-r1-of-3.cfrt');
  });

  // FM-12: every form round-trips, so a name written by one device is read back
  // to the same values by another.
  it('round-trips every form', () => {
    const names = [
      journalName(Generation.FIRST),
      journalName(Generation.of((1n << 64n) - 1n)),
      indexSnapshotName(Generation.of(9n)),
      keyring(0, 1),
      keyring(2, 3),
    ];
    for (const name of names) {
      const parsed = parseControlObjectName(formatControlObjectName(name));
      expect(controlObjectNamesEqual(parsed, name), formatControlObjectName(name)).toBe(true);
    }
  });

  // FM-12: Journal records and Index Snapshots use replica index 0, count 1.
  it('gives the single-written kinds replica zero of one', () => {
    for (const name of [journalName(Generation.of(1n)), indexSnapshotName(Generation.of(1n))]) {
      expect(name.replica.index).toBe(0);
      expect(name.replica.count).toBe(1);
      expect(name.setDigest).toBeUndefined();
    }
  });

  it('reports the kind a name belongs to', () => {
    expect(journalName(Generation.FIRST).kind).toBe('journal');
    expect(indexSnapshotName(Generation.FIRST).kind).toBe('index-snapshot');
    expect(keyring(0, 1).kind).toBe('keyring');
    expect(keyring(0, 1).setDigest).toBe('a1b2');
  });

  // FM-12: numbers are spelled in decimal without leading zeros, the digest is
  // lowercase hex, and nothing outside the three forms is a control-object name.
  it('rejects names outside the forms', () => {
    const names = [
      'jrn-4', // no extension
      'jrn-.cfrt', // no generation
      'jrn-04.cfrt', // a second spelling of generation 4
      'jrn-4x.cfrt', // not a number
      'jrn--4.cfrt', // signed, in effect
      'log-4.cfrt', // not a kind coffret writes
      'key-12-a1b2-r1-of.cfrt', // no replica count
      'key-12-a1b2-r1-to-3.cfrt', // not the `of` separator
      'key-12-a1b2-1-of-3.cfrt', // no `r` on the index
      'key-12-zz-r1-of-3.cfrt', // digest is not hex
      'key-12-A1B2-r1-of-3.cfrt', // digest is not lowercase
      'key-12--r1-of-3.cfrt', // no digest
      '0011.cfrt', // a Container, not a control object
    ];
    for (const name of names) {
      expect(errorCode(() => parseControlObjectName(name)), `${name} should not parse`).toBe(
        'malformed_object_name',
      );
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
    ).toBe('malformed_object_name');
  });
});
