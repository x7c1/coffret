import { expect, it, vi } from 'vitest';

import { denyEveryPermission, type PermissionGate } from './permissions.js';

it('denies every permission request, check and device', () => {
  let request: ((...args: unknown[]) => void) | null = null;
  let check: ((...args: unknown[]) => boolean) | null = null;
  let device: ((...args: unknown[]) => boolean) | null = null;
  const gate = {
    setPermissionRequestHandler: (handler: typeof request) => {
      request = handler;
    },
    setPermissionCheckHandler: (handler: typeof check) => {
      check = handler;
    },
    setDevicePermissionHandler: (handler: typeof device) => {
      device = handler;
    },
  } as unknown as PermissionGate;

  denyEveryPermission(gate);

  for (const permission of ['media', 'notifications', 'clipboard-read', 'geolocation', 'fullscreen']) {
    const callback = vi.fn();
    request!({}, permission, callback, {});
    expect(callback).toHaveBeenCalledWith(false);
    expect(check!({}, permission, 'http://127.0.0.1:1', {})).toBe(false);
  }
  expect(device!({ deviceType: 'usb' })).toBe(false);
});
