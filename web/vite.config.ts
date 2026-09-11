import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import pkg from './package.json'

const here = dirname(fileURLToPath(import.meta.url))

// The vendored output directory lives in one place — the
// `openkite.vendoredBundleDir` field in web/package.json — so the build, the
// bundle-size report, and the CI freshness gate all follow a single rename.
const vendoredBundleDir = resolve(here, pkg.openkite.vendoredBundleDir)

// One source tree, two output targets, selected by `--mode`:
//
// - default (`npm run build`): the DESKTOP target. Library mode bundles the
//   React UI as a fixed-name IIFE + CSS pair, vendored next to the xterm
//   bundle so the Rust host can `include_str!` it with no manifest/hash
//   parsing (same convention as tools/build-xterm). `npm run check:bundle`
//   gates this output's freshness in CI.
//
// - `--mode web` (`npm run build:web`, run by web/build.sh): the BROWSER
//   target. A standard Vite app build (index.html + hashed assets) into
//   web/dist/, served statically by the PR-preview image and by
//   `python3 -m http.server`. The difference is output shape only — the
//   source, plugins, and define block are shared, so the two targets render
//   the same UI.
//
// `define` is load-bearing for both: Vite's library mode does not replace
// `process.env.NODE_ENV` on its own, so without it React ships BOTH the
// development and production builds (and the dev branch dereferences an
// undefined `process` at runtime in the webview). Pinning it to "production"
// statically drops the dev paths and roughly quarters the bundle.
export default defineConfig(({ mode }) => {
  const shared = {
    plugins: [react(), tailwindcss()],
    define: {
      'process.env.NODE_ENV': JSON.stringify('production'),
    },
  }

  if (mode === 'web') {
    return {
      ...shared,
      build: {
        outDir: resolve(here, 'dist'),
        emptyOutDir: true,
        target: 'es2022',
      },
    }
  }

  return {
    ...shared,
    build: {
      outDir: vendoredBundleDir,
      emptyOutDir: true,
      target: 'es2022',
      // The Rust host loads a stable URL; hashed filenames would need a
      // manifest read, which the desktop target intentionally avoids.
      lib: {
        entry: resolve(here, 'src/mount.tsx'),
        name: 'OpenKiteReactSpike',
        formats: ['iife'],
        fileName: () => 'app.js',
        cssFileName: 'app',
      },
    },
  }
})
