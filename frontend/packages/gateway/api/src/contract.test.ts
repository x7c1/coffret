// The explorer's half of the contract with the server.
//
// The three files under `contract/` are what `coffret-server` sends, written by
// its own cases: every refusal a route can answer with, every state the
// work answer can be in, and one of every other answer a route gives, taken
// from a real Library through the routes. Those cases fail when the server
// sends something the files do not hold; these fail when the files hold
// something this client does not read as itself. Between the two, a field, a
// kind or a status that changes on one side cannot go unnoticed on the other.
//
// What "read as itself" means is the part the hand-written cases beside this
// one cannot say. A refusal is read through `refusalOf`, which is the decoder
// every screen's refusal goes through. An answer is read through a narrowing
// that knows every field the type declares and every literal each union
// admits, and nothing else — so a field the server grew, a field it dropped,
// and a status spelled differently all stop here. The tables of literals are
// `Record`s over the unions themselves, which makes the compiler hold them to
// the types in both directions: a literal added to a union and not here, or
// written here and not in the union, does not compile.

import { expect, it } from 'vitest';

import type {
  Work,
  ByteCount,
  Catalog,
  CatalogState,
  DeclinedEntry,
  Delete,
  DeletePreview,
  DeleteStatus,
  DisplacedFill,
  DisplacedFreeze,
  Fill,
  FillStatus,
  Finding,
  FindingReason,
  Freeze,
  FreezePreview,
  FreezeStatus,
  LibraryState,
  Phase,
  Reconnect,
  ReconnectState,
  RefusedPack,
  PackRefusalReason,
  Step,
  Stopped,
  Sync,
  SyncStatus,
} from './work';
import { workOf } from './work';
import workAnswers from './contract/work.json';
import answers from './contract/answers.json';
import refusals from './contract/refusals.json';
import type { Browsed } from './browse';
import type { Folders } from './folders';
import type { Library } from './library';
import type { ContainerKind, EntryState, ListedFile, ListedFolder, Listing } from './list';
import type { Mapped, MappedMarker } from './map';
import type { Reconnecting } from './reconnect';
import type { Unlocking } from './unlock';
import type { Refreshed } from './refresh';
import type { PlacementReason, Refused, RefusalKind, SurfacedFinding } from './refusal';
import { NO_FOLDER_HERE, refusalOf } from './refusal';
import type { RefusedPart, Upload } from './upload';
import { uploadOf } from './upload';

/** Every literal of a union, as a table the compiler holds to it. */
type Literals<T extends string> = Record<T, true>;

/**
 * The kinds the server can send. The two this client mints where no answer of
 * the server's shape arrived are marked as such, so that the table still covers
 * the whole union and a case can say which of them the files must hold.
 */
const KINDS: Record<RefusalKind, 'server' | 'client'> = {
  bad_path: 'server',
  bad_request: 'server',
  unauthorized: 'server',
  no_such_entry: 'server',
  no_such_route: 'server',
  declined: 'server',
  refused_placement: 'server',
  epoch: 'server',
  conflict: 'server',
  locked: 'server',
  storage: 'server',
  unverified: 'server',
  server: 'server',
  unreachable: 'client',
  unrecognized: 'client',
};

const PLACEMENT_REASONS: Literals<PlacementReason> = {
  unmapped: true,
  unmaterializable: true,
  reserved: true,
  refused_root: true,
  surfaced: true,
  key_lost: true,
  pack_resident: true,
  unauthenticated: true,
};

/**
 * Which finding names a refusal carries, as against the ones only a run's
 * finding does — nothing a fetch does is declined over a file that changed
 * inside a Pack, one this device no longer has, or one whose Entry left the
 * Library.
 */
const SURFACED: Record<SurfacedFinding, 'refusal' | 'finding'> = {
  ForeignFile: 'refusal',
  LocallyChanged: 'refusal',
  WitnessedDeletion: 'refusal',
  UnreachablePlace: 'refusal',
  KeyLost: 'refusal',
  ReservedComponent: 'refusal',
  ChangedInPack: 'finding',
  DeletedLocally: 'finding',
  MovedToTrash: 'finding',
  KeptEdited: 'finding',
  MoveToTrashRefused: 'finding',
};

const FINDING_REASONS: Literals<FindingReason> = {
  surfaced: true,
  key_lost: true,
  refused_root: true,
  root_missing: true,
  root_on_another_filesystem: true,
  keyring_degraded: true,
  keyring_repaired: true,
  parcel_unheld: true,
};
const FILL_STATUSES: Literals<FillStatus> = {
  filling: true,
  done: true,
  stopped: true,
  superseded: true,
};
const SYNC_STATUSES: Literals<SyncStatus> = { syncing: true, done: true, stopped: true };
const FREEZE_STATUSES: Literals<FreezeStatus> = { freezing: true, done: true, stopped: true };
const DELETE_STATUSES: Literals<DeleteStatus> = { deleting: true, done: true, stopped: true };
const PACK_REFUSAL_REASONS: Literals<PackRefusalReason> = { key_lost: true, unverified: true };
const PHASES: Literals<Phase> = {
  catching_up: true,
  settling: true,
  scanning: true,
  packing: true,
  uploading: true,
  committing: true,
  fetching: true,
};
const CATALOG_STATES: Literals<CatalogState> = {
  catching_up: true,
  caught_up: true,
  behind: true,
};
const LIBRARY_STATES: Literals<LibraryState> = { locked: true, unlocked: true };
const RECONNECT_STATES: Literals<ReconnectState> = {
  waiting: true,
  renewed: true,
  refused: true,
  timed_out: true,
  failed: true,
};
const ENTRY_STATES: Literals<EntryState> = { present: true, remote: true, added: true };
const CONTAINER_KINDS: Literals<ContainerKind> = { 'one-file': true, pack: true };
const MARKERS: Literals<MappedMarker> = { written: true, adopted: true, reset: true };

/**
 * Every literal a narrowing met, by the table it was read against, so that a
 * case can say the files exercised the whole of a union rather than a part.
 */
const met = new Map<object, Set<string>>();

/** `value` as a literal of the table's union, or a failure naming `where`. */
function one<T extends string>(table: Record<T, unknown>, value: unknown, where: string): T {
  if (typeof value !== 'string' || !Object.hasOwn(table, value)) {
    throw new Error(`${where}: ${JSON.stringify(value)} is not a literal this client knows`);
  }
  const seen = met.get(table) ?? new Set<string>();
  seen.add(value);
  met.set(table, seen);
  return value as T;
}

/** The literals of `table` no narrowing met. */
function unmet(table: object): string[] {
  const seen = met.get(table) ?? new Set<string>();
  return Object.keys(table).filter((literal) => !seen.has(literal));
}

type Fields = Record<string, unknown>;

/**
 * `value` as an object holding exactly these fields: every one of `required`,
 * any of `optional`, and nothing else.
 */
function object(value: unknown, where: string, required: string[], optional: string[] = []): Fields {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`${where}: expected an object, got ${JSON.stringify(value)}`);
  }
  const fields = value as Fields;
  for (const name of required) {
    if (!(name in fields)) {
      throw new Error(`${where}: the server sends no \`${name}\`, which this client reads`);
    }
  }
  for (const name of Object.keys(fields)) {
    if (!required.includes(name) && !optional.includes(name)) {
      throw new Error(`${where}: the server sends \`${name}\`, which this client does not read`);
    }
  }
  return fields;
}

function string(value: unknown, where: string): string {
  if (typeof value !== 'string') {
    throw new Error(`${where}: expected a string, got ${JSON.stringify(value)}`);
  }
  return value;
}

function number(value: unknown, where: string): number {
  if (typeof value !== 'number' || !Number.isInteger(value)) {
    throw new Error(`${where}: expected an integer, got ${JSON.stringify(value)}`);
  }
  return value;
}

function boolean(value: unknown, where: string): boolean {
  if (typeof value !== 'boolean') {
    throw new Error(`${where}: expected a boolean, got ${JSON.stringify(value)}`);
  }
  return value;
}

function nullable<T>(value: unknown, read: (value: unknown) => T): T | null {
  return value === null ? null : read(value);
}

function list<T>(value: unknown, where: string, read: (value: unknown, where: string) => T): T[] {
  if (!Array.isArray(value)) {
    throw new Error(`${where}: expected an array, got ${JSON.stringify(value)}`);
  }
  return value.map((item, index) => read(item, `${where}[${index}]`));
}

/**
 * A refusal inside an answer, read as strictly as the rest of it: every literal
 * one this client knows, or a failure. The runtime's own reading of the same
 * refusal (`refusedOf`) never throws, and lands an unknown literal where the
 * request path does; the cases below hold the two to one result over the files.
 */
function refused(value: unknown, where: string): Refused {
  const fields = object(value, where, ['error', 'message'], ['reason', 'surfaced']);
  return {
    kind: one(KINDS, fields.error, `${where}.error`),
    message: string(fields.message, `${where}.message`),
    reason:
      fields.reason === undefined ? null : one(PLACEMENT_REASONS, fields.reason, `${where}.reason`),
    surfaced:
      fields.surfaced === undefined ? null : one(SURFACED, fields.surfaced, `${where}.surfaced`),
  };
}

/**
 * A run's status and its refusal, as the one pair they may be in: a refusal
 * exactly where the status is `stopped`, and `null` everywhere else. A stopped
 * run that says nothing about why, and a running one carrying a refusal, are
 * both failures here.
 */
function standing<Status extends string>(
  table: Literals<Status>,
  fields: Fields,
  where: string,
): { status: Exclude<Status, 'stopped'>; stopped: null } | Stopped {
  const status = one(table, fields.status, `${where}.status`);
  if (status === 'stopped') {
    if (fields.stopped === null) {
      throw new Error(`${where}: a run that stopped says nothing about what stopped it`);
    }
    return { status: 'stopped', stopped: refused(fields.stopped, `${where}.stopped`) };
  }
  if (fields.stopped !== null) {
    throw new Error(`${where}: a run that is ${status} carries a refusal`);
  }
  return { status: status as Exclude<Status, 'stopped'>, stopped: null };
}

function declinedEntry(value: unknown, where: string): DeclinedEntry {
  const fields = object(value, where, ['path', 'error', 'message'], ['reason', 'surfaced']);
  const { path, ...refusal } = fields;
  return { path: string(path, `${where}.path`), ...refused(refusal, where) };
}

function finding(value: unknown, where: string): Finding {
  const fields = object(value, where, ['path', 'message', 'reason'], ['surfaced']);
  return {
    path: nullable(fields.path, (path) => string(path, `${where}.path`)),
    message: string(fields.message, `${where}.message`),
    reason: one(FINDING_REASONS, fields.reason, `${where}.reason`),
    surfaced:
      fields.surfaced === undefined ? null : one(SURFACED, fields.surfaced, `${where}.surfaced`),
  };
}

function step(value: unknown, where: string): Step {
  const fields = object(value, where, ['phase', 'done', 'total', 'bytes']);
  return {
    phase: one(PHASES, fields.phase, `${where}.phase`),
    done: number(fields.done, `${where}.done`),
    total: nullable(fields.total, (total) => number(total, `${where}.total`)),
    bytes: nullable(fields.bytes, (bytes) => byteCount(bytes, `${where}.bytes`)),
  };
}

function byteCount(value: unknown, where: string): ByteCount {
  const fields = object(value, where, ['done', 'total']);
  return {
    done: number(fields.done, `${where}.done`),
    total: number(fields.total, `${where}.total`),
  };
}

function folders(value: unknown, where: string): string[] {
  return list(value, where, string);
}

function fill(value: unknown, where: string): Fill {
  const fields = object(value, where, [
    'run',
    'folder',
    'status',
    'total',
    'done',
    'declined',
    'findings',
    'waiting',
    'discarded',
    'displaced',
    'stopped',
  ]);
  return {
    run: number(fields.run, `${where}.run`),
    folder: string(fields.folder, `${where}.folder`),
    total: number(fields.total, `${where}.total`),
    done: number(fields.done, `${where}.done`),
    declined: list(fields.declined, `${where}.declined`, declinedEntry),
    findings: list(fields.findings, `${where}.findings`, finding),
    waiting: folders(fields.waiting, `${where}.waiting`),
    discarded: folders(fields.discarded, `${where}.discarded`),
    displaced: list(fields.displaced, `${where}.displaced`, displacedFill),
    ...standing(FILL_STATUSES, fields, where),
  };
}

/** A run a later one took the record from: always stopped, and never with a queue. */
function displaced(fields: Fields, where: string): Stopped {
  // A table of the one status a displaced run can have, so any other fails as
  // a literal this client does not know there.
  return standing<'stopped'>({ stopped: true }, fields, where) as Stopped;
}

function displacedFill(value: unknown, where: string): DisplacedFill {
  const fields = object(value, where, [
    'run',
    'folder',
    'status',
    'total',
    'done',
    'declined',
    'findings',
    'stopped',
  ]);
  return {
    run: number(fields.run, `${where}.run`),
    folder: string(fields.folder, `${where}.folder`),
    total: number(fields.total, `${where}.total`),
    done: number(fields.done, `${where}.done`),
    declined: list(fields.declined, `${where}.declined`, declinedEntry),
    findings: list(fields.findings, `${where}.findings`, finding),
    ...displaced(fields, where),
  };
}

function sync(value: unknown, where: string): Sync {
  const fields = object(value, where, ['run', 'status', 'added', 'findings', 'step', 'stopped']);
  return {
    run: number(fields.run, `${where}.run`),
    added: number(fields.added, `${where}.added`),
    findings: list(fields.findings, `${where}.findings`, finding),
    step: nullable(fields.step, (value) => step(value, `${where}.step`)),
    ...standing(SYNC_STATUSES, fields, where),
  };
}

function freeze(value: unknown, where: string): Freeze {
  const fields = object(value, where, [
    'run',
    'folder',
    'status',
    'packs',
    'entries',
    'findings',
    'step',
    'waiting',
    'discarded',
    'displaced',
    'stopped',
  ]);
  return {
    run: number(fields.run, `${where}.run`),
    folder: string(fields.folder, `${where}.folder`),
    packs: number(fields.packs, `${where}.packs`),
    entries: number(fields.entries, `${where}.entries`),
    findings: list(fields.findings, `${where}.findings`, finding),
    step: nullable(fields.step, (value) => step(value, `${where}.step`)),
    waiting: folders(fields.waiting, `${where}.waiting`),
    discarded: folders(fields.discarded, `${where}.discarded`),
    displaced: list(fields.displaced, `${where}.displaced`, displacedFreeze),
    ...standing(FREEZE_STATUSES, fields, where),
  };
}

function displacedFreeze(value: unknown, where: string): DisplacedFreeze {
  const fields = object(value, where, [
    'run',
    'folder',
    'status',
    'packs',
    'entries',
    'findings',
    'step',
    'stopped',
  ]);
  return {
    run: number(fields.run, `${where}.run`),
    folder: string(fields.folder, `${where}.folder`),
    packs: number(fields.packs, `${where}.packs`),
    entries: number(fields.entries, `${where}.entries`),
    findings: list(fields.findings, `${where}.findings`, finding),
    step: nullable(fields.step, (value) => step(value, `${where}.step`)),
    ...displaced(fields, where),
  };
}

function refusedPack(value: unknown, where: string): RefusedPack {
  const fields = object(value, where, ['spared', 'kept', 'reason', 'message']);
  return {
    spared: folders(fields.spared, `${where}.spared`),
    kept: number(fields.kept, `${where}.kept`),
    reason: one(PACK_REFUSAL_REASONS, fields.reason, `${where}.reason`),
    message: string(fields.message, `${where}.message`),
  };
}

function deletion(value: unknown, where: string): Delete {
  const fields = object(value, where, [
    'run',
    'folder',
    'paths',
    'status',
    'entries',
    'bytes',
    'removed',
    'rebuilt',
    'rebuild_read',
    'rebuild_written',
    'refused',
    'missing',
    'findings',
    'step',
    'waiting',
    'stopped',
  ]);
  return {
    run: number(fields.run, `${where}.run`),
    folder: nullable(fields.folder, (folder) => string(folder, `${where}.folder`)),
    paths: folders(fields.paths, `${where}.paths`),
    entries: number(fields.entries, `${where}.entries`),
    bytes: number(fields.bytes, `${where}.bytes`),
    removed: number(fields.removed, `${where}.removed`),
    rebuilt: number(fields.rebuilt, `${where}.rebuilt`),
    rebuild_read: number(fields.rebuild_read, `${where}.rebuild_read`),
    rebuild_written: number(fields.rebuild_written, `${where}.rebuild_written`),
    refused: list(fields.refused, `${where}.refused`, refusedPack),
    missing: folders(fields.missing, `${where}.missing`),
    findings: list(fields.findings, `${where}.findings`, finding),
    step: nullable(fields.step, (value) => step(value, `${where}.step`)),
    waiting: number(fields.waiting, `${where}.waiting`),
    ...standing(DELETE_STATUSES, fields, where),
  };
}

/** How the catalog stands: a refusal exactly where it is behind, and none elsewhere. */
function catalog(value: unknown, where: string): Catalog {
  const fields = object(value, where, ['state', 'stopped']);
  const state = one(CATALOG_STATES, fields.state, `${where}.state`);
  if (state === 'behind') {
    if (fields.stopped === null) {
      throw new Error(`${where}: a catalog that is behind says nothing about what stopped it`);
    }
    return { state, stopped: refused(fields.stopped, `${where}.stopped`) };
  }
  if (fields.stopped !== null) {
    throw new Error(`${where}: a catalog that is ${state} carries a refusal`);
  }
  return { state, stopped: null };
}

function reconnect(value: unknown, where: string): Reconnect {
  const fields = object(value, where, ['state', 'message']);
  return {
    state: one(RECONNECT_STATES, fields.state, `${where}.state`),
    message: string(fields.message, `${where}.message`),
  };
}

function work(value: unknown, where: string): Work {
  const fields = object(value, where, [
    'server',
    'library',
    'catalog',
    'fill',
    'sync',
    'freeze',
    'delete',
    'reconnect',
  ]);
  return {
    server: string(fields.server, `${where}.server`),
    library: one(LIBRARY_STATES, fields.library, `${where}.library`),
    catalog: catalog(fields.catalog, `${where}.catalog`),
    fill: nullable(fields.fill, (value) => fill(value, `${where}.fill`)),
    sync: nullable(fields.sync, (value) => sync(value, `${where}.sync`)),
    freeze: nullable(fields.freeze, (value) => freeze(value, `${where}.freeze`)),
    delete: nullable(fields.delete, (value) => deletion(value, `${where}.delete`)),
    reconnect: nullable(fields.reconnect, (value) => reconnect(value, `${where}.reconnect`)),
  };
}

function listedFolder(value: unknown, where: string): ListedFolder {
  const fields = object(value, where, ['name', 'path', 'mapped']);
  return {
    name: string(fields.name, `${where}.name`),
    path: string(fields.path, `${where}.path`),
    mapped: boolean(fields.mapped, `${where}.mapped`),
  };
}

function listedFile(value: unknown, where: string): ListedFile {
  const fields = object(value, where, [
    'name',
    'path',
    'size',
    'mtime',
    'state',
    'container',
    'openable',
    'content_type',
  ]);
  return {
    name: string(fields.name, `${where}.name`),
    path: string(fields.path, `${where}.path`),
    size: number(fields.size, `${where}.size`),
    mtime: nullable(fields.mtime, (mtime) => string(mtime, `${where}.mtime`)),
    state: one(ENTRY_STATES, fields.state, `${where}.state`),
    container: nullable(fields.container, (kind) =>
      one(CONTAINER_KINDS, kind, `${where}.container`),
    ),
    openable: boolean(fields.openable, `${where}.openable`),
    content_type: string(fields.content_type, `${where}.content_type`),
  };
}

function listing(value: unknown, where: string): Listing {
  const fields = object(value, where, [
    'path',
    'mapped',
    'held',
    'folders',
    'files',
    'folders_on_disk',
  ]);
  return {
    path: string(fields.path, `${where}.path`),
    mapped: boolean(fields.mapped, `${where}.mapped`),
    held: boolean(fields.held, `${where}.held`),
    folders: list(fields.folders, `${where}.folders`, listedFolder),
    files: list(fields.files, `${where}.files`, listedFile),
    folders_on_disk: list(fields.folders_on_disk, `${where}.folders_on_disk`, string),
  };
}

function refusedPart(value: unknown, where: string): RefusedPart {
  const fields = object(value, where, ['name', 'error', 'message'], ['reason', 'surfaced']);
  const { name, ...refusal } = fields;
  return { name: string(name, `${where}.name`), ...refused(refusal, where) };
}

function upload(value: unknown, where: string): Upload {
  const fields = object(value, where, ['written', 'refused']);
  return {
    written: folders(fields.written, `${where}.written`),
    refused: list(fields.refused, `${where}.refused`, refusedPart),
  };
}

// Every refusal the server can send reaches a screen as itself: the kind it
// was sent as rather than `unrecognized`, and the reason and the finding it
// named rather than `null` — which every screen would show as the generic
// sentence, with nothing saying that anything had gone missing.
it('reads every refusal the server sends as the refusal it is', async () => {
  const kinds = new Set<string>();
  const reasons = new Set<string>();
  const findings = new Set<string>();

  for (const [index, sent] of refusals.entries()) {
    const where = `refusals[${index}]`;
    const body = object(sent.body, where, ['error', 'message'], ['reason', 'surfaced', 'written']);
    const refusal = await refusalOf(
      new Response(JSON.stringify(body), {
        status: sent.status,
        headers: { 'content-type': 'application/json' },
      }),
    );

    expect(refusal.kind, where).toBe(one(KINDS, body.error, `${where}.error`));
    expect(refusal.status, where).toBe(sent.status);
    expect(refusal.message, where).toBe(body.message);
    expect(refusal.reason, where).toBe(
      body.reason === undefined ? null : one(PLACEMENT_REASONS, body.reason, `${where}.reason`),
    );
    expect(refusal.surfaced, where).toBe(
      body.surfaced === undefined ? null : one(SURFACED, body.surfaced, `${where}.surfaced`),
    );
    expect(refusal.written, where).toEqual(
      body.written === undefined ? null : folders(body.written, `${where}.written`),
    );

    kinds.add(refusal.kind);
    if (refusal.reason !== null) {
      reasons.add(refusal.reason);
    }
    if (refusal.surfaced !== null) {
      findings.add(refusal.surfaced);
    }
  }

  // And the file is the whole of what the server sends, so a kind, a reason
  // or a finding name it lacks is one this client has never been shown.
  const serverKinds = Object.entries(KINDS)
    .filter(([, minted]) => minted === 'server')
    .map(([kind]) => kind);
  expect([...kinds].sort()).toEqual(serverKinds.sort());
  expect([...reasons].sort()).toEqual(Object.keys(PLACEMENT_REASONS).sort());
  const refusalFindings = Object.entries(SURFACED)
    .filter(([, carried]) => carried === 'refusal')
    .map(([name]) => name);
  expect([...findings].sort()).toEqual(refusalFindings.sort());
});

// Every state the work answer can be in narrows to `Work`, and between
// them the answers exercise every literal of every union inside it: a status,
// a phase or a standing this client has a word for and the server never sends
// is as much a disagreement as the other way round.
it('reads every work answer the server sends through the Work type', () => {
  const read = workAnswers.map((answer, index) => work(answer, `work[${index}]`));

  expect(read.length).toBeGreaterThan(0);
  // What a page is handed is what the strict reading makes of the same answer:
  // the runtime's narrowing drops nothing and renames nothing the server sends.
  expect(workAnswers.map(workOf)).toEqual(read);
  expect(
    read.some((answer) => answer.fill?.displaced.some((run) => run.declined.length > 0)),
    'a displaced fill with the Entries it declined',
  ).toBe(true);
  expect(
    read.some((answer) => answer.freeze?.step?.bytes != null),
    'a freeze sending a Pack, with how many bytes of it have gone',
  ).toBe(true);
  for (const table of [
    FILL_STATUSES,
    SYNC_STATUSES,
    FREEZE_STATUSES,
    DELETE_STATUSES,
    PACK_REFUSAL_REASONS,
    PHASES,
    CATALOG_STATES,
    LIBRARY_STATES,
    FINDING_REASONS,
    RECONNECT_STATES,
  ]) {
    expect(unmet(table), 'literals no answer sent').toEqual([]);
  }
  expect(unmet(SURFACED), 'finding names no finding carried').toEqual([]);
});

// The answers of the other routes, taken from a real Library through them. The
// listing is the one with a case that matters most: the Library root on a
// device that maps one top-level folder and not the root — `mapped` false over
// folders that each say for themselves — which is what the explorer's first
// screen is on such a device.
it('reads every other answer the server sends through its type', () => {
  const library: Library = (() => {
    const fields = object(answers.library, 'library', ['name', 'library_id', 'provider']);
    return {
      name: string(fields.name, 'library.name'),
      library_id: string(fields.library_id, 'library.library_id'),
      provider: string(fields.provider, 'library.provider'),
    };
  })();
  const listed: Folders = {
    folders: folders(object(answers.folders, 'folders', ['folders']).folders, 'folders'),
  };
  const refreshed: Refreshed = (() => {
    const fields = object(answers.refreshed, 'refreshed', ['advanced', 'gained', 'entries']);
    return {
      advanced: boolean(fields.advanced, 'refreshed.advanced'),
      gained: number(fields.gained, 'refreshed.gained'),
      entries: number(fields.entries, 'refreshed.entries'),
    };
  })();
  const reconnecting: Reconnecting = (() => {
    const fields = object(answers.reconnecting, 'reconnecting', ['url', 'message']);
    return {
      url: string(fields.url, 'reconnecting.url'),
      message: string(fields.message, 'reconnecting.message'),
    };
  })();
  const unlocking = (name: 'already_unlocked' | 'unlocking'): Unlocking => {
    const fields = object(answers.unlock[name], `unlock.${name}`, ['library', 'message']);
    return {
      library: one(LIBRARY_STATES, fields.library, `unlock.${name}.library`),
      message: string(fields.message, `unlock.${name}.message`),
    };
  };
  const alreadyUnlocked = unlocking('already_unlocked');
  const askedTheApp = unlocking('unlocking');
  const browsed: Browsed = (() => {
    const fields = object(answers.browsed, 'browsed', ['path', 'parent', 'folders']);
    return {
      path: string(fields.path, 'browsed.path'),
      parent: nullable(fields.parent, (value) => string(value, 'browsed.parent')),
      folders: list(fields.folders, 'browsed.folders', (value, where) => {
        const folder = object(value, where, ['name', 'path']);
        return { name: string(folder.name, `${where}.name`), path: string(folder.path, `${where}.path`) };
      }),
    };
  })();
  const mapped: Mapped = (() => {
    const fields = object(answers.mapped, 'mapped', [
      'prefix',
      'local_root',
      'replaced',
      'marker',
      'message',
    ]);
    return {
      prefix: nullable(fields.prefix, (value) => string(value, 'mapped.prefix')),
      local_root: string(fields.local_root, 'mapped.local_root'),
      replaced: nullable(fields.replaced, (value) => string(value, 'mapped.replaced')),
      marker: one(MARKERS, fields.marker, 'mapped.marker'),
      message: string(fields.message, 'mapped.message'),
    };
  })();
  const preview: FreezePreview = (() => {
    const fields = object(answers.freeze_preview, 'freeze_preview', [
      'folder',
      'files',
      'bytes',
      'in_pack',
      'changed_in_pack',
      'not_here',
      'unavailable',
      'after_current',
      'already_packing',
    ]);
    return {
      folder: string(fields.folder, 'freeze_preview.folder'),
      files: number(fields.files, 'freeze_preview.files'),
      bytes: number(fields.bytes, 'freeze_preview.bytes'),
      in_pack: number(fields.in_pack, 'freeze_preview.in_pack'),
      changed_in_pack: number(fields.changed_in_pack, 'freeze_preview.changed_in_pack'),
      not_here: number(fields.not_here, 'freeze_preview.not_here'),
      unavailable: number(fields.unavailable, 'freeze_preview.unavailable'),
      after_current: boolean(fields.after_current, 'freeze_preview.after_current'),
      already_packing: boolean(fields.already_packing, 'freeze_preview.already_packing'),
    };
  })();
  const deletePreview: DeletePreview = (() => {
    const fields = object(answers.delete_preview, 'delete_preview', [
      'folder',
      'paths',
      'entries',
      'bytes',
      'removed',
      'rebuilt',
      'rebuild_read',
      'rebuild_written',
      'refused',
      'missing',
      'after_current',
    ]);
    return {
      folder: nullable(fields.folder, (value) => string(value, 'delete_preview.folder')),
      paths: folders(fields.paths, 'delete_preview.paths'),
      entries: number(fields.entries, 'delete_preview.entries'),
      bytes: number(fields.bytes, 'delete_preview.bytes'),
      removed: number(fields.removed, 'delete_preview.removed'),
      rebuilt: number(fields.rebuilt, 'delete_preview.rebuilt'),
      rebuild_read: number(fields.rebuild_read, 'delete_preview.rebuild_read'),
      rebuild_written: number(fields.rebuild_written, 'delete_preview.rebuild_written'),
      refused: list(fields.refused, 'delete_preview.refused', refusedPack),
      missing: folders(fields.missing, 'delete_preview.missing'),
      after_current: boolean(fields.after_current, 'delete_preview.after_current'),
    };
  })();
  const listings = Object.fromEntries(
    Object.entries(answers.listings).map(([name, value]) => [name, listing(value, name)]),
  );
  const uploads = {
    written: upload(answers.uploads.written, 'uploads.written'),
    refused: upload(answers.uploads.refused, 'uploads.refused'),
  };
  expect(uploadOf(answers.uploads.written)).toEqual(uploads.written);
  expect(uploadOf(answers.uploads.refused)).toEqual(uploads.refused);

  expect(library.provider).toBe('s3');
  expect(listed.folders.length).toBeGreaterThan(0);
  expect(refreshed.entries).toBeGreaterThan(0);
  expect(reconnecting.url.length).toBeGreaterThan(0);
  expect(alreadyUnlocked.library).toBe('unlocked');
  expect(askedTheApp.library).toBe('locked');
  expect(browsed.folders.map((folder) => folder.name)).toEqual(['albums', 'scans']);
  expect(browsed.parent).not.toBeNull();
  expect(mapped.local_root).toBe(browsed.folders[1].path);
  expect(mapped.prefix).toBe('books');
  expect(preview.folder).toBe('albums');
  expect(preview.files).toBeGreaterThan(0);
  expect(deletePreview.entries).toBeGreaterThan(0);
  expect(deletePreview.missing.length).toBeGreaterThan(0);
  expect(uploads.written.written.length).toBeGreaterThan(0);
  expect(uploads.refused.refused.length).toBeGreaterThan(0);

  const root = listings.unmapped_root;
  expect(root.path).toBe('');
  expect(root.mapped).toBe(false);
  expect(root.folders.map((folder) => folder.mapped)).toContain(true);
  expect(listings.root.mapped).toBe(true);
  expect(listings.nowhere.held).toBe(false);
  // A folder on disk the Library does not have, beside the one it does: named
  // in its own field and not among the catalog's folders.
  expect(listings.albums.folders_on_disk).toEqual(['extras']);
  expect(listings.albums.folders.map((folder) => folder.name)).not.toContain('extras');

  expect(unmet(ENTRY_STATES), 'row states no listing sent').toEqual([]);
  expect(unmet(CONTAINER_KINDS), 'Container kinds no listing sent').toEqual([]);
  expect(
    Object.values(listings).some((shown) => shown.files.some((file) => file.container === null)),
    'a row with no Container of its own yet',
  ).toBe(true);
});

// The one refusal a page says without asking. Clicking a row of a folder no
// mapping reaches makes no request, so the page says the server's sentence for
// it — and this holds the page's copy to the one the server sends.
it('says the unmapped sentence in the server’s words', () => {
  const unmapped = refusals
    .map((sent) => sent.body)
    .filter((body) => 'reason' in body && body.reason === 'unmapped');

  expect(unmapped.map((body) => body.error).sort()).toEqual(['declined', 'refused_placement']);
  for (const body of unmapped) {
    expect(body.message).toBe(NO_FOLDER_HERE);
  }
});

// The pairing the narrowing holds a run's two fields to, shown failing both
// ways: a run that stopped without saying why is a screen left to invent a
// cause, and one that is still going with a refusal is a sentence nobody should
// be shown. The same of a displaced run, which is only ever stopped, and of the
// catalog.
it('rejects a status and a refusal that do not go together', () => {
  const storage = { error: 'storage', message: 'the Library’s Storage did not answer' };
  const run = { run: 1, added: 0, findings: [], step: null };

  expect(() => sync({ ...run, status: 'stopped', stopped: null }, 'sync')).toThrow(
    /says nothing about what stopped it/,
  );
  expect(() => sync({ ...run, status: 'syncing', stopped: storage }, 'sync')).toThrow(
    /carries a refusal/,
  );
  expect(sync({ ...run, status: 'stopped', stopped: storage }, 'sync').stopped?.kind).toBe(
    'storage',
  );

  const kept = { run: 1, folder: 'albums', total: 3, done: 1, declined: [], findings: [] };
  expect(() => displacedFill({ ...kept, status: 'stopped', stopped: null }, 'kept')).toThrow(
    /says nothing about what stopped it/,
  );
  expect(() => displacedFill({ ...kept, status: 'done', stopped: null }, 'kept')).toThrow(
    /not a literal/,
  );

  expect(() => catalog({ state: 'behind', stopped: null }, 'catalog')).toThrow(
    /says nothing about what stopped it/,
  );
  expect(() => catalog({ state: 'caught_up', stopped: storage }, 'catalog')).toThrow(
    /carries a refusal/,
  );
});
