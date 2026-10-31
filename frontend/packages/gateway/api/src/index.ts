/**
 * What the explorer reads a Library through.
 *
 * The twenty-three routes `coffret-server` answers, as typed calls: which Library
 * this is, every folder in it, what one folder holds, which files stand either
 * side of one row in reading order, one Entry's plaintext,
 * files added to a folder, what the Library has become since this device last
 * looked and whether it has anything newer than that, what the server is doing
 * on its own — which carries whether it still holds the Library open — the
 * three calls that ask it to take that work up
 * again, the freeze among them also packing a folder somebody chose, the
 * question of what that freeze would pack, the deletion of files or a folder
 * and the question of what it would take out, the move of files and folders
 * and the question of what it would do to this device's files, the one that renews the grant
 * Storage stopped taking, the one that asks for a locked Library to be
 * unlocked, the two that choose a folder on this device and map part of
 * the Library to it, and the two that read and keep which set of colours this
 * device's explorer is drawn in. The types are this package's word
 * for the server's serialization — written by hand, one file per route, so that
 * a field the server gains has one obvious place to land here — and every
 * refusal arrives as one shape a screen can branch on.
 *
 * Beside them, the explorer host's own three under `/auth`: exchanging a launch
 * ticket for a browser token, asking whether a session still stands, and
 * ending it. Every request carries that token, which this package holds for
 * the page (`credential`), and goes to the page's own origin and nowhere else.
 *
 * Nothing above this package builds a URL or reads a status code. That is the
 * whole point of it: the app package knows what a Library holds, and this one
 * knows how to ask.
 */

export {
  getWork,
  previewDelete,
  previewFreeze,
  startDelete,
  startFill,
  startFreeze,
  startSync,
  stopWork,
} from './work';
export type {
  Work,
  AddUnderWay,
  Logged,
  LoggedKind,
  Move,
  MovePair,
  MoveStatus,
  LoggedWork,
  QueueFlow,
  ByteCount,
  Catalog,
  CatalogState,
  DeclinedEntry,
  Delete,
  DeletePreview,
  DeleteStatus,
  DeleteTarget,
  DisplacedFill,
  DisplacedFreeze,
  Fill,
  FillStatus,
  Finding,
  FindingReason,
  Freeze,
  FreezePreview,
  FreezeStatus,
  KeptParcels,
  LibraryState,
  NotStopped,
  PackRefusalReason,
  Phase,
  Reconnect,
  ReconnectState,
  RefusedPack,
  Step,
  Stopped,
  StopOffer,
  StoppedAdd,
  StopTarget,
  Sync,
  SyncStatus,
  TurnTaker,
} from './work';
export { getAppearance, setAppearance } from './appearance';
export type { Appearance } from './appearance';
export { checkSession, launch } from './auth';
export type { Launched, SessionStanding } from './auth';
export {
  dropToken,
  heldToken,
  holdToken,
  onSessionEnding,
  withdrawAnswers,
} from './credential';
export type { Ending, EndingListener } from './credential';
export { browseFolders } from './browse';
export type { Browsed, BrowsedFolder } from './browse';
export { getBeside } from './beside';
export type { Beside } from './beside';
export { getFile, peekFile, readAhead } from './file';
export { getFolders } from './folders';
export type { Folders } from './folders';
export { getLibrary } from './library';
export type { Library } from './library';
export { getListing } from './list';
export type { ContainerKind, EntryState, Listing, ListedFile, ListedFolder } from './list';
export { mapFolder } from './map';
export { moveEntries, previewMove } from './move';
export type { MovePreview } from './move';
export { componentDefect, reservedName } from './names';
export type { ComponentDefect, ReservedName } from './names';
export type { Mapped, MappedMarker } from './map';
export { startReconnect } from './reconnect';
export type { Reconnecting } from './reconnect';
export { refreshCatalog } from './refresh';
export type { Refreshed } from './refresh';
export { getRemoteHead } from './remoteHead';
export type { HeadState, RemoteHead } from './remoteHead';
export { isLockedRefusal, isRefusal, NO_FOLDER_HERE, Refusal } from './refusal';
export { ForeignUrl } from './request';
export type { Progress } from './request';
export type { PlacementReason, Refused, RefusalKind, SurfacedFinding } from './refusal';
export { askToUnlock } from './unlock';
export type { Unlocking } from './unlock';
export { addedFile, addFiles, isOverBudget, OverBudget, overdrawnBy } from './upload';
export type { Added, AddedSource, Adding, Overdrawn, RefusedPart, Upload } from './upload';
