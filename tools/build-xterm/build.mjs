// Build the vendored xterm.js IIFE bundle for OpenKite.
// Produces a minified IIFE for `include_str!` in
// `src/components/terminal.rs`. Run from the tools/build-xterm dir.
//
//   node build.mjs ../../assets/vendored/xterm
//   # Then append the wrapper trailer (window.openkite._term_*) — see
//   # build.mjs comment block.

import { build } from 'esbuild';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const outDir = process.argv[2];
if (!outDir) {
    console.error('usage: node build.mjs <out-dir>');
    process.exit(1);
}

mkdirSync(outDir, { recursive: true });

await build({
    entryPoints: [resolve(here, 'node_modules/@xterm/xterm/lib/xterm.js')],
    bundle: true,
    format: 'iife',
    target: 'es2020',
    minify: true,
    legalComments: 'none',
    outfile: join(outDir, 'xterm.js'),
    globalName: 'OpenKiteXterm',
});

const cssSrc = resolve(here, 'node_modules/@xterm/xterm/css/xterm.css');
copyFileSync(cssSrc, join(outDir, 'xterm.css'));

const xtermPkg = JSON.parse(
    readFileSync(resolve(here, 'node_modules/@xterm/xterm/package.json'), 'utf8')
);
const esbuildPkg = JSON.parse(
    readFileSync(resolve(here, 'node_modules/esbuild/package.json'), 'utf8')
);
const sourceTxt = [
    'OpenKite vendored xterm.js bundle.',
    '',
    'Packages:',
    `  @xterm/xterm@${xtermPkg.version}`,
    `  esbuild@${esbuildPkg.version} IIFE minified, es2020 target.`,
    '',
    'Rebuild:',
    '  cd tools/build-xterm && npm install && node build.mjs ../../assets/vendored/xterm',
    '',
    'Cache-buster id: xterm-bundle-v1 (bump on each rebuild; see',
    'crate::components::terminal::xterm_host_path).',
    '',
].join('\n');
writeFileSync(join(outDir, 'SOURCE.txt'), sourceTxt);
