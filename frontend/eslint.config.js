import js from '@eslint/js';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  {
    ignores: [
      '**/dist/**',
      '**/dist-types/**',
      // The desktop app's bundles (scripts/desktop.sh build).
      'packages/apps/desktop/release/**',
      '**/node_modules/**',
      // Gitignored scratch space (agent logs, repro scripts, etc.); never linted.
      '**/.tmp/**',
      '**/*.config.{js,ts}',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
);
