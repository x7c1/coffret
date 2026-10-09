import { describe, expect, it } from 'vitest';

import type { Delete, Freeze, Sync } from '@coffret/api';

import { waitingClause, waitsFor } from './turns';

const MOVING = { phase: 'uploading', done: 1, total: 2, bytes: null } as const;

function syncing(over: Partial<Sync> = {}): Sync {
  return {
    run: 1,
    step: null,
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
    waiting: 0,
    status: 'deleting',
    stopped: null,
    ...over,
  } as Delete;
}

describe('what a run waiting its turn waits for', () => {
  it('names the other flow under way, never its own', () => {
    const turns = { freeze: freezing(), sync: syncing(), deletion: deleting() };
    expect(waitsFor('sync', { ...turns, deletion: null })).toBe('packing');
    expect(waitsFor('freeze', { ...turns, deletion: null })).toBe('backup');
    expect(waitsFor('deletion', { ...turns, freeze: null })).toBe('backup');
    expect(waitsFor('sync', { ...turns, freeze: null })).toBe('deletion');
  });

  it('names nothing where nothing else is under way', () => {
    expect(waitsFor('sync', { freeze: null, sync: syncing(), deletion: null })).toBeNull();
    expect(
      waitsFor('freeze', {
        freeze: freezing(),
        sync: syncing({ status: 'done' }),
        deletion: deleting({ status: 'done' }),
      }),
    ).toBeNull();
  });

  // Of two others under way, one is itself waiting: the one holding the turn
  // is the one that has said how far it has got.
  it('names the one that is moving where two others are under way', () => {
    const deletionMoving = {
      freeze: freezing(),
      sync: syncing(),
      deletion: deleting({ step: MOVING }),
    };
    expect(waitsFor('freeze', deletionMoving)).toBe('deletion');
    const syncMoving = {
      freeze: freezing(),
      sync: syncing({ step: MOVING }),
      deletion: deleting(),
    };
    expect(waitsFor('deletion', syncMoving)).toBe('backup');
    const noneMoving = { freeze: freezing(), sync: syncing(), deletion: deleting() };
    expect(waitsFor('deletion', noneMoving)).toBe('packing');
  });

  it('says it as one clause for every line', () => {
    expect(waitingClause(null)).toBe('');
    expect(waitingClause('deletion')).toBe(' — waiting for the deletion under way to finish');
  });
});
