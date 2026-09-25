// The explorer's half of the contract with the server.
//
// The three files under `contract/` are what `coffret-server` sends, written by
// its own cases: every refusal a route can answer with, every state the
// activity answer can be in, and one of every other answer a route gives, taken
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
  Activity,
  Catalog,
  CatalogState,
  DeclinedEntry,
  Fill,
  FillStatus,
  Finding,
  FindingReason,
  Freeze,
  FreezeStatus,
  LibraryState,
  Phase,
  Refused,
  Step,
  Sync,
  SyncStatus,
} from './activity';
import activityAnswers from './contract/activity.json';
import answers from './contract/answers.json';
import refusals from './contract/refusals.json';
import type { Folders } from './folders';
import type { Library } from './library';
import type { ContainerKind, EntryState, ListedFile, ListedFolder, Listing } from './list';
import type { Locked } from './lock';
import type { Refreshed } from './refresh';
import type { DeclinedReason, RefusalKind, SurfacedFinding } from './refusal';
import { refusalOf } from './refusal';
import type { RefusedPart, Upload } from './upload';

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
  epoch: 'server',
  locked: 'server',
  storage: 'server',
  unverified: 'server',
  server: 'server',
  unreachable: 'client',
  unrecognized: 'client',
};

const DECLINED_REASONS: Literals<DeclinedReason> = {
  unmapped: true,
  unmaterializable: true,
  reserved: true,
  refused_root: true,
  surfaced: true,
  locked: true,
  pack_resident: true,
};

/**
 * Which finding names a refusal carries, as against the ones only a run's
 * finding does — nothing a fetch does is declined over a file that changed
 * inside a Pack or one this device no longer has.
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
};

const FINDING_REASONS: Literals<FindingReason> = {
  surfaced: true,
  locked: true,
  refused_root: true,
  root_missing: true,
  root_on_another_filesystem: true,
};
const FILL_STATUSES: Literals<FillStatus> = {
  filling: true,
  done: true,
  stopped: true,
  superseded: true,
};
const SYNC_STATUSES: Literals<SyncStatus> = { syncing: true, done: true, stopped: true };
const FREEZE_STATUSES: Literals<FreezeStatus> = { freezing: true, done: true, stopped: true };
const PHASES: Literals<Phase> = {
  catching_up: true,
  reconciling: true,
  scanning: true,
  packing: true,
  uploading: true,
  fetching: true,
};
const CATALOG_STATES: Literals<CatalogState> = {
  catching_up: true,
  caught_up: true,
  behind: true,
};
const LIBRARY_STATES: Literals<LibraryState> = { locked: true, unlocked: true };
const ENTRY_STATES: Literals<EntryState> = { present: true, remote: true, uploading: true };
const CONTAINER_KINDS: Literals<ContainerKind> = { 'one-file': true, pack: true };

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

function refused(value: unknown, where: string): Refused {
  const fields = object(value, where, ['error', 'message'], ['reason', 'surfaced']);
  return {
    error: one(KINDS, fields.error, `${where}.error`),
    message: string(fields.message, `${where}.message`),
    ...(fields.reason === undefined
      ? {}
      : { reason: one(DECLINED_REASONS, fields.reason, `${where}.reason`) }),
    ...(fields.surfaced === undefined
      ? {}
      : { surfaced: one(SURFACED, fields.surfaced, `${where}.surfaced`) }),
  };
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
    ...(fields.surfaced === undefined
      ? {}
      : { surfaced: one(SURFACED, fields.surfaced, `${where}.surfaced`) }),
  };
}

function step(value: unknown, where: string): Step {
  const fields = object(value, where, ['phase', 'done', 'total']);
  return {
    phase: one(PHASES, fields.phase, `${where}.phase`),
    done: number(fields.done, `${where}.done`),
    total: nullable(fields.total, (total) => number(total, `${where}.total`)),
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
    'waiting',
    'dropped',
    'displaced',
    'stopped',
  ]);
  return {
    run: number(fields.run, `${where}.run`),
    folder: string(fields.folder, `${where}.folder`),
    status: one(FILL_STATUSES, fields.status, `${where}.status`),
    total: number(fields.total, `${where}.total`),
    done: number(fields.done, `${where}.done`),
    declined: list(fields.declined, `${where}.declined`, declinedEntry),
    waiting: folders(fields.waiting, `${where}.waiting`),
    dropped: folders(fields.dropped, `${where}.dropped`),
    displaced: list(fields.displaced, `${where}.displaced`, fill),
    stopped: nullable(fields.stopped, (stopped) => refused(stopped, `${where}.stopped`)),
  };
}

function sync(value: unknown, where: string): Sync {
  const fields = object(value, where, ['run', 'status', 'added', 'findings', 'step', 'stopped']);
  return {
    run: number(fields.run, `${where}.run`),
    status: one(SYNC_STATUSES, fields.status, `${where}.status`),
    added: number(fields.added, `${where}.added`),
    findings: list(fields.findings, `${where}.findings`, finding),
    step: nullable(fields.step, (value) => step(value, `${where}.step`)),
    stopped: nullable(fields.stopped, (stopped) => refused(stopped, `${where}.stopped`)),
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
    'dropped',
    'displaced',
    'stopped',
  ]);
  return {
    run: number(fields.run, `${where}.run`),
    folder: string(fields.folder, `${where}.folder`),
    status: one(FREEZE_STATUSES, fields.status, `${where}.status`),
    packs: number(fields.packs, `${where}.packs`),
    entries: number(fields.entries, `${where}.entries`),
    findings: list(fields.findings, `${where}.findings`, finding),
    step: nullable(fields.step, (value) => step(value, `${where}.step`)),
    waiting: folders(fields.waiting, `${where}.waiting`),
    dropped: folders(fields.dropped, `${where}.dropped`),
    displaced: list(fields.displaced, `${where}.displaced`, freeze),
    stopped: nullable(fields.stopped, (stopped) => refused(stopped, `${where}.stopped`)),
  };
}

function catalog(value: unknown, where: string): Catalog {
  const fields = object(value, where, ['state', 'trouble']);
  return {
    state: one(CATALOG_STATES, fields.state, `${where}.state`),
    trouble: nullable(fields.trouble, (trouble) => refused(trouble, `${where}.trouble`)),
  };
}

function activity(value: unknown, where: string): Activity {
  const fields = object(value, where, ['server', 'library', 'catalog', 'fill', 'sync', 'freeze']);
  return {
    server: string(fields.server, `${where}.server`),
    library: one(LIBRARY_STATES, fields.library, `${where}.library`),
    catalog: catalog(fields.catalog, `${where}.catalog`),
    fill: nullable(fields.fill, (value) => fill(value, `${where}.fill`)),
    sync: nullable(fields.sync, (value) => sync(value, `${where}.sync`)),
    freeze: nullable(fields.freeze, (value) => freeze(value, `${where}.freeze`)),
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
  const fields = object(value, where, ['path', 'mapped', 'held', 'folders', 'files']);
  return {
    path: string(fields.path, `${where}.path`),
    mapped: boolean(fields.mapped, `${where}.mapped`),
    held: boolean(fields.held, `${where}.held`),
    folders: list(fields.folders, `${where}.folders`, listedFolder),
    files: list(fields.files, `${where}.files`, listedFile),
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
      body.reason === undefined ? null : one(DECLINED_REASONS, body.reason, `${where}.reason`),
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
  expect([...reasons].sort()).toEqual(Object.keys(DECLINED_REASONS).sort());
  const refusalFindings = Object.entries(SURFACED)
    .filter(([, carried]) => carried === 'refusal')
    .map(([name]) => name);
  expect([...findings].sort()).toEqual(refusalFindings.sort());
});

// Every state the activity answer can be in narrows to `Activity`, and between
// them the answers exercise every literal of every union inside it: a status,
// a phase or a standing this client has a word for and the server never sends
// is as much a disagreement as the other way round.
it('reads every activity answer the server sends through the Activity type', () => {
  const read = activityAnswers.map((answer, index) => activity(answer, `activity[${index}]`));

  expect(read.length).toBeGreaterThan(0);
  for (const table of [
    FILL_STATUSES,
    SYNC_STATUSES,
    FREEZE_STATUSES,
    PHASES,
    CATALOG_STATES,
    LIBRARY_STATES,
    FINDING_REASONS,
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
  const locked: Locked = {
    locked: boolean(object(answers.locked, 'locked', ['locked']).locked, 'locked.locked'),
  };
  const listings = Object.fromEntries(
    Object.entries(answers.listings).map(([name, value]) => [name, listing(value, name)]),
  );
  const uploads = {
    written: upload(answers.uploads.written, 'uploads.written'),
    refused: upload(answers.uploads.refused, 'uploads.refused'),
  };

  expect(library.provider).toBe('s3');
  expect(listed.folders.length).toBeGreaterThan(0);
  expect(refreshed.entries).toBeGreaterThan(0);
  expect(locked.locked).toBe(true);
  expect(uploads.written.written.length).toBeGreaterThan(0);
  expect(uploads.refused.refused.length).toBeGreaterThan(0);

  const root = listings.unmapped_root;
  expect(root.path).toBe('');
  expect(root.mapped).toBe(false);
  expect(root.folders.map((folder) => folder.mapped)).toContain(true);
  expect(listings.root.mapped).toBe(true);
  expect(listings.nowhere.held).toBe(false);

  expect(unmet(ENTRY_STATES), 'row states no listing sent').toEqual([]);
  expect(unmet(CONTAINER_KINDS), 'Container kinds no listing sent').toEqual([]);
  expect(
    Object.values(listings).some((shown) => shown.files.some((file) => file.container === null)),
    'a row with no Container of its own yet',
  ).toBe(true);
});
