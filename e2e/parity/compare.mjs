#!/usr/bin/env node
// G3 cross-host parity gate: grade the desktop and browser hosts against each
// other on *computed classes and token values* per core route. Pixels are not
// part of this gate — the per-host visual baselines (G2) own those, and they
// legitimately differ in data.
//
//   node e2e/parity/compare.mjs --desktop <dir> --web <dir> \
//        [--allowlist e2e/parity/allowlist.json] [--report <file.json>]
//
// Exit codes: 0 = parity holds (every delta is allowlisted), 1 = unexplained
// delta / stale allowlist entry / invalid allowlist, 2 = usage or I/O error.
//
// What is graded, per route:
//   * resolved token values on the document element (every custom property the
//     stylesheet declares, plus any the host sets inline);
//   * for each contract node: presence, the class chain, the contract `data-*`
//     attributes, and the computed values of the properties named in
//     `crates/openkite-ui/assets/parity-snapshot.js`.
// What is reported but not graded: class-token inventories and match counts.
// The hosts run on different data (the desktop boots disconnected, the browser
// host serves fixtures), so inventory counts are review material, not parity.

import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const ROUTES = [
  ['/', 'home'],
  ['/cluster', 'cluster'],
  ['/workloads', 'workloads'],
  ['/config', 'config'],
];

const SCHEMA = 1;
const DELTA_KINDS = new Set(['token', 'class', 'attr', 'style', 'node']);
const REQUIRED_FIELDS = {
  token: ['key'],
  class: ['selector', 'key'],
  attr: ['selector', 'key'],
  style: ['selector', 'property'],
  node: ['selector'],
};

function parseArgs(argv) {
  const args = { desktop: null, web: null, allowlist: null, report: null };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (flag === '--desktop') args.desktop = value, index += 1;
    else if (flag === '--web') args.web = value, index += 1;
    else if (flag === '--allowlist') args.allowlist = value, index += 1;
    else if (flag === '--report') args.report = value, index += 1;
    else if (flag === '--help' || flag === '-h') args.help = true;
    else throw new Error(`unknown argument: ${flag}`);
  }
  return args;
}

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'));
}

/** Every key of both maps, so a property one host declares and the other does not is a delta. */
function unionKeys(...objects) {
  const keys = new Set();
  for (const object of objects) {
    if (!object) continue;
    for (const key of Object.keys(object)) keys.add(key);
  }
  return [...keys].sort();
}

function sorted(values) {
  return [...values].sort();
}

/**
 * Grade one route. Returns `{ route, deltas, notices }`; every delta carries
 * the values from both hosts so the report can be reviewed without rerunning.
 */
function gradeRoute(route, slug, desktopDir, webDir) {
  const deltas = [];
  const notices = [];
  const push = (delta) => deltas.push({ route, ...delta });

  const desktopPath = join(desktopDir, `${slug}.desktop.json`);
  const webPath = join(webDir, `${slug}.web.json`);
  for (const [host, path] of [['desktop', desktopPath], ['web', webPath]]) {
    if (!existsSync(path)) {
      // A missing snapshot is never a design delta: the run itself failed.
      throw new Error(`missing ${host} snapshot for ${route}: ${path}`);
    }
  }

  const desktop = readJson(desktopPath);
  const web = readJson(webPath);
  for (const [host, snapshot] of [['desktop', desktop], ['web', web]]) {
    if (snapshot.schema !== SCHEMA) {
      throw new Error(`${host} snapshot for ${route} has schema ${snapshot.schema}, gate expects ${SCHEMA}`);
    }
    if (snapshot.errors && snapshot.errors.length) {
      throw new Error(`${host} host reported snapshot errors for ${route}: ${snapshot.errors.join('; ')}`);
    }
    if (!snapshot.mounted || !snapshot.mounted.app || !snapshot.mounted.view) {
      throw new Error(`${host} host did not mount the shell/view for ${route}: ${JSON.stringify(snapshot.mounted)}`);
    }
    if (snapshot.route_marker !== route) {
      throw new Error(
        `${host} host resolved route ${JSON.stringify(snapshot.route_marker)} but the gate asked for ${route}`,
      );
    }
  }

  // The graded frame: equal inner sizes are not required (a window manager may
  // claim a title bar), but both hosts must sit in the same breakpoint band or
  // different media-query branches are being compared.
  if (desktop.viewport.band !== web.viewport.band) {
    throw new Error(
      `viewport bands differ for ${route}: desktop ${desktop.viewport.width}x${desktop.viewport.height} ` +
        `(${desktop.viewport.band}) vs web ${web.viewport.width}x${web.viewport.height} (${web.viewport.band})`,
    );
  }
  notices.push({
    route,
    kind: 'viewport',
    detail: `desktop ${desktop.viewport.width}x${desktop.viewport.height}@${desktop.viewport.dpr}, ` +
      `web ${web.viewport.width}x${web.viewport.height}@${web.viewport.dpr}, band ${web.viewport.band}`,
  });

  // 1. Tokens: the values both hosts actually paint with.
  for (const name of unionKeys(desktop.tokens, web.tokens)) {
    const left = desktop.tokens?.[name] ?? null;
    const right = web.tokens?.[name] ?? null;
    if (left !== right) push({ kind: 'token', key: name, desktop: left, web: right });
  }

  // 2. Contract nodes: presence, class chain, contract attributes, computed values.
  for (const selector of unionKeys(desktop.nodes, web.nodes)) {
    const left = desktop.nodes?.[selector] ?? { present: false };
    const right = web.nodes?.[selector] ?? { present: false };
    if (left.present !== right.present) {
      push({
        kind: 'node',
        selector,
        desktop: left.present ? `present (${left.count})` : 'absent',
        web: right.present ? `present (${right.count})` : 'absent',
      });
      continue;
    }
    if (!left.present) {
      notices.push({ route, kind: 'node-absent-both', detail: selector });
      continue;
    }
    if (left.count !== right.count) {
      // Data-dependent (a tag per capability, a nav badge per count): review only.
      notices.push({ route, kind: 'count', detail: `${selector}: desktop ${left.count}, web ${right.count}` });
    }
    for (const token of unionKeys(
      Object.fromEntries((left.classes ?? []).map((name) => [name, true])),
      Object.fromEntries((right.classes ?? []).map((name) => [name, true])),
    )) {
      const inLeft = (left.classes ?? []).includes(token);
      const inRight = (right.classes ?? []).includes(token);
      if (inLeft !== inRight) push({ kind: 'class', selector, key: token, desktop: inLeft, web: inRight });
    }
    for (const name of unionKeys(left.attrs, right.attrs)) {
      const leftValue = left.attrs?.[name] ?? null;
      const rightValue = right.attrs?.[name] ?? null;
      if (leftValue !== rightValue) push({ kind: 'attr', selector, key: name, desktop: leftValue, web: rightValue });
    }
    for (const property of unionKeys(left.style, right.style)) {
      const leftValue = left.style?.[property] ?? null;
      const rightValue = right.style?.[property] ?? null;
      if (leftValue !== rightValue) {
        push({ kind: 'style', selector, property, desktop: leftValue, web: rightValue });
      }
    }
  }

  // 3. Inventories: printed for review, never graded.
  for (const region of ['chrome', 'view']) {
    const left = desktop[region] ?? {};
    const right = web[region] ?? {};
    const onlyLeft = sorted(Object.keys(left).filter((name) => !(name in right)));
    const onlyRight = sorted(Object.keys(right).filter((name) => !(name in left)));
    const countDiff = sorted(
      Object.keys(left).filter((name) => name in right && left[name] !== right[name]),
    );
    if (onlyLeft.length || onlyRight.length || countDiff.length) {
      notices.push({
        route,
        kind: `inventory-${region}`,
        detail: [
          onlyLeft.length ? `desktop-only: ${onlyLeft.join(' ')}` : null,
          onlyRight.length ? `web-only: ${onlyRight.join(' ')}` : null,
          countDiff.length ? `count differs: ${countDiff.join(' ')}` : null,
        ].filter(Boolean).join(' | '),
      });
    }
  }

  return { route, deltas, notices };
}

function validateAllowlist(allowlist) {
  const problems = [];
  if (allowlist.version !== 1) problems.push('allowlist version must be 1');
  if (!Array.isArray(allowlist.entries)) return [...problems, 'allowlist entries must be an array'];
  allowlist.entries.forEach((entry, index) => {
    const where = `entries[${index}]`;
    if (typeof entry.route !== 'string' || !(entry.route === '*' || entry.route.startsWith('/'))) {
      problems.push(`${where}.route must be "*" or a route path`);
    }
    if (!DELTA_KINDS.has(entry.kind)) {
      problems.push(`${where}.kind must be one of ${[...DELTA_KINDS].join(', ')}`);
    }
    for (const field of REQUIRED_FIELDS[entry.kind] ?? []) {
      if (typeof entry[field] !== 'string' || !entry[field].length) {
        problems.push(`${where} (${entry.kind}) needs a non-empty "${field}"`);
      }
    }
    if (typeof entry.why !== 'string' || entry.why.trim().length < 15) {
      problems.push(`${where}.why must explain the delta (>= 15 chars)`);
    }
    if (typeof entry.reviewed_by !== 'string' || !entry.reviewed_by.trim().length) {
      problems.push(`${where}.reviewed_by is required: an allowlist entry without a reviewer is not reviewed`);
    }
  });
  return problems;
}

/** An entry matches when every field it names is equal; `route: "*"` matches any route. */
function matches(entry, delta) {
  if (entry.route !== '*' && entry.route !== delta.route) return false;
  if (entry.kind !== delta.kind) return false;
  for (const field of ['selector', 'key', 'property']) {
    if (entry[field] !== undefined && entry[field] !== delta[field]) return false;
  }
  return true;
}

function formatValue(value) {
  if (value === null || value === undefined) return '∅';
  if (typeof value === 'boolean') return value ? 'present' : 'absent';
  return String(value);
}

function subject(delta) {
  switch (delta.kind) {
    case 'token':
      return `token ${delta.key}`;
    case 'node':
      return `node ${delta.selector}`;
    case 'class':
      return `${delta.selector} class "${delta.key}"`;
    case 'attr':
      return `${delta.selector} attribute ${delta.key}`;
    case 'style':
      return `${delta.selector} ${delta.property}`;
    default:
      return delta.key ?? '';
  }
}

function describe(delta) {
  return `${delta.route}  ${subject(delta)}  desktop=${formatValue(delta.desktop)}  web=${formatValue(delta.web)}`;
}

function main(argv) {
  let args;
  try {
    args = parseArgs(argv);
  } catch (error) {
    console.error(`compare: ${error.message}`);
    return 2;
  }
  if (args.help) {
    console.log('usage: compare.mjs --desktop <dir> --web <dir> [--allowlist <file>] [--report <file>]');
    return 0;
  }
  if (!args.desktop || !args.web) {
    console.error('compare: --desktop and --web are required');
    return 2;
  }

  let allowlist = { version: 1, entries: [] };
  if (args.allowlist) {
    try {
      allowlist = readJson(args.allowlist);
    } catch (error) {
      console.error(`compare: cannot read allowlist ${args.allowlist}: ${error.message}`);
      return 2;
    }
  }
  const allowlistProblems = validateAllowlist(allowlist);
  if (allowlistProblems.length) {
    console.error('compare: the allowlist itself is invalid:');
    for (const problem of allowlistProblems) console.error(`  - ${problem}`);
    return 1;
  }

  const routes = [];
  const allDeltas = [];
  const notices = [];
  try {
    for (const [route, slug] of ROUTES) {
      const graded = gradeRoute(route, slug, args.desktop, args.web);
      routes.push({ route, slug, deltas: graded.deltas.length });
      allDeltas.push(...graded.deltas);
      notices.push(...graded.notices);
    }
  } catch (error) {
    console.error(`compare: ${error.message}`);
    return 1;
  }

  const used = new Array(allowlist.entries.length).fill(0);
  const unexplained = [];
  const allowlisted = [];
  for (const delta of allDeltas) {
    let allowed = false;
    allowlist.entries.forEach((entry, index) => {
      if (matches(entry, delta)) {
        used[index] += 1;
        allowed = true;
      }
    });
    (allowed ? allowlisted : unexplained).push(delta);
  }
  const stale = allowlist.entries.filter((_, index) => used[index] === 0);

  const pass = unexplained.length === 0 && stale.length === 0;
  const report = {
    schema: SCHEMA,
    gate: 'G3 cross-host parity',
    compares: 'computed classes and token values',
    routes,
    deltas: allDeltas.length,
    allowlisted,
    unexplained,
    stale_allowlist_entries: stale,
    notices,
    result: pass ? 'pass' : 'fail',
  };
  if (args.report) writeFileSync(args.report, `${JSON.stringify(report, null, 2)}\n`);

  for (const route of routes) {
    console.log(`route ${route.route.padEnd(11)} ${String(route.deltas).padStart(3)} delta(s)`);
  }
  if (allowlisted.length) {
    console.log(`\nallowlisted deltas (${allowlisted.length}):`);
    for (const delta of allowlisted) console.log(`  ✓ ${describe(delta)}`);
  }
  if (unexplained.length) {
    console.log(`\nUnexplained deltas (${unexplained.length}):`);
    for (const delta of unexplained) console.log(`  ✗ ${describe(delta)}`);
    console.log(
      '\nEvery divergence needs either a fix or an entry in e2e/parity/allowlist.json\n' +
        'with a reason and a reviewer. When in doubt, fix the host.',
    );
  }
  if (stale.length) {
    console.log(`\nStale allowlist entries (${stale.length}): the delta they excuse no longer occurs.`);
    for (const entry of stale) console.log(`  ✗ ${entry.route} ${entry.kind} ${entry.key ?? entry.selector ?? ''}`);
    console.log('Delete them: an allowlist that outlives its delta hides the next regression.');
  }
  if (notices.length) {
    console.log(`\nReview material, not graded (${notices.length}):`);
    for (const notice of notices) console.log(`  · ${notice.route} ${notice.kind}: ${notice.detail}`);
  }

  console.log(`\nG3 cross-host parity: ${pass ? 'PASS' : 'FAIL'} (${allDeltas.length} delta(s), ${allowlisted.length} allowlisted)`);
  return pass ? 0 : 1;
}

process.exit(main(process.argv.slice(2)));
