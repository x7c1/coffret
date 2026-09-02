import { apiUrl, askedForJson } from './request';

/**
 * What a lock came to.
 *
 * One field, and it is always `true`. That is the guarantee rather than a
 * placeholder: the server is locked, and it was locked before this answer was
 * written — a lock that had not taken effect would not have answered.
 */
export interface Locked {
  locked: boolean;
}

/**
 * Locks the server — `POST /api/lock`.
 *
 * The Passphrase was spent once, when the server was started, and what it
 * produced lives until this ends it. Until then a machine somebody walked away
 * from is one that opens the whole Library to whoever sits down at it, which is
 * what this is for.
 *
 * Everything that needs the Master Key is refused from the moment it answers,
 * with the sentence saying the Passphrase is required; the Library's name and
 * what the server was doing go on being answered, because neither of them is
 * anything the Master Key keeps. Work already running finishes what it began.
 *
 * There is deliberately no call that undoes it. The Passphrase is typed at a
 * terminal, so what unlocks the server is starting it again — a route that took
 * one would carry the Passphrase through the browser, which this product does
 * not do.
 */
export function lockServer(signal?: AbortSignal): Promise<Locked> {
  return askedForJson<Locked>(apiUrl('lock'), signal, 'POST');
}
