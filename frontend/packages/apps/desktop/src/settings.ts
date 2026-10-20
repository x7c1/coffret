// What the app reads from its environment before it starts the addon.

/** The installed app's identifier. */
export const IDENTIFIER = 'io.github.x7c1.coffret';

/**
 * The development app's: the one run from a checkout, which serves the
 * Libraries under the development state directory and runs beside the
 * installed app.
 */
export const DEV_IDENTIFIER = 'io.github.x7c1.coffret.dev';

/** The variable the server reads its idle interval from, which the app reads too. */
export const IDLE_MINUTES = 'COFFRET_IDLE_MINUTES';

/** The variable that moves where the Libraries are. */
export const STATE_DIR = 'COFFRET_STATE_DIR';

/** The interval when nothing sets one, the server's own default. */
export const DEFAULT_IDLE_MINUTES = 30n;

/** The largest value the addon's own reading takes. */
const MAX_MINUTES = 2n ** 64n - 1n;

/**
 * Which app this is, from whether it runs from a package or from a checkout.
 *
 * A run from a checkout is the development app, and is refused where
 * `COFFRET_STATE_DIR` is not set: without it the addon would serve the
 * Libraries under the default state directory, which is the production
 * Library's, from a build of whatever branch the checkout is on.
 */
export function identifierFor(packaged: boolean, env: Record<string, string | undefined>): string {
  if (packaged) {
    return IDENTIFIER;
  }
  if (!env[STATE_DIR]) {
    throw new Error(
      `${STATE_DIR} is not set; run the development app through make desktop-electron-dev, ` +
        'which sets it to the development state directory.',
    );
  }
  return DEV_IDENTIFIER;
}

/**
 * The idle interval in minutes, from what `COFFRET_IDLE_MINUTES` was set to:
 * `undefined`, or the empty string, where it was not set.
 *
 * The rule is the addon's own, read here as well so that the app says it in
 * the same words: refused rather than defaulted where it is set and is not a
 * whole number of at least one, since somebody who set it meant something by
 * it. A bigint, since the addon takes any whole number a 64-bit unsigned
 * integer holds, and the app hands it on as written here.
 */
export function idleMinutes(setting: string | undefined): bigint {
  if (setting === undefined || setting === '') {
    return DEFAULT_IDLE_MINUTES;
  }
  if (!/^\+?[0-9]+$/.test(setting) || BigInt(setting) > MAX_MINUTES) {
    throw new Error(`${IDLE_MINUTES} is not a whole number of minutes: ${JSON.stringify(setting)}`);
  }
  const minutes = BigInt(setting);
  if (minutes < 1n) {
    throw new Error(`${IDLE_MINUTES} must be at least 1, not ${minutes}`);
  }
  return minutes;
}
