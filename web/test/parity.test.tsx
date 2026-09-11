// Fixture-driven DOM parity check for the browser/staging build.
//
// No browser binary is available in this environment, so this renders the
// React tree the way Vite ships it — same components, same fixture data as a
// static host — and asserts the DOM text/class inventory. The desktop host
// renders these same components; the only difference is the data source (host
// bridge vs fixture fallback), and both return the same kube `List` shape. The
// check also confirms the desktop (vendored) and web (dist) bundles carry the
// same UI markers.
//
// Run via `npm run test:parity` (builds web/dist, then SSR-builds this file).
import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import { resolve } from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { ResourceTable } from '../src/ResourceTable'
import { Shell } from '../src/Shell'
import { countRows, rowsFromList } from '../src/bridge'
import { FIXTURE_CONTEXT, fixtureList } from '../src/fixtures'
import { COUNTED_KINDS } from '../src/shell/nav'

const webRoot = process.cwd()
const repoRoot = resolve(webRoot, '..')

function classTokens(html: string): Set<string> {
  const out = new Set<string>()
  for (const match of html.matchAll(/class="([^"]*)"/g)) {
    for (const token of match[1].split(/\s+/)) if (token) out.add(token)
  }
  return out
}

// 1. Fixtures cover every counted kind and the pods list the spike table reads.
for (const kind of COUNTED_KINDS) {
  const count = countRows(fixtureList(kind, null))
  assert.ok(count > 0, `fixture for '${kind}' must have rows (got ${count})`)
}
assert.equal(countRows(fixtureList('pods', 'default')), 6, 'default-namespace pod fixture count')
assert.equal(countRows(fixtureList('pods', null)), 12, 'all-namespace pod fixture count')

// 2. The shell renders live content with fixtures: chrome, nav badges, context.
const counts = Object.fromEntries(
  COUNTED_KINDS.map((kind) => [kind, countRows(fixtureList(kind, null))]),
)
const shellHtml = renderToStaticMarkup(
  <Shell initialCounts={counts} initialContext={FIXTURE_CONTEXT} />,
)

const shellClasses = classTokens(shellHtml)
for (const required of [
  'app',
  'sidebar',
  'brand',
  'brand-word',
  'cluster-btn',
  'nav',
  'nav-section',
  'nav-title',
  'nav-item',
  'nav-badge',
  'nav-divider',
  'main',
  'topbar',
  'breadcrumbs',
  'view',
  'active',
  'page-head',
  'eyebrow',
  'sidebar-footer',
  'dot',
  'ok',
]) {
  assert.ok(shellClasses.has(required), `shell DOM missing class token '${required}'`)
}

for (const marker of [
  'class="brand-word">Open<strong>Kite</strong>',
  FIXTURE_CONTEXT.context as string,
  'Connected',
  'class="nav-item active"',
  'class="nav-badge">12<',
  'class="nav-divider"',
  '<h1>Pods</h1>',
  'class="eyebrow">Workloads<',
]) {
  assert.ok(shellHtml.includes(marker), `shell DOM missing '${marker}'`)
}

for (const label of ['Nodes', 'Pods', 'Deployments', 'Services', 'ConfigMaps', 'Argo CD']) {
  assert.ok(shellHtml.includes(`>${label}<`), `shell DOM missing nav label '${label}'`)
}

// 3. The spike table renders rows from fixture data, not the empty state.
const rows = rowsFromList(fixtureList('pods', 'default'), Date.now())
assert.equal(rows.length, 6, 'rowsFromList(fixture pods) count')
const tableHtml = renderToStaticMarkup(<ResourceTable rows={rows} source="fixtures" />)
assert.ok(tableHtml.includes('openkite-api-6d9f4b7c8-2xk4p'), 'table missing fixture pod name')
assert.ok(!tableHtml.includes('data-testid="empty-state"'), 'table rendered the empty state')
assert.equal(
  (tableHtml.match(/data-testid="resource-row"/g) ?? []).length,
  6,
  'table row count',
)

// 4. Desktop (vendored) and web (dist) bundles ship the same UI markers.
const markers = [
  'openkite-react-spike-root',
  'brand-word',
  'cluster-btn',
  'nav-badge',
  'nav-divider',
  'sidebar-footer',
]
const desktopBundle = readFileSync(
  resolve(repoRoot, 'assets/vendored/openkite-react-spike/app.js'),
  'utf8',
)
for (const marker of markers) {
  assert.ok(desktopBundle.includes(marker), `desktop bundle missing '${marker}'`)
}

const distAssets = resolve(webRoot, 'dist/assets')
const distJs = readdirSync(distAssets).find((name) => name.endsWith('.js'))
assert.ok(distJs, 'web/dist/assets/*.js not found — run npm run build:web')
const webBundle = readFileSync(resolve(distAssets, distJs as string), 'utf8')
for (const marker of markers) {
  assert.ok(webBundle.includes(marker), `web bundle missing '${marker}'`)
}

console.log(
  `parity ok: fixtures for ${COUNTED_KINDS.length} kinds, shell + table DOM asserted, ` +
    `${markers.length} markers present in both bundles`,
)
