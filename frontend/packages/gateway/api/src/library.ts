import { apiUrl, askedForJson } from './request';

/**
 * Which Library the browser is looking at — `GET /api/library`.
 *
 * Three fields, and what is missing from them is the point: not the epoch, not
 * the checkpoint, not the folder id or the bucket name. The provider is here
 * because "on Google Drive" is something a person recognizes their own Library
 * by; it names the provider and nothing about the account.
 */
export interface Library {
  /** What this device calls the Library, which is what the status bar shows. */
  name: string;
  library_id: string;
  provider: string;
}

/** Asks which Library this is. */
export function getLibrary(signal?: AbortSignal): Promise<Library> {
  return askedForJson<Library>(apiUrl('library'), signal);
}
