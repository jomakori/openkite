// OKT-67 measurement harness.
//
// Serves the PRODUCTION vendored bundle (assets/vendored/openkite-react-spike)
// and a headless browser engine, fulfils the two same-origin bridge endpoints
// the way the Rust asset handlers do, then records the numbers the spike gates
// on: cold start -> first paint, bridge round-trip, 500-row render, idle RSS.
//
// The real wry webview cannot link in this environment (no glib-2.0) and needs
// a display, so the engine is a proxy for WebKitGTK — see the findings doc for
// the caveat. The wire envelopes are captured verbatim and asserted, so the
// bridge contract itself is verified exactly, independent of the engine.
//
// Usage:
//   node measure/measure.mjs [--engine=chromium|webkit] [--list-rows=30]
//                            [--rows=500] [--out=measure/report.json]

import { createServer } from 'node:http'
import { mkdtempSync, readFileSync, readdirSync, writeFileSync } from 'node:fs'
import { cpus, tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = resolve(here, '..')
const bundleDir = resolve(webRoot, '../assets/vendored/openkite-react-spike')

const args = Object.fromEntries(
  process.argv.slice(2).map((arg) => {
    const [key, value] = arg.replace(/^--/, '').split('=')
    return [key, value ?? true]
  }),
)
const engineName = String(args.engine ?? 'chromium')
const listRows = Number(args['list-rows'] ?? 30)
const renderRows = Number(args.rows ?? 500)
const baseline = Boolean(args.baseline)
const outPath = resolve(webRoot, String(args.out ?? 'measure/report.json'))

const appJs = readFileSync(join(bundleDir, 'app.js'))
const appCss = readFileSync(join(bundleDir, 'app.css'))

function podItem(index) {
  return {
    metadata: {
      name: `pod-${String(index).padStart(4, '0')}`,
      namespace: index % 3 === 0 ? 'kube-system' : 'default',
      creationTimestamp: new Date(Date.now() - index * 60_000).toISOString(),
    },
    status: {
      phase: ['Running', 'Pending', 'Succeeded'][index % 3],
      containerStatuses: [{ ready: index % 2 === 0 }, { ready: true }],
    },
  }
}

const listResult = {
  status: 'ok',
  result: {
    apiVersion: 'v1',
    kind: 'PodList',
    items: Array.from({ length: listRows }, (_, i) => podItem(i)),
  },
}

const received = { openkite: [], spike: [] }

const pageHead = `<meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>OKT-67 spike measurement</title>
    <script>window.__okt67ScriptStart = performance.now()</script>`

const indexHtml = `<!doctype html>
<html lang="en">
  <head>
    ${pageHead}
    <link rel="stylesheet" href="/app.css" />
  </head>
  <body class="bg-slate-950 text-slate-100 antialiased">
    <div id="root"></div>
    <script src="/app.js"></script>
  </body>
</html>`

// Empty-page control: the same engine/profile with no React bundle, so the
// renderer RSS delta over this baseline is the marginal cost of the UI layer.
const baselineHtml = `<!doctype html>
<html lang="en">
  <head>
    ${pageHead}
  </head>
  <body class="bg-slate-950 text-slate-100 antialiased">
    <div id="root"></div>
  </body>
</html>`

function readBody(req) {
  return new Promise((resolve) => {
    const chunks = []
    req.on('data', (chunk) => chunks.push(chunk))
    req.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')))
  })
}

const server = createServer(async (req, res) => {
  const url = req.url ?? '/'
  if (req.method === 'GET' && url === '/') {
    res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' }).end(baseline ? baselineHtml : indexHtml)
    return
  }
  if (req.method === 'GET' && url === '/app.js') {
    res.writeHead(200, { 'Content-Type': 'application/javascript' }).end(appJs)
    return
  }
  if (req.method === 'GET' && url === '/app.css') {
    res.writeHead(200, { 'Content-Type': 'text/css' }).end(appCss)
    return
  }
  if (req.method === 'POST' && url === '/openkite') {
    const body = await readBody(req)
    received.openkite.push(JSON.parse(body))
    res
      .writeHead(200, { 'Content-Type': 'application/json' })
      .end(JSON.stringify(listResult))
    return
  }
  if (req.method === 'POST' && url === '/openkite-spike') {
    const body = await readBody(req)
    received.spike.push(JSON.parse(body))
    res
      .writeHead(200, { 'Content-Type': 'application/json' })
      .end(JSON.stringify({ status: 'ok', result: { context: 'okt67-spike-context', connected: true } }))
    return
  }
  res.writeHead(404).end('not found')
})

await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
const port = server.address().port
const url = `http://127.0.0.1:${port}/`

function readProc(pid) {
  try {
    const status = readFileSync(`/proc/${pid}/status`, 'utf8')
    return {
      ppid: Number((status.match(/PPid:\s+(\d+)/) ?? [])[1] ?? -1),
      rssKb: Number((status.match(/VmRSS:\s+(\d+)/) ?? [])[1] ?? 0),
      cmdline: readFileSync(`/proc/${pid}/cmdline`, 'utf8'),
    }
  } catch {
    return null
  }
}

function allProcs() {
  return readdirSync('/proc')
    .filter((name) => /^\d+$/.test(name))
    .map(Number)
}

// Sum RSS over the browser process tree. The renderer(s) are the webview
// process equivalent; the browser process is reported for context.
function treeRss(rootPid) {
  const procs = new Map()
  for (const pid of allProcs()) {
    const info = readProc(pid)
    if (info) procs.set(pid, info)
  }
  const inTree = new Set([rootPid])
  let changed = true
  while (changed) {
    changed = false
    for (const [pid, info] of procs) {
      if (!inTree.has(pid) && inTree.has(info.ppid)) {
        inTree.add(pid)
        changed = true
      }
    }
  }
  let rendererKb = 0
  let allKb = 0
  const processes = []
  for (const pid of inTree) {
    const info = procs.get(pid)
    if (!info) continue
    const isRenderer =
      info.cmdline.includes('--type=renderer') || info.cmdline.includes('WebKitWebProcess')
    allKb += info.rssKb
    if (isRenderer) rendererKb += info.rssKb
    processes.push({ pid, rssKb: info.rssKb, isRenderer })
  }
  return { rendererKb, allKb, processes }
}

function findRootPid(profileDir) {
  for (const pid of allProcs()) {
    const info = readProc(pid)
    if (info && info.cmdline.includes(profileDir) && !info.cmdline.includes('--type=')) {
      return pid
    }
  }
  return null
}

const profileDir = mkdtempSync(join(tmpdir(), `okt67-${engineName}-`))
const playwright = await import('playwright')
const engine = playwright[engineName]
if (!engine) throw new Error(`unknown engine ${engineName}`)

const browserArgs =
  engineName === 'chromium'
    ? ['--no-sandbox', '--disable-dev-shm-usage', '--js-flags=--expose-gc']
    : []
const context = await engine.launchPersistentContext(profileDir, {
  headless: true,
  args: browserArgs,
})
const page = context.pages()[0] ?? (await context.newPage())

const pageErrors = []
page.on('pageerror', (err) => pageErrors.push(String(err)))
page.on('console', (msg) => {
  if (msg.type() === 'error') pageErrors.push(msg.text())
})

const wallStart = Date.now()
await page.goto(url, { waitUntil: 'load', timeout: 30_000 })
let readyWallMs = null
let cold = null
let renderBench = null

if (!baseline) {
  await page.waitForFunction(() => window.__openkite_react_spike?.ready === true, null, {
    timeout: 30_000,
  })
  readyWallMs = Date.now() - wallStart
  try {
    await page.waitForFunction(() => window.__openkite_react_spike?.loaded === true, null, {
      timeout: 30_000,
    })
  } catch {
    pageErrors.push('controller.loaded never became true')
  }

  cold = await page.evaluate(() => {
    const controller = window.__openkite_react_spike
    const nav = performance.getEntriesByType('navigation')[0]
    const fcp = performance.getEntriesByName('first-contentful-paint')[0]
    return {
      bundleStartMs: controller.bundleStartMs,
      mountMs: controller.mountMs,
      firstPaintMs: controller.firstPaintMs,
      loadedMs: controller.loadedMs,
      contextMs: controller.contextMs,
      listMs: controller.listMs,
      rowCount: controller.rowCount,
      bridgeMode: controller.bridgeMode,
      error: controller.error,
      nav: nav
        ? { responseEndMs: nav.responseEnd, domContentLoadedMs: nav.domContentLoadedEventEnd, loadMs: nav.loadEventEnd }
        : null,
      fcpMs: fcp ? fcp.startTime : null,
    }
  })

  const syntheticRows = Array.from({ length: renderRows }, (_, i) => ({
    name: `pod-${String(i).padStart(4, '0')}`,
    namespace: i % 3 === 0 ? 'kube-system' : 'default',
    phase: ['Running', 'Pending', 'Succeeded'][i % 3],
    ready: `${(i % 2) + 1}/2`,
    age: `${i % 60}m`,
  }))

  renderBench = await page.evaluate(async (rows) => {
    const controller = window.__openkite_react_spike
    const samples = []
    for (let i = 0; i < 7; i += 1) samples.push(await controller.renderRows(rows))
    const sorted = [...samples].sort((a, b) => a - b)
    return {
      samples,
      minMs: sorted[0],
      medianMs: sorted[Math.floor(sorted.length / 2)],
      maxMs: sorted[sorted.length - 1],
      rowsRendered: controller.rowCount,
      renderCount: controller.renderCount,
    }
  }, syntheticRows)
}

await page.waitForTimeout(1500)
const rootPid = findRootPid(profileDir)
const emptyRss = { rendererKb: 0, allKb: 0, processes: [] }
const rssBeforeGc = rootPid ? treeRss(rootPid) : emptyRss

await page.evaluate(() => {
  if (typeof window.gc === 'function') window.gc()
})
await page.waitForTimeout(800)
const rss = rootPid ? treeRss(rootPid) : emptyRss

await context.close()
server.close()

const bundleBytes = { appJs: appJs.length, appCss: appCss.length }

const report = {
  ticket: 'OKT-67',
  engine: engineName,
  measuredAt: new Date().toISOString(),
  env: {
    node: process.version,
    platform: process.platform,
    arch: process.arch,
    cpus: cpus().length,
  },
  bundle: {
    appJsBytes: bundleBytes.appJs,
    appCssBytes: bundleBytes.appCss,
    totalBytes: bundleBytes.appJs + bundleBytes.appCss,
  },
  baseline,
  coldStart: cold ? { ...cold, readyWallMs } : null,
  renderBench,
  idleRss: {
    rendererKb: rss.rendererKb,
    allKb: rss.allKb,
    processes: rss.processes,
  },
  idleRssBeforeGc: {
    rendererKb: rssBeforeGc.rendererKb,
    allKb: rssBeforeGc.allKb,
  },
  wire: {
    openkitePost: received.openkite[0] ?? null,
    spikePost: received.spike[0] ?? null,
    openkitePostCount: received.openkite.length,
    spikePostCount: received.spike.length,
  },
  pageErrors,
}

writeFileSync(outPath, JSON.stringify(report, null, 2))

const kb = (n) => (n / 1024).toFixed(2)
console.log(`engine:            ${engineName}`)
console.log(`mode:              ${baseline ? 'baseline (no UI bundle)' : 'spike'}`)
console.log(`bundle:            app.js ${bundleBytes.appJs} B + app.css ${bundleBytes.appCss} B`)
if (cold) {
  console.log(`cold first paint:  ${cold.firstPaintMs?.toFixed(2)} ms (bundle-relative)`)
  console.log(`cold loaded:       ${cold.loadedMs?.toFixed(2)} ms (bundle-relative, ${cold.rowCount} rows)`)
  console.log(`context rtt:       ${cold.contextMs?.toFixed(2)} ms`)
  console.log(`list rtt:          ${cold.listMs?.toFixed(2)} ms`)
}
if (renderBench) {
  console.log(`render ${renderRows} rows:  min ${renderBench.minMs.toFixed(2)} / median ${renderBench.medianMs.toFixed(2)} / max ${renderBench.maxMs.toFixed(2)} ms`)
}
console.log(
  `idle renderer RSS: ${rss.rendererKb} kB (${kb(rss.rendererKb)} MiB) post-GC / ` +
    `${rssBeforeGc.rendererKb} kB (${kb(rssBeforeGc.rendererKb)} MiB) pre-GC`,
)
console.log(
  `idle tree RSS:     ${rss.allKb} kB (${kb(rss.allKb)} MiB) post-GC / ` +
    `${rssBeforeGc.allKb} kB (${kb(rssBeforeGc.allKb)} MiB) pre-GC`,
)
if (cold) console.log(`bridge mode:       ${cold.bridgeMode}`)
console.log(`wire openkite:     ${received.openkite.length} POST(s), spike: ${received.spike.length} POST(s)`)
console.log(`page errors:       ${pageErrors.length}`)
console.log(`report:            ${outPath}`)
