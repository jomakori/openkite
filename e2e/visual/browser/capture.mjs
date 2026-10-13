#!/usr/bin/env node
// Capture hydrated-client visual baselines of the browser host's core
// surfaces at the three breakpoints the OpenDesign mockup is graded at
// (OKT-191, Decision D1: grade against the mockup directly, not desktop).
//
// Boots the production `openkite-web` host and drives a real headless
// Chromium/Chrome process over the DevTools Protocol directly -- same
// no-dependency approach as e2e/browser/harness.mjs (OKT-188). No
// Playwright, no browser automation package is installed.
//
// A screenshot only counts once the real wasm-bindgen client has mounted
// over the SSR tree (the wasm payload resource has a non-zero
// `responseEnd` and the app root is present): a raw SSR screenshot would
// grade markup the mockup's rendered surfaces never actually show a user,
// since the SSR pass alone omits anything the hydrated client paints only
// after mount (see crates/openkite-web/src/ssr.rs doc comment).
//
// Usage:
//   OPENKITE_WEB_BIN=/path/to/openkite-web \
//   OPENKITE_WEB_ROOT=/path/to/hydration-assets \
//   OPENKITE_BROWSER=/path/to/chromium \
//   node e2e/visual/browser/capture.mjs <output-dir>
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { accessSync, constants, existsSync } from 'node:fs';
import { mkdir, rm, writeFile } from 'node:fs/promises';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));

// The four surfaces OKT-191 grades. Overlays (palette/switcher) and the
// remaining desktop-only surfaces (logs/terminal) stay out of scope -- they
// are not part of this ticket's acceptance list.
const ROUTES = [
  { slug: 'home', route: '/' },
  { slug: 'cluster', route: '/cluster' },
  { slug: 'workloads', route: '/workloads' },
  { slug: 'config', route: '/config' },
];

// The three breakpoints named in the acceptance text, verbatim.
const VIEWPORTS = [
  { slug: '1440x900', width: 1440, height: 900 },
  { slug: '834x1112', width: 834, height: 1112 },
  { slug: '390x844', width: 390, height: 844 },
];

const chromium = process.env.OPENKITE_BROWSER;
const hostBin = process.env.OPENKITE_WEB_BIN;
const webRoot = process.env.OPENKITE_WEB_ROOT;
const outDir = path.resolve(process.argv[2] ?? path.join(scriptDir, 'baselines'));

function fail(message) {
  console.error(`FAIL: ${message}`);
  process.exit(1);
}

if (!chromium || !hostBin || !webRoot) {
  fail('OPENKITE_BROWSER, OPENKITE_WEB_BIN, and OPENKITE_WEB_ROOT are required');
}
if (Number(process.versions.node.split('.')[0]) < 22) {
  fail(`Node.js 22+ required for its built-in WebSocket; found ${process.version}`);
}
for (const [name, value] of [['OPENKITE_WEB_BIN', hostBin], ['OPENKITE_WEB_ROOT', webRoot], ['OPENKITE_BROWSER', chromium]]) {
  if (!path.isAbsolute(value)) fail(`${name} must be an absolute path: ${value}`);
}
if (!existsSync(hostBin)) fail(`OPENKITE_WEB_BIN does not exist: ${hostBin}`);
try {
  accessSync(hostBin, constants.X_OK);
} catch (error) {
  fail(`OPENKITE_WEB_BIN is not executable: ${hostBin}: ${error.message}`);
}
for (const asset of ['openkite-web-client.js', 'openkite-web-client_bg.wasm']) {
  if (!existsSync(path.join(webRoot, asset))) fail(`OPENKITE_WEB_ROOT missing ${asset}: ${webRoot}`);
}
if (!existsSync(chromium)) fail(`OPENKITE_BROWSER does not exist: ${chromium}`);
try {
  accessSync(chromium, constants.X_OK);
} catch (error) {
  fail(`OPENKITE_BROWSER is not executable: ${chromium}: ${error.message}`);
}

await rm(outDir, { recursive: true, force: true });
await mkdir(outDir, { recursive: true });

const hostPort = await freePort();
const hostAddr = `127.0.0.1:${hostPort}`;
const baseUrl = `http://${hostAddr}`;
const browserPort = await freePort();
const profileDir = await mkdtemp0();
const serverLog = [];
const browserLog = [];
const errors = [];
let serverProcess;
let browser;
let debugWebSocket;
let sequence = 0;
const waiting = new Map();

try {
  serverProcess = spawn(hostBin, [], {
    env: {
      ...process.env,
      OPENKITE_ADDR: hostAddr,
      OPENKITE_WEB_ROOT: webRoot,
      KUBECONFIG: path.join(scriptDir, '..', '..', 'browser', 'fixtures', 'kubeconfig'),
      KUBERNETES_SERVICE_HOST: '',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  serverProcess.stdout.on('data', (chunk) => serverLog.push(chunk.toString()));
  serverProcess.stderr.on('data', (chunk) => serverLog.push(chunk.toString()));
  serverProcess.on('error', (error) => errors.push(`web host spawn failed: ${error.message}`));

  browser = spawn(chromium, [
    '--headless',
    '--no-sandbox',
    '--disable-gpu',
    '--disable-dev-shm-usage',
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-background-networking',
    '--hide-scrollbars',
    '--remote-allow-origins=*',
    '--remote-debugging-address=127.0.0.1',
    `--remote-debugging-port=${browserPort}`,
    `--user-data-dir=${profileDir}`,
    'about:blank',
  ], { stdio: ['ignore', 'ignore', 'pipe'] });
  browser.stderr.on('data', (chunk) => browserLog.push(chunk.toString()));
  browser.on('error', (error) => errors.push(`browser spawn failed: ${error.message}`));

  await waitForHost(baseUrl, serverProcess, serverLog);
  const chromeEndpoint = `http://127.0.0.1:${browserPort}`;
  const version = await waitForJson(`${chromeEndpoint}/json/version`, browser, browserLog);
  console.log(`Browser: ${version.Browser}`);
  const tabs = await waitForJson(`${chromeEndpoint}/json/list`, browser, browserLog);
  const tab = tabs.find((item) => item.type === 'page');
  assert.ok(tab, 'Chromium DevTools has no page target');
  debugWebSocket = await connectWebSocket(tab.webSocketDebuggerUrl);
  debugWebSocket.addEventListener('message', handleMessage);
  debugWebSocket.addEventListener('message', collectEvents);
  await command('Runtime.enable');
  await command('Page.enable');

  const captured = [];
  for (const viewport of VIEWPORTS) {
    await command('Emulation.setDeviceMetricsOverride', {
      width: viewport.width,
      height: viewport.height,
      deviceScaleFactor: 1,
      mobile: viewport.width < 768,
    });
    for (const target of ROUTES) {
      if (errors.length) throw new Error(errors.join('\n'));
      await command('Page.navigate', { url: `${baseUrl}${target.route}` });
      await waitForExpression(`location.pathname === ${JSON.stringify(target.route === '/' ? '/' : target.route)}`);
      await waitForHydration();
      // Let layout settle one frame after hydration mounts.
      await delay(300);
      const { data } = await command('Page.captureScreenshot', {
        format: 'png',
        captureBeyondViewport: false,
      });
      const name = `${target.slug}-${viewport.slug}.png`;
      await writeFile(path.join(outDir, name), Buffer.from(data, 'base64'));
      captured.push(name);
      console.log(`captured ${name}`);
    }
  }

  assert.equal(errors.length, 0, errors.join('\n'));
  console.log(`PASS: captured ${captured.length} hydrated-client baselines in ${outDir}`);
} catch (error) {
  console.error(`FAIL: ${error instanceof Error ? error.stack : String(error)}`);
  console.error(`server log (tail): ${serverLog.join('').slice(-4000)}`);
  console.error(`browser log (tail): ${browserLog.join('').slice(-2000)}`);
  process.exitCode = 1;
} finally {
  debugWebSocket?.close();
  if (serverProcess && serverProcess.exitCode === null) {
    serverProcess.kill('SIGTERM');
    await waitForExit(serverProcess, 'web host');
  }
  if (browser && browser.exitCode === null) {
    browser.kill('SIGTERM');
    await waitForExit(browser, 'browser');
  }
  await rm(profileDir, { recursive: true, force: true }).catch(() => {});
}

async function waitForHydration(timeoutMs = 20000) {
  await waitForExpression(
    `document.querySelector('[data-surface="app"]') !== null && performance.getEntriesByType('resource').some(item => item.name.includes('openkite-web-client_bg.wasm') && item.responseEnd > 0)`,
    timeoutMs,
  );
}

async function evaluate(expression) {
  const result = await command('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
  assert.equal(result.exceptionDetails, undefined, `browser evaluation failed: ${JSON.stringify(result.exceptionDetails)}`);
  return result.result.value;
}

async function waitForExpression(expression, timeoutMs = 20000) {
  const end = Date.now() + timeoutMs;
  let last;
  while (Date.now() < end) {
    if (errors.length) throw new Error(errors.join('\n'));
    try {
      last = await evaluate(expression);
      if (last === true) return;
    } catch (error) {
      last = error.message;
    }
    await delay(150);
  }
  throw new Error(`timed out waiting for: ${expression}\nlast result: ${last}`);
}

async function command(method, params = {}) {
  const id = ++sequence;
  const promise = new Promise((resolve, reject) => waiting.set(id, { resolve, reject }));
  debugWebSocket.send(JSON.stringify({ id, method, params }));
  return promise;
}

function handleMessage(event) {
  let message;
  try {
    message = JSON.parse(event.data);
  } catch {
    return;
  }
  if (!message.id) return;
  const entry = waiting.get(message.id);
  if (!entry) return;
  waiting.delete(message.id);
  if (message.error) entry.reject(new Error(`${message.error.message}: ${message.error.data ?? ''}`));
  else entry.resolve(message.result ?? {});
}

function collectEvents(event) {
  let message;
  try {
    message = JSON.parse(event.data);
  } catch {
    return;
  }
  if (message.method === 'Runtime.exceptionThrown') {
    errors.push(message.params.exceptionDetails?.text ?? 'JavaScript exception');
  }
}

function waitForExit(child, label) {
  return new Promise((resolve) => {
    const timeout = setTimeout(() => {
      try {
        child.kill('SIGKILL');
      } catch {
        /* already gone */
      }
      resolve();
    }, 5000);
    child.once('exit', () => {
      clearTimeout(timeout);
      resolve();
    });
    child.once('error', () => {
      clearTimeout(timeout);
      resolve();
    });
  }).catch(() => {});
}

async function connectWebSocket(url) {
  const socket = new WebSocket(url);
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true });
    socket.addEventListener('error', reject, { once: true });
  });
  return socket;
}

async function waitForHost(url, child, logs) {
  const end = Date.now() + 15000;
  while (Date.now() < end) {
    if (child.exitCode !== null) throw new Error(`web host exited before it became ready: ${logs.join('').slice(-2000)}`);
    try {
      const response = await fetch(url, { redirect: 'manual', signal: AbortSignal.timeout(500) });
      if (response.status === 200 && (response.headers.get('content-type') ?? '').includes('text/html')) return;
    } catch {
      /* retry until the host binds */
    }
    await delay(100);
  }
  throw new Error(`web host did not become ready at ${url}: ${logs.join('').slice(-2000)}`);
}

async function waitForJson(url, child, logs) {
  const end = Date.now() + 15000;
  while (Date.now() < end) {
    if (child.exitCode !== null) throw new Error(`Chromium exited before DevTools came up: ${logs.join('').slice(-2000)}`);
    try {
      const response = await fetch(url, { signal: AbortSignal.timeout(500) });
      if (response.ok) return await response.json();
    } catch {
      /* retry until remote debugging is ready */
    }
    await delay(100);
  }
  throw new Error(`Chromium DevTools endpoint did not become ready at ${url}: ${logs.join('').slice(-2000)}`);
}

async function freePort() {
  return new Promise((resolve, reject) => {
    const socket = net.createServer();
    socket.once('error', reject);
    socket.listen(0, '127.0.0.1', () => {
      const port = socket.address().port;
      socket.close((error) => (error ? reject(error) : resolve(port)));
    });
  });
}

async function mkdtemp0() {
  const fsPromises = await import('node:fs/promises');
  return fsPromises.mkdtemp(path.join(os.tmpdir(), 'openkite-chrome-profile-'));
}
