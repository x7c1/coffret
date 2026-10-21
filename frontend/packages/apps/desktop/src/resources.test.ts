import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { resourcesFor } from './resources.js';

/** This package's `dist/`, where the app's code is built in a checkout. */
const BUILT = fileURLToPath(new URL('../dist', import.meta.url));

describe('the resources of a packaged app', () => {
  it('are under the resources directory, outside the asar archive', () => {
    const resources = resourcesFor(true, '/opt/Coffret/resources', '/opt/Coffret/resources/app.asar/dist');
    expect(resources.addon).toBe(join('/opt/Coffret/resources', 'coffret.node'));
    expect(resources.assets).toBe(join('/opt/Coffret/resources', 'assets'));
    expect(resources.missingAddon).toMatch(/install Coffret again/);
  });
});

describe('the resources of an app run from the checkout', () => {
  const resources = resourcesFor(false, '/unused', BUILT);

  it('take the addon from where make napi-addon leaves it', () => {
    const expected = fileURLToPath(new URL('../../../../../backend/target/release/coffret.node', import.meta.url));
    expect(resources.addon).toBe(expected);
    expect(resources.missingAddon).toMatch(/make napi-addon/);
  });

  it("take the icons from the package's assets", () => {
    expect(resources.assets).toBe(fileURLToPath(new URL('../assets', import.meta.url)));
    expect(existsSync(join(resources.assets, 'tray.png'))).toBe(true);
  });
});
