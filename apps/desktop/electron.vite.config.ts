import { resolve } from 'node:path';

import react from '@vitejs/plugin-react';
import { defineConfig } from 'electron-vite';

const shared = resolve(import.meta.dirname, 'src/shared');

export default defineConfig({
  main: {
    resolve: { alias: { '@shared': shared } },
    build: {
      rolldownOptions: {
        input: {
          index: resolve(import.meta.dirname, 'src/main/index.ts'),
          'engine-host': resolve(import.meta.dirname, 'src/engine-host/index.ts'),
        },
      },
    },
  },
  preload: {
    resolve: { alias: { '@shared': shared } },
    build: {
      rolldownOptions: {
        input: { index: resolve(import.meta.dirname, 'src/preload/index.ts') },
        // Sandboxed preload scripts must be CommonJS.
        output: { format: 'cjs', entryFileNames: '[name].cjs' },
      },
    },
  },
  renderer: {
    resolve: { alias: { '@shared': shared } },
    plugins: [react()],
    build: {
      minify: true,
      rolldownOptions: {
        input: { index: resolve(import.meta.dirname, 'src/renderer/index.html') },
      },
    },
  },
});
