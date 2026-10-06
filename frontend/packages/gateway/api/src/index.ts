/**
 * What the explorer reads a Library through.
 *
 * The eleven routes `coffret-server` answers, as typed calls: which Library
 * this is, every folder in it, what one folder holds, one Entry's plaintext,
 * files added to a folder, what the Library has become since this device last
 * looked, what the server is doing on its own — which carries whether it still
 * holds the Library open — the three calls that ask it to take that work up
 * again, and the one that renews the grant Storage stopped taking. The types are this package's word for the server's
 * serialization — written by hand, one file per route, so that a field the
 * server gains has one obvious place to land here — and every refusal arrives as
 * one shape a screen can branch on.
 *
 * Nothing above this package builds a URL or reads a status code. That is the
 * whole point of it: the app package knows what a Library holds, and this one
 * knows how to ask.
 */

export { getWork, startFill, startFreeze, startSync } from './work';
export type {
  Work,
  Catalog,
  CatalogState,
  DeclinedEntry,
  DisplacedFill,
  DisplacedFreeze,
  Fill,
  FillStatus,
  Finding,
  FindingReason,
  Freeze,
  FreezeStatus,
  LibraryState,
  NotStopped,
  Phase,
  Reconnect,
  ReconnectState,
  Step,
  Stopped,
  Sync,
  SyncStatus,
} from './work';
export { getFile } from './file';
export { getFolders } from './folders';
export type { Folders } from './folders';
export { getLibrary } from './library';
export type { Library } from './library';
export { getListing } from './list';
export type { ContainerKind, EntryState, Listing, ListedFile, ListedFolder } from './list';
export { startReconnect } from './reconnect';
export type { Reconnecting } from './reconnect';
export { refreshCatalog } from './refresh';
export type { Refreshed } from './refresh';
export { isRefusal, NO_FOLDER_HERE, Refusal } from './refusal';
export type { PlacementReason, Refused, RefusalKind, SurfacedFinding } from './refusal';
export { addFiles } from './upload';
export type { Added, Adding, RefusedPart, Upload } from './upload';
