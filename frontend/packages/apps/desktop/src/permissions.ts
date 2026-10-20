// No page of the app is granted anything a browser asks a person about.

import type { Session } from 'electron';

/** The part of a session that answers a page's permission requests. */
export type PermissionGate = Pick<
  Session,
  'setPermissionRequestHandler' | 'setPermissionCheckHandler' | 'setDevicePermissionHandler'
>;

/**
 * Denies every permission request and check in `session`: the camera, the
 * microphone, notifications, the clipboard's reading, devices and the rest.
 * Neither the Passphrase page nor the explorer asks for any, so one asked for
 * is answered no without a prompt.
 */
export function denyEveryPermission(session: PermissionGate): void {
  session.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
  session.setPermissionCheckHandler(() => false);
  session.setDevicePermissionHandler(() => false);
}
