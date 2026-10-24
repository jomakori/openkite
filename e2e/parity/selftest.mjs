#!/usr/bin/env node
// Self-test for the G3 comparator: proves that compare.mjs actually fails on a
// single-class, single-token or single-property divergence, and that the
// allowlist is the only way to pass with a divergence — never a silent
// tolerance. Runs in CI before the real cross-host comparison, and locally
// without a browser or a desktop build.
//
//   node e2e/parity/selftest.mjs
//
// Exit 0 when every case behaves as specified, 1 otherwise.

import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const comparePath = join(here, 'compare.mjs');

const ROUTES = [
  ['/', 'home'],
  ['/cluster', 'cluster'],
  ['/workloads', 'workloads'],
  ['/config', 'config'],
];

/** A snapshot with the shape the extractor produces, for one route. */
function snapshot(route, overrides = {}) {
  const base = {
    schema: 1,
    route,
    route_marker: route,
    viewport: { width: 800, height: 600, dpr: 1, band: 'medium' },
    mounted: { app: true, view: true },
    tokens: {
      '--bg': '#0b0d10',
      '--panel': 'rgba(255, 255, 255, 0.04)',
      '--text': '#e6edf3',
      '--accent': '#6ee7ff',
    },
    chrome: { app: 1, topbar: 1, sidebar: 1, 'nav-item': 6, badge: 2 },
    view: { view: 1, 'page-head': 1, toolbar: 1, 'tag-row': 1, panel: 1 },
    nodes: {
      '.app': {
        present: true,
        count: 1,
        classes: ['app'],
        attrs: { 'data-surface': 'app' },
        style: { display: 'flex', 'background-color': 'rgb(11, 13, 16)', 'font-size': '13px' },
      },
      '[data-route]': {
        present: true,
        count: 1,
        classes: ['route'],
        attrs: { 'data-route': route },
        style: {},
      },
      '.tag': {
        present: true,
        count: 3,
        classes: ['tag'],
        attrs: {},
        style: { 'border-radius': '999px' },
      },
    },
  };
  return { ...base, ...overrides };
}

function writePair(root, mutate = {}) {
  const desktopDir = join(root, 'desktop');
  const webDir = join(root, 'web');
  mkdirSync(desktopDir, { recursive: true });
  mkdirSync(webDir, { recursive: true });
  for (const [route, slug] of ROUTES) {
    const desktop = snapshot(route, mutate.desktop?.(route) ?? {});
    const web = snapshot(route, mutate.web?.(route) ?? {});
    writeFileSync(join(desktopDir, `${slug}.desktop.json`), JSON.stringify(desktop, null, 2));
    writeFileSync(join(webDir, `${slug}.web.json`), JSON.stringify(web, null, 2));
  }
  return { desktopDir, webDir };
}

function run(desktopDir, webDir, allowlist = null) {
  const args = [comparePath, '--desktop', desktopDir, '--web', webDir];
  if (allowlist) args.push('--allowlist', allowlist);
  const result = spawnSync(process.execPath, args, { encoding: 'utf8' });
  return { status: result.status, stdout: result.stdout, stderr: result.stderr };
}

function writeAllowlist(root, entries) {
  const path = join(root, 'allowlist.json');
  writeFileSync(path, JSON.stringify({ version: 1, entries }, null, 2));
  return path;
}

const cases = [];
function check(name, fn) {
  cases.push({ name, fn });
}

check('identical hosts pass with no deltas', (root) => {
  const { desktopDir, webDir } = writePair(root);
  const { status, stdout } = run(desktopDir, webDir);
  if (status !== 0) throw new Error(`expected exit 0, got ${status}\n${stdout}`);
  if (!/0 delta\(s\)/.test(stdout)) throw new Error(`expected zero deltas, got:\n${stdout}`);
});

check('one extra class fails the gate and names the class', (root) => {
  const { desktopDir, webDir } = writePair(root, {
    web: (route) =>
      route === '/workloads'
        ? { nodes: { ...snapshot(route).nodes, '.tag': { ...snapshot(route).nodes['.tag'], classes: ['tag', 'tag-muted'] } } }
        : {},
  });
  const { status, stdout } = run(desktopDir, webDir);
  if (status !== 1) throw new Error(`expected exit 1, got ${status}\n${stdout}`);
  if (!stdout.includes('class "tag-muted"')) throw new Error(`delta not named:\n${stdout}`);
});

check('one differing token value fails the gate', (root) => {
  const { desktopDir, webDir } = writePair(root, {
    web: () => ({ tokens: { ...snapshot('/').tokens, '--accent': '#ff5c8a' } }),
  });
  const { status, stdout } = run(desktopDir, webDir);
  if (status !== 1) throw new Error(`expected exit 1, got ${status}\n${stdout}`);
  if (!stdout.includes('token --accent')) throw new Error(`delta not named:\n${stdout}`);
});

check('a token declared by one host only is a delta', (root) => {
  const { desktopDir, webDir } = writePair(root, {
    desktop: () => ({ tokens: { ...snapshot('/').tokens, '--terminal-bg': '#000' } }),
  });
  const { status, stdout } = run(desktopDir, webDir);
  if (status !== 1) throw new Error(`expected exit 1, got ${status}\n${stdout}`);
  if (!stdout.includes('token --terminal-bg')) throw new Error(`delta not named:\n${stdout}`);
});

check('one computed property difference fails the gate', (root) => {
  const { desktopDir, webDir } = writePair(root, {
    web: (route) => ({
      nodes: {
        ...snapshot(route).nodes,
        '.app': { ...snapshot(route).nodes['.app'], style: { display: 'flex', 'background-color': 'rgb(12, 14, 17)', 'font-size': '13px' } },
      },
    }),
  });
  const { status, stdout } = run(desktopDir, webDir);
  if (status !== 1) throw new Error(`expected exit 1, got ${status}\n${stdout}`);
  if (!stdout.includes('.app background-color')) throw new Error(`delta not named:\n${stdout}`);
});

check('a node missing on one host fails the gate', (root) => {
  const { desktopDir, webDir } = writePair(root, {
    desktop: (route) => ({
      nodes: { ...snapshot(route).nodes, '.tag': { present: false, count: 0 } },
    }),
  });
  const { status, stdout } = run(desktopDir, webDir);
  if (status !== 1) throw new Error(`expected exit 1, got ${status}\n${stdout}`);
  if (!stdout.includes('node .tag')) throw new Error(`delta not named:\n${stdout}`);
});

check('an allowlisted delta passes, and is printed', (root) => {
  const { desktopDir, webDir } = writePair(root, {
    web: (route) =>
      route === '/workloads'
        ? { nodes: { ...snapshot(route).nodes, '.tag': { ...snapshot(route).nodes['.tag'], classes: ['tag', 'tag-muted'] } } }
        : {},
  });
  const allowlist = writeAllowlist(root, [
    {
      route: '/workloads',
      kind: 'class',
      selector: '.tag',
      key: 'tag-muted',
      why: 'fixture capability list paints one extra tag on the browser host',
      reviewed_by: 'sisyphus-junior',
      ticket: 'OKT-192',
    },
  ]);
  const { status, stdout } = run(desktopDir, webDir, allowlist);
  if (status !== 0) throw new Error(`expected exit 0, got ${status}\n${stdout}`);
  if (!stdout.includes('allowlisted deltas (1)')) throw new Error(`allowlist not reported:\n${stdout}`);
});

check('an allowlist entry that no longer matches is a failure', (root) => {
  const { desktopDir, webDir } = writePair(root);
  const allowlist = writeAllowlist(root, [
    {
      route: '/config',
      kind: 'token',
      key: '--gone',
      why: 'this delta was fixed and the entry was left behind',
      reviewed_by: 'sisyphus-junior',
    },
  ]);
  const { status, stdout } = run(desktopDir, webDir, allowlist);
  if (status !== 1) throw new Error(`expected exit 1, got ${status}\n${stdout}`);
  if (!stdout.includes('Stale allowlist entries (1)')) throw new Error(`stale entry not reported:\n${stdout}`);
});

check('an allowlist entry without a reason or reviewer is rejected', (root) => {
  const { desktopDir, webDir } = writePair(root);
  const allowlist = writeAllowlist(root, [{ route: '/', kind: 'token', key: '--accent' }]);
  const { status, stdout, stderr } = run(desktopDir, webDir, allowlist);
  if (status !== 1) throw new Error(`expected exit 1, got ${status}\n${stdout}${stderr}`);
  if (!stderr.includes('allowlist itself is invalid')) throw new Error(`invalid allowlist not reported:\n${stderr}`);
});

check('a host that resolved the wrong route fails hard', (root) => {
  const { desktopDir, webDir } = writePair(root, {
    desktop: (route) => (route === '/config' ? { route_marker: '/' } : {}),
  });
  const { status, stderr } = run(desktopDir, webDir);
  if (status !== 1) throw new Error(`expected exit 1, got ${status}\n${stderr}`);
  if (!stderr.includes('resolved route')) throw new Error(`route mismatch not reported:\n${stderr}`);
});

check('a missing snapshot fails hard instead of being skipped', (root) => {
  const { desktopDir, webDir } = writePair(root);
  rmSync(join(webDir, 'cluster.web.json'));
  const { status, stderr } = run(desktopDir, webDir);
  if (status !== 1) throw new Error(`expected exit 1, got ${status}\n${stderr}`);
  if (!stderr.includes('missing web snapshot')) throw new Error(`missing snapshot not reported:\n${stderr}`);
});

check('hosts in different breakpoint bands are not compared', (root) => {
  const { desktopDir, webDir } = writePair(root, {
    web: () => ({ viewport: { width: 1280, height: 900, dpr: 1, band: 'wide' } }),
  });
  const { status, stderr } = run(desktopDir, webDir);
  if (status !== 1) throw new Error(`expected exit 1, got ${status}\n${stderr}`);
  if (!stderr.includes('viewport bands differ')) throw new Error(`band mismatch not reported:\n${stderr}`);
});

check('count differences are notices, not deltas', (root) => {
  const { desktopDir, webDir } = writePair(root, {
    desktop: (route) => ({
      nodes: { ...snapshot(route).nodes, '.tag': { ...snapshot(route).nodes['.tag'], count: 1 } },
    }),
  });
  const { status, stdout } = run(desktopDir, webDir);
  if (status !== 0) throw new Error(`expected exit 0 (counts are review material), got ${status}\n${stdout}`);
  if (!stdout.includes('count: .tag: desktop 1, web 3')) throw new Error(`count notice missing:\n${stdout}`);
});

let failures = 0;
for (const { name, fn } of cases) {
  const root = mkdtempSync(join(tmpdir(), 'parity-selftest-'));
  try {
    fn(root);
    console.log(`ok   ${name}`);
  } catch (error) {
    failures += 1;
    console.log(`FAIL ${name}\n     ${error.message.replace(/\n/g, '\n     ')}`);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

console.log(`\ncomparator self-test: ${cases.length - failures}/${cases.length} passed`);
process.exit(failures === 0 ? 0 : 1);
