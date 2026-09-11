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

// OKT-67 spike build: bundle the React 19 + Tailwind 4 UI as a fixed-name
// IIFE + CSS pair, vendored next to the xterm bundle so the Rust host can
// `include_str!` it with no manifest/hash parsing (same convention as
// tools/build-xterm).
//
// `define` is load-bearing: Vite's library mode does not replace
// `process.env.NODE_ENV` on its own, so without it React ships BOTH the
// development and production builds (and the dev branch dereferences an
// undefined `process` at runtime in the webview). Pinning it to "production"
// statically drops the dev paths and roughly quarters the bundle.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  define: {
    'process.env.NODE_ENV': JSON.stringify('production'),
  },
  build: {
    outDir: vendoredBundleDir,
    emptyOutDir: true,
    target: 'es2022',
    // The Rust host loads a stable URL; hashed filenames would need a
    // manifest read, which the spike intentionally avoids.
    lib: {
      entry: resolve(here, 'src/mount.tsx'),
      name: 'OpenKiteReactSpike',
      formats: ['iife'],
      fileName: () => 'app.js',
      cssFileName: 'app',
    },
  },
})
