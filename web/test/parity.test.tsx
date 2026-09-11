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
import { Inspector } from '../src/Inspector'
import { LogDock } from '../src/LogDock'
import { ResourceTable } from '../src/ResourceTable'
import { ResourceView } from '../src/ResourceView'
import { Shell } from '../src/Shell'
import { countRows, rowsFromList, rowsFromPayload } from '../src/bridge'
import { FIXTURE_CONTEXT, fixtureList } from '../src/fixtures'
import { fixtureLogLines, parseLogLines } from '../src/logs'
import { BottomNav } from '../src/shell/BottomNav'
import { PullIndicator } from '../src/shell/PullIndicator'
import { ToastViewport } from '../src/shell/Toast'
import { COUNTED_KINDS } from '../src/shell/nav'
import {
  filterRows,
  namespaceCounts,
  pageCount,
  PAGE_SIZE,
  pageItems,
  paginate,
  sortRows,
  statusTone,
} from '../src/table'

const webRoot = process.cwd()
const repoRoot = resolve(webRoot, '..')

function classTokens(html: string): Set<string> {
  const out = new Set<string>()
  for (const match of html.matchAll(/class="([^"]*)"/g)) {
    for (const token of match[1].split(/\s+/)) if (token) out.add(token)
  }
  return out
}

function assertClasses(html: string, tokens: string[], label: string): void {
  const classes = classTokens(html)
  for (const token of tokens) {
    assert.ok(classes.has(token), `${label} missing class token '${token}'`)
  }
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

assertClasses(
  shellHtml,
  [
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
    'toolbar',
    'resource-table',
    'log-panel',
    'bottom-nav',
    'toast',
  ],
  'shell DOM',
)

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

// 2b. The host hands the console the nav id of the taken-over route; the
// shell must open on that view (and its breadcrumb) rather than always Pods.
const configHtml = renderToStaticMarkup(
  <Shell route="configmaps" initialCounts={counts} initialContext={FIXTURE_CONTEXT} />,
)
assert.ok(configHtml.includes('<h1>ConfigMaps</h1>'), 'config route opens ConfigMaps')
assert.ok(configHtml.includes('Config &amp; Storage'), 'config route breadcrumbs its section')
assert.ok(
  configHtml.includes('class="nav-item active"') &&
    configHtml.includes('aria-current="page"'),
  'config route marks a nav item active',
)

const overviewHtml = renderToStaticMarkup(
  <Shell route="overview" initialCounts={counts} initialContext={FIXTURE_CONTEXT} />,
)
assert.ok(overviewHtml.includes('<h1>Overview</h1>'), 'cluster route opens Overview')
assert.ok(overviewHtml.includes('class="eyebrow">Cluster<'), 'cluster route breadcrumbs its section')

// 3. The spike table renders rows from fixture data, not the empty state.
const pods = rowsFromList(fixtureList('pods', null), Date.now())
const defaultPods = rowsFromList(fixtureList('pods', 'default'), Date.now())
assert.equal(defaultPods.length, 6, 'rowsFromList(fixture pods default) count')
const tableHtml = renderToStaticMarkup(<ResourceTable rows={defaultPods} source="fixtures" />)
assert.ok(tableHtml.includes('openkite-api-6d9f4b7c8-2xk4p'), 'table missing fixture pod name')
assert.ok(!tableHtml.includes('data-testid="empty-state"'), 'table rendered the empty state')
assert.equal(
  (tableHtml.match(/data-testid="resource-row"/g) ?? []).length,
  6,
  'table row count',
)
assertClasses(
  tableHtml,
  ['pill', 'success', 'warn', 'danger', 'health-dots', 'dot', 'ok', 'err', 'restarts'],
  'fixture table tones',
)

// 4. Pure table/logic helpers behave as the render expects.
assert.equal(statusTone('Running'), 'success', 'Running tone')
assert.equal(statusTone('Synced'), 'success', 'Synced tone')
assert.equal(statusTone('Pending'), 'warn', 'Pending tone')
assert.equal(statusTone('OutOfSync'), 'warn', 'OutOfSync tone')
assert.equal(statusTone('CrashLoopBackOff'), 'danger', 'CrashLoopBackOff tone')
assert.equal(statusTone('Unknown'), 'neutral', 'Unknown tone')

const byName = sortRows(pods, 'name', 'asc')
assert.ok(
  byName[0].name.localeCompare(byName[byName.length - 1].name) <= 0,
  'name ascending sort',
)
const byAge = sortRows(pods, 'age', 'desc')
assert.ok(byAge[0].ageSeconds >= byAge[byAge.length - 1].ageSeconds, 'age descending sort')
assert.equal(sortRows(pods, 'restarts', 'desc')[0].restarts, 14, 'restarts descending sort')

const crashPod = pods.find((pod) => pod.name.startsWith('redis-'))
assert.ok(crashPod, 'redis fixture pod present')
assert.equal(crashPod?.restarts, 14, 'restartCount mapped onto the row')
assert.equal(crashPod?.controller, 'Deployment/redis', 'ownerReference mapped onto controller')
assert.ok(crashPod?.health.includes(false), 'unready container mapped onto health')

assert.equal(namespaceCounts(pods).length, 3, 'namespace count')
assert.equal(filterRows(pods, 'default', '').length, 6, 'namespace filter')
assert.equal(filterRows(pods, 'all', 'coredns').length, 2, 'text filter')
assert.equal(pageCount(pods.length), Math.ceil(pods.length / PAGE_SIZE), 'page count')
assert.equal(paginate(pods, 1).length, PAGE_SIZE, 'paginate page 1')
assert.deepEqual(pageItems(1, 1), [1], 'single-page pager')
assert.deepEqual(pageItems(1, 3), [1, 2, 3], 'three-page pager')

assert.equal(rowsFromPayload(fixtureList('pods', null), Date.now()).length, 12, 'payload List shape')
const bareItems = (fixtureList('pods', null) as { items: unknown[] }).items
assert.equal(rowsFromPayload(bareItems, Date.now()).length, 12, 'payload bare-array shape')

const fixtureLogs = parseLogLines(fixtureLogLines('demo-pod'))
assert.ok(fixtureLogs.some((line) => line.level === 'ERROR'), 'fixture logs carry an error line')
const hostLogs = parseLogLines('10:42:07.114 INFO GET /healthz 200 1.2ms')
assert.equal(hostLogs[0].method, 'GET', 'host log method parsed')
assert.equal(hostLogs[0].level, 'INFO', 'host log level parsed')

// 5. Resource view: toolbar + sortable table + pagination with fixture rows.
const viewHtml = renderToStaticMarkup(
  <ResourceView
    rows={pods}
    section="Workloads"
    title="Pods"
    noun="pods"
    source="fixtures"
    onSelect={() => {}}
    onToast={() => {}}
  />,
)
assertClasses(
  viewHtml,
  [
    'page-head',
    'page-actions',
    'btn',
    'btn-primary',
    'btn-secondary',
    'toolbar',
    'chip-row',
    'chip',
    'active',
    'search-field',
    'panel',
    'table-wrap',
    'resource-table',
    'th-sort',
    'pill',
    'success',
    'health-dots',
    'restarts',
    'controller',
    'node',
    'qos',
    'panel-footer',
    'pager',
  ],
  'resource view',
)
assert.equal(
  (viewHtml.match(/data-testid="resource-row"/g) ?? []).length,
  PAGE_SIZE,
  'resource view first page row count',
)
assert.ok(viewHtml.includes('Showing 8 of 12 pods'), 'pagination summary')
assert.ok(viewHtml.includes('chip active'), 'namespace "All" chip active')

// 6. Inspector slide-over is an overlay rendered on top of the current view.
const inspectorHtml = renderToStaticMarkup(
  <Inspector
    row={pods[0]}
    open
    onClose={() => {}}
    onViewLogs={() => {}}
    onToast={() => {}}
  />,
)
assertClasses(
  inspectorHtml,
  [
    'inspector',
    'open',
    'inspector-scrim',
    'show',
    'inspector-header',
    'inspector-body',
    'inspector-eyebrow',
    'kv-list',
    'kv-row',
    'inspector-actions',
    'btn',
  ],
  'inspector',
)
assert.ok(inspectorHtml.includes('Resource summary'), 'inspector summary heading')
assert.ok(inspectorHtml.includes(pods[0].name), 'inspector carries the selected row name')

// 7. Log dock is an inline collapsible panel fed by log lines.
const logHtml = renderToStaticMarkup(
  <LogDock
    open
    paused
    collapsed={false}
    pod={pods[0].name}
    lines={fixtureLogLines(pods[0].name)}
    onTogglePause={() => {}}
    onToggleCollapse={() => {}}
    onClear={() => {}}
    onClose={() => {}}
  />,
)
assertClasses(
  logHtml,
  [
    'log-panel',
    'open',
    'paused',
    'log-header',
    'log-title',
    'log-pod',
    'log-actions',
    'log-paused',
    'log-body',
    'log-line',
    'log-time',
    'log-level',
    'log-msg',
  ],
  'log dock',
)
assert.ok(logHtml.includes('Log stream paused'), 'log dock shows the paused banner')

// 8. Toast, pull indicator and bottom nav primitives.
const toastHtml = renderToStaticMarkup(<ToastViewport message="Resources refreshed" show />)
assertClasses(toastHtml, ['toast', 'show'], 'toast')
assert.ok(toastHtml.includes('Resources refreshed'), 'toast shows its message')

const pullHtml = renderToStaticMarkup(<PullIndicator show text="Release to refresh" />)
assertClasses(pullHtml, ['pull-indicator', 'show', 'spinner'], 'pull indicator')

const navHtml = renderToStaticMarkup(
  <BottomNav
    activeSection="workloads"
    logOpen
    onSelect={() => {}}
    onToggleLogs={() => {}}
    onMenu={() => {}}
  />,
)
assertClasses(
  navHtml,
  ['bottom-nav', 'bottom-tabs', 'bottom-tab', 'active', 'icon'],
  'bottom nav',
)

// 9. theme.css defines every scoped selector these primitives rely on.
const css = readFileSync(resolve(webRoot, 'src/theme.css'), 'utf8').replace(/\s+/g, ' ')
for (const selector of [
  '#openkite-react-spike-root .toolbar',
  '#openkite-react-spike-root .chip.active',
  '#openkite-react-spike-root .search-field input',
  '#openkite-react-spike-root .panel',
  '#openkite-react-spike-root .resource-table th',
  '#openkite-react-spike-root .resource-table.compact th',
  '#openkite-react-spike-root .th-sort',
  '#openkite-react-spike-root .pill.success',
  '#openkite-react-spike-root .panel-footer',
  '#openkite-react-spike-root .pager button.active',
  '#openkite-react-spike-root .inspector.open',
  '#openkite-react-spike-root .inspector-scrim.show',
  '#openkite-react-spike-root .kv-list',
  '#openkite-react-spike-root .log-panel',
  '#openkite-react-spike-root .log-line',
  '#openkite-react-spike-root .toast.show',
  '#openkite-react-spike-root .bottom-nav',
  '#openkite-react-spike-root .pull-indicator.show',
  '#openkite-react-spike-root .spinner',
]) {
  assert.ok(css.includes(selector), `theme.css missing scoped selector '${selector}'`)
}

// 10. Desktop (vendored) and web (dist) bundles ship the same UI markers.
const markers = [
  'openkite-react-spike-root',
  '__openkite_react_console',
  'brand-word',
  'cluster-btn',
  'nav-badge',
  'nav-divider',
  'sidebar-footer',
  'resource-table',
  'th-sort',
  'inspector',
  'kv-list',
  'log-panel',
  'log-body',
  'toast',
  'bottom-nav',
  'pull-indicator',
  'pager',
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
  `parity ok: fixtures for ${COUNTED_KINDS.length} kinds, shell + table + inspector + log dock + toast DOM asserted, ` +
    `${markers.length} markers present in both bundles`,
)
