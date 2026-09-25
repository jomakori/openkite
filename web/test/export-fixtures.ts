// Fixture exporter for the console parity gate (OKT-129).
//
// The parity gate renders the SAME surface twice from the SAME payloads: once
// in headless Chromium over the served `web/dist` (a static host answers
// `POST /openkite` with 405, so the console falls back to the fixtures it
// bundles) and once on the real `openkite` binary under Xvfb with
// `OPENKITE_CONSOLE_FIXTURES` pointing at this file. The browser consumes the
// compiled `web/src/fixtures.ts` and the host consumes this export of the very
// same module, so the two sides agree by construction.
//
// Two things about the clock:
//
// - `web/src/fixtures.ts` builds `metadata.creationTimestamp` from
//   `Date.now()` (fixture ages are readable: "12m", not "2026-09-18"), so the
//   ages a render shows depend on WHEN the data was built — not on the data
//   itself. This file therefore records `exportedAt`, and the host rebases
//   every timestamp onto its own clock when it loads it
//   (`src/console_fixtures.rs`), which reproduces exactly what the browser
//   renders from its own load-time clock. Without that, the committed copy
//   would age by the time CI renders it and the desktop would show "241m"
//   where the browser shows "12m".
// - Because of the same clock, this file can never be byte-compared against a
//   fresh export. `--check` re-exports in memory and compares with the clock
//   fields stripped, so a stale copy (renamed pod, changed phase, wrong
//   namespace) still fails the job.
//
// Run via `npm run export:parity-fixtures` (write) or
// `npm run export:parity-fixtures -- --check` (verify, writes nothing).
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { FIXTURE_CONTEXT, fixtureList } from '../src/fixtures'
import { COUNTED_KINDS } from '../src/shell/nav'

// The npm script runs from `web/` (same convention as test/parity.test.tsx).
const webRoot = process.cwd()
const out = resolve(webRoot, '..', 'e2e', 'parity', 'fixtures.json')

// Every kind the console asks the bridge for during a parity capture: the
// counted kinds behind the nav badges and the overview summary, plus the kinds
// the shell chrome and detail surfaces read (`namespaces` for the topbar
// filter, `services`/`configmaps` for the table views, `events` for the
// detail panel).
const KINDS = [
  ...new Set([...COUNTED_KINDS, 'pods', 'configmaps', 'services', 'events', 'namespaces']),
]

/** Fields that encode when the payload was built rather than what it is. */
const CLOCK_KEYS = new Set(['exportedAt', 'creationTimestamp'])

/** Recursively drop the clock fields and sort keys, for a stable comparison. */
function withoutClock(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(withoutClock)
  if (!value || typeof value !== 'object') return value
  const out: Record<string, unknown> = {}
  for (const key of Object.keys(value as Record<string, unknown>).sort()) {
    if (CLOCK_KEYS.has(key)) continue
    out[key] = withoutClock((value as Record<string, unknown>)[key])
  }
  return out
}

const kinds: Record<string, unknown> = {}
for (const kind of KINDS) {
  const list = fixtureList(kind, null)
  const items = (list as { items?: unknown[] }).items
  if (!Array.isArray(items)) {
    throw new Error(`fixtures: '${kind}' did not produce a kube List with items`)
  }
  kinds[kind] = list
}
const payload = { context: FIXTURE_CONTEXT, exportedAt: Date.now(), kinds }
const summary = Object.entries(kinds)
  .map(([kind, list]) => `${kind}=${((list as { items: unknown[] }).items ?? []).length}`)
  .join(' ')

if (process.argv.includes('--check')) {
  let committed: unknown
  try {
    committed = JSON.parse(readFileSync(out, 'utf8'))
  } catch (err) {
    console.error(`check-parity-fixtures: cannot read ${out}: ${String(err)}`)
    console.error('check-parity-fixtures: run `npm run export:parity-fixtures` and commit the copy')
    process.exit(1)
  }
  const before = JSON.stringify(withoutClock(committed))
  const after = JSON.stringify(withoutClock(payload))
  if (before !== after) {
    console.error(`check-parity-fixtures: ${out} is STALE (differs from web/src/fixtures.ts)`)
    console.error(`check-parity-fixtures: committed ${before.length} B vs fresh ${after.length} B`)
    console.error('check-parity-fixtures: re-export and commit the copy in this PR')
    process.exit(1)
  }
  console.log(`check-parity-fixtures: ${out} matches web/src/fixtures.ts (${summary})`)
} else {
  mkdirSync(dirname(out), { recursive: true })
  writeFileSync(out, `${JSON.stringify(payload, null, 2)}\n`)
  console.log(`export-fixtures: wrote ${out}`)
  console.log(`export-fixtures: ${Object.keys(kinds).length} kinds (${summary})`)
}
