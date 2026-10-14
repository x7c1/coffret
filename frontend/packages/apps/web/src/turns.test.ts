import { describe, expect, it } from 'vitest';

import type { Delete, Freeze, Sync } from '@coffret/api';

import { heldBy, waitingClause, waitsFor } from './turns';

const MOVING = { phase: 'uploading', done: 1, total: 2, bytes: null } as const;

function syncing(over: Partial<Sync> = {}): Sync {
  return {
    run: 1,
    step: null,
    waits: false,
    status: 'syncing',
    added: 0,
    findings: [],
    stopped: null,
    ...over,
  } as Sync;
}

function freezing(over: Partial<Freeze> = {}): Freeze {
  return {
    run: 1,
    step: null,
    waits: false,
    waiting: [],
    discarded: [],
    displaced: [],
    folder: 'books/vol-1',
    status: 'freezing',
    packs: 0,
    entries: 0,
    findings: [],
    stopped: null,
    ...over,
  } as Freeze;
}

function deleting(over: Partial<Delete> = {}): Delete {
  return {
    run: 1,
    folder: 'albums',
    paths: [],
    entries: 0,
    bytes: 0,
    removed: 0,
    rebuilt: 0,
    rebuild_read: 0,
    rebuild_written: 0,
    refused: [],
    missing: [],
    findings: [],
    step: null,
    waits: false,
    waiting: 0,
    status: 'deleting',
    stopped: null,
    ...over,
  } as Delete;
}

describe('what a run waiting its turn waits for', () => {
  it('names the flow holding the turn, as the server says it', () => {
    const turns = {
      freeze: freezing({ waits: true }),
      sync: syncing({ waits: true }),
      deletion: deleting(),
    };
    expect(waitsFor('sync', { ...turns, turn: 'delete' })).toBe('deletion');
    expect(waitsFor('freeze', { ...turns, turn: 'delete' })).toBe('deletion');
    expect(waitsFor('delete', { ...turns, turn: 'delete' })).toBeNull();
    expect(
      waitsFor('delete', { ...turns, deletion: deleting({ waits: true }), turn: 'sync' }),
    ).toBe('backup');
    expect(waitsFor('sync', { ...turns, turn: 'freeze' })).toBe('packing');
  });

  // The window the guess read backwards in: a run that has taken the turn and
  // not said a step yet. The server says it is not waiting, and that is the
  // whole of the answer — whatever else is running beside it.
  it('reports no wait for a run the server says is not waiting, step or no step', () => {
    const turns = {
      turn: 'sync' as const,
      freeze: freezing({ waits: true }),
      sync: syncing({ waits: false, step: null }),
      deletion: deleting({ status: 'done', stopped: null }),
    };
    expect(waitsFor('sync', turns)).toBeNull();
    expect(waitsFor('freeze', turns)).toBe('backup');
  });

  it('never infers from a step', () => {
    // A waiting run with a step is still waiting, and a holder with none is
    // still the holder.
    const turns = {
      turn: 'delete' as const,
      freeze: null,
      sync: syncing({ waits: true, step: MOVING }),
      deletion: deleting({ step: null }),
    };
    expect(waitsFor('sync', turns)).toBe('deletion');
    expect(waitsFor('delete', turns)).toBeNull();
  });

  it('says nothing where nobody holds the turn', () => {
    expect(
      waitsFor('sync', {
        turn: null,
        freeze: null,
        sync: syncing({ waits: true }),
        deletion: null,
      }),
    ).toBeNull();
  });

  it('says nothing for a flow with no run on record', () => {
    expect(waitsFor('freeze', { turn: 'sync', freeze: null, sync: syncing(), deletion: null }))
      .toBeNull();
  });

  it('names whoever holds the turn for a run not armed yet', () => {
    expect(heldBy('delete')).toBe('deletion');
    expect(heldBy('freeze')).toBe('packing');
    expect(heldBy(null)).toBeNull();
  });

  it('says it as one clause for every line', () => {
    expect(waitingClause(null)).toBe('');
    expect(waitingClause('deletion')).toBe(' — waiting for the deletion under way to finish');
  });
});
