// How the desktop app is packaged: electron-builder's configuration, run by
// `scripts/desktop.sh build` (`make desktop-build`) and by the Bundle workflow
// through it. It makes one bundle per host:
//
//   macOS (arm64)   Coffret_<version>_aarch64.dmg, holding Coffret.app
//   Linux (amd64)   coffret-desktop_<version>_amd64.deb, the package
//                   `coffret-desktop`
//
// The names, and the .deb's package name, are the ones the desktop bundles
// have always had, so that the release guide's names hold and `apt` upgrades
// an installed `coffret-desktop` in place.
//
// The version is the workspace version of backend/Cargo.toml, which
// scripts/desktop.sh reads through `cargo metadata` and hands over as
// COFFRET_BUNDLE_VERSION; package.json's own is a placeholder nothing reads.
// Without it this refuses to package, rather than ship the placeholder.
//
// The app's code — dist/ and package.json — goes into the asar archive. The
// Node-API addon (coffret.node, built by `make napi-addon` with the explorer
// compiled in) and the icons (assets/) ship beside it as files of the
// resources directory, where src/resources.ts finds them: Node loads an addon
// from a file on disk.

/** The variable scripts/desktop.sh hands the workspace version over in. */
const VERSION_VARIABLE = 'COFFRET_BUNDLE_VERSION';

function bundleVersion() {
  const version = process.env[VERSION_VARIABLE];
  if (!version) {
    throw new Error(`${VERSION_VARIABLE} is not set; package the app through scripts/desktop.sh build (make desktop-build)`);
  }
  return version;
}

/** @type {import('electron-builder').Configuration} */
const config = {
  appId: 'io.github.x7c1.coffret',
  productName: 'Coffret',
  // The bundles are written here, apart from dist/, which is the compiled code.
  directories: { output: 'release', buildResources: 'packaging' },
  // The .deb's Version, Homepage and Description are taken from these.
  // desktopName is what Electron takes the app ID of its windows on Linux from
  // (the Wayland app ID and the X11 window class) before the app sets its own
  // (src/identity.ts), the installed identifier either way.
  extraMetadata: {
    version: bundleVersion(),
    description: 'A personal E2EE file library: the desktop app, which opens a Library and shows its explorer',
    homepage: 'https://github.com/x7c1/coffret',
    desktopName: 'io.github.x7c1.coffret.desktop',
  },
  files: ['dist/**/*', '!dist/**/*.map', 'package.json'],
  extraResources: [
    { from: '../../../../backend/target/release/coffret.node', to: 'coffret.node' },
    { from: 'assets', to: 'assets' },
  ],
  asar: true,
  // Nothing is fetched or rebuilt for the package: the app has no runtime
  // dependencies, and the addon is built by cargo, not node-gyp.
  npmRebuild: false,
  nodeGypRebuild: false,
  publish: null,

  // The switches compiled into the Electron binary, flipped before it is
  // signed. Off: running the binary as plain Node (ELECTRON_RUN_AS_NODE),
  // NODE_OPTIONS and NODE_EXTRA_CA_CERTS, and the --inspect family of flags
  // (and SIGUSR1's inspector), so that nothing outside the app can make the
  // installed binary run other code with its permissions, or attach a
  // debugger to the process holding an unlocked Library. On: loading the app
  // from its asar archive alone, never from a loose `app/` directory beside
  // it; the addon is a resource the app loads by path, not app code, so it is
  // unaffected.
  electronFuses: {
    runAsNode: false,
    enableNodeOptionsEnvironmentVariable: false,
    enableNodeCliInspectArguments: false,
    onlyLoadAppFromAsar: true,
  },

  mac: {
    target: [{ target: 'dmg', arch: ['arm64'] }],
    category: 'public.app-category.utilities',
    // The Icon Composer icon, compiled with Xcode's actool (Xcode 26 or later)
    // into Assets.car for macOS 26 and later; the .icns electron-builder also
    // bundles beside it is the icon before macOS 26 (scripts/gen-icons.sh).
    icon: 'packaging/icon.icon',
    // Signed ad hoc, with no Developer ID, and not notarized: the hardened
    // runtime on, with packaging/entitlements.mac.plist, for the app and every
    // helper. allow-jit is what V8 needs to compile JavaScript under it.
    // disable-library-validation is there because an ad hoc signature carries
    // no Team ID: under the hardened runtime's library validation the loader
    // refuses every library whose Team ID is not the executable's, and with
    // none on either side that is Electron's own framework, before the app
    // runs at all, and the addon after it. The addon is signed ad hoc with the
    // rest of the bundle.
    identity: '-',
    hardenedRuntime: true,
    gatekeeperAssess: false,
    notarize: false,
    entitlements: 'packaging/entitlements.mac.plist',
    entitlementsInherit: 'packaging/entitlements.mac.plist',
  },
  dmg: {
    artifactName: 'Coffret_${version}_aarch64.${ext}',
  },

  linux: {
    target: [{ target: 'deb', arch: ['x64'] }],
    executableName: 'coffret-desktop',
    icon: 'packaging/icon.png',
    category: 'Utility',
    synopsis: 'Coffret',
    maintainer: 'x7c1',
    desktop: {
      entry: {
        Name: 'Coffret',
        // The window's app ID is the installed identifier (src/identity.ts),
        // which is what GNOME matches here, so that it shows the windows as
        // this entry's.
        StartupWMClass: 'io.github.x7c1.coffret',
      },
    },
  },
  deb: {
    packageName: 'coffret-desktop',
    // electron-builder's default Depends (GTK 3, NSS, libnotify and the rest
    // Electron links), and no Recommends: its default, libappindicator3-1, is
    // not in Ubuntu 24.04, and Electron's tray icon speaks
    // StatusNotifierItem over D-Bus by itself.
    recommends: [],
    artifactName: 'coffret-desktop_${version}_amd64.${ext}',
  },
};

export default config;
