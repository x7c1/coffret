/**
 * What the explorer reads a Library through.
 *
 * The six routes `coffret-server` answers, as typed calls: which Library this
 * is, every folder in it, what one folder holds, one Entry's plaintext, what the
 * server is bringing over on its own, and the one call that asks it to bring a
 * folder over again. The types are this package's word for the server's
 * serialization — written by hand, one file per route, so that a field the
 * server gains has one obvious place to land here — and every refusal arrives as
 * one shape a screen can branch on.
 *
 * Nothing above this package builds a URL or reads a status code. That is the
 * whole point of it: the app package knows what a Library holds, and this one
 * knows how to ask.
 */

export { getActivity, startFill } from './activity';
export type { Activity, DeclinedEntry, Fill, FillStatus, Refused } from './activity';
export { getFile } from './file';
export { getFolders } from './folders';
export type { Folders } from './folders';
export { getLibrary } from './library';
export type { Library } from './library';
export { getListing } from './list';
export type { ContainerKind, EntryState, Listing, ListedFile, ListedFolder } from './list';
export { isRefusal, Refusal } from './refusal';
export type { DeclinedReason, RefusalKind, SurfacedFinding } from './refusal';
