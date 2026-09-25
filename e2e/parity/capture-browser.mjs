// Browser-side capture for the console parity gate (OKT-129).
//
// Renders the served `web/dist` in headless Chromium and screenshots each
// parity surface at the desktop's frame size. This is the DEPLOYED path, not a
// simulation of it: the preview image is static nginx over `web/dist`, so a
// `POST /openkite` is answered by the static handler (`405` from nginx; `501`
// from `python3 -m http.server`) — a transport failure that `web/src/bridge.ts`
// turns into the bundled-fixture fallback. The captures are only valid if that
// fallback really is live, so the script proves it three ways before it writes
// anything:
//
//   1. a probe POST to `/openkite` must NOT be answered by a bridge host,
//   2. the topbar must show `FIXTURE_CONTEXT.context` ("staging (fixtures)"),
//   3. each table surface must show at least one name from `fixtures.json`.
//
// Without those, a fixture render could silently be compared against fixture
// renders and called parity.
//
// Surfaces are reached through the console's own route API
// (`window.__openkite_react_console.setRoute`, the entry the desktop host
// drives from `src/react_spike.rs`) — not by clicking. At 800×600 the sidebar
// is an off-canvas overlay (theme.css `@media (max-width: 1024px)`), so
// clicking a nav item would open it and capture a different shell state than
// the desktop renders.
//
// `playwright` is a dev dependency of `web/`, and a bare import would be
// resolved against THIS file's directory (`e2e/parity/`), where it does not
// exist. Load it from the web package explicitly — `OPENKITE_WEB_ROOT`
// overrides the default location.
//
// Run it as:
//   node e2e/parity/capture-browser.mjs \
//     http://127.0.0.1:8000 e2e/parity/out 800x600 e2e/parity/fixtures.json
import { mkdirSync, readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = process.env.OPENKITE_WEB_ROOT ?? resolve(here, '..', '..', 'web')
const playwrightEntry = resolve(webRoot, 'node_modules', 'playwright', 'index.mjs')

let chromium
try {
  ;({ chromium } = await import(pathToFileURL(playwrightEntry).href))
} catch (err) {
  console.error(`[parity] cannot load playwright from ${playwrightEntry}: ${String(err)}`)
  console.error('[parity] install it with `cd web && npm ci` (or `npx playwright install`)')
  process.exit(2)
}

const [base, outDir, size = '800x600', fixturesPath = resolve(here, 'fixtures.json')] =
  process.argv.slice(2)
if (!base || !outDir) {
  console.error('usage: capture-browser.mjs <base-url> <out-dir> [WxH] [fixtures.json]')
  process.exit(2)
}
const [width, height] = size.split('x').map(Number)
const fixtures = JSON.parse(readFileSync(fixturesPath, 'utf8'))
const itemNames = (kind) =>
  (fixtures.kinds?.[kind]?.items ?? []).map((item) => item?.metadata?.name).filter(Boolean)

const SURFACES = [
  { id: '01-pods', route: 'pods', names: itemNames('pods') },
  { id: '02-overview', route: 'overview', names: [] },
  { id: '03-configmaps', route: 'configmaps', names: itemNames('configmaps') },
]

/** Resource-load errors are expected here: they are the fixture fallback firing. */
const EXPECTED_TRANSPORT_ERROR =
  /Failed to load resource: the server responded with a status of (405|501)/

const fail = (message) => {
  console.error(`[parity] FAIL: ${message}`)
  process.exitCode = 1
}

mkdirSync(outDir, { recursive: true })

const browser = await chromium.launch({
  args: [
    '--force-color-profile=srgb',
    '--disable-lcd-text',
    '--font-render-hinting=none',
    // GTK draws overlay scrollbars that reserve no gutter, so suppress
    // Chromium's classic ones rather than compare a gutter to nothing.
    '--hide-scrollbars',
  ],
})
const page = await browser.newPage({
  viewport: { width, height },
  deviceScaleFactor: 1,
  reducedMotion: 'reduce',
  colorScheme: 'light',
})

const problems = []
page.on('pageerror', (err) => problems.push(`pageerror: ${String(err)}`))
page.on('console', (msg) => {
  if (msg.type() !== 'error') return
  const text = msg.text()
  if (EXPECTED_TRANSPORT_ERROR.test(text)) return
  problems.push(`console error: ${text}`)
})

await page.goto(base, { waitUntil: 'networkidle' })

// 1. The static host must not be a bridge host: if something answers this
//    POST, the console would render host data and the capture would measure a
//    different data path than the committed baseline.
const probe = await page.evaluate(async () => {
  try {
    const res = await fetch('/openkite', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        id: 1,
        plugin: 'openkite-react-spike',
        request: { op: 'list', kind: 'pods', ns: null },
      }),
    })
    return { ok: res.ok, status: res.status }
  } catch (err) {
    return { ok: false, status: 0, error: String(err) }
  }
})
if (probe.ok) {
  fail(
    `a bridge host answered POST /openkite (HTTP ${probe.status}); ` +
      'this capture is not the static-host fixture path',
  )
} else {
  console.log(
    `[parity] browser: POST /openkite refused (HTTP ${probe.status}) -> fixture fallback`,
  )
}

for (const { id, route, names } of SURFACES) {
  const applied = await page.evaluate((target) => {
    const console_ = window.__openkite_react_console
    if (!console_ || typeof console_.setRoute !== 'function') return false
    console_.setRoute(target)
    return true
  }, route)
  if (!applied) {
    fail('window.__openkite_react_console.setRoute is unavailable — the console did not mount')
    break
  }
  // The console fetches the new kind's rows through the bridge and repaints.
  await page.waitForTimeout(900)

  const text = await page.evaluate(() => document.body.innerText)
  const marker = fixtures.context?.context
  if (marker && !text.includes(marker)) {
    fail(`${id}: cluster context '${marker}' is not rendered — fixture fallback is not live`)
  }
  if (names.length && !names.some((name) => text.includes(name))) {
    fail(`${id}: none of the ${names.length} fixture names are rendered`)
  }
  if (!text.trim()) fail(`${id}: empty render`)

  const path = resolve(outDir, `${id}.browser.png`)
  await page.screenshot({ path, animations: 'disabled' })
  console.log(`[parity] browser ${id}: route=${route} -> ${path}`)
}

const viewport = page.viewportSize()
console.log(`[parity] browser viewport ${viewport.width}x${viewport.height}`)
await browser.close()

if (problems.length) {
  for (const problem of problems) console.error(`[parity] ${problem}`)
  fail(`${problems.length} browser-side error(s) — a console error is a finding, not noise`)
}
if (process.exitCode) process.exit(process.exitCode)
console.log(`[parity] browser capture ok: ${SURFACES.length} surfaces at ${size}`)
