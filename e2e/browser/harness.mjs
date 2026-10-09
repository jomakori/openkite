#!/usr/bin/env node
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';

const chromium = process.env.OPENKITE_BROWSER;
const hostBin = process.env.OPENKITE_WEB_BIN;
const webRoot = process.env.OPENKITE_WEB_ROOT;
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const artifactDir = process.env.OPENKITE_ARTIFACT_DIR
  ? path.resolve(process.env.OPENKITE_ARTIFACT_DIR)
  : await mkdtemp(path.join(os.tmpdir(), 'openkite-browser-e2e-'));
await mkdir(artifactDir, { recursive: true });
const reportPath = path.join(artifactDir, 'report.json');
const ownsArtifactDir = !process.env.OPENKITE_ARTIFACT_DIR;
const profileDir = await mkdtemp(path.join(os.tmpdir(), 'openkite-chrome-profile-'));
const hostPort = await freePort();
const hostAddr = `127.0.0.1:${hostPort}`;
const baseUrl = `http://${hostAddr}`;
const browserPort = await freePort();
const errors = [];
const consoleLog = [];
const serverLog = [];
let browser;
let serverProcess;
let debugWebSocket;
let sequence = 0;
const waiting = new Map();

try {
  serverProcess = spawn(hostBin, [], {
    env: {
      ...process.env,
      OPENKITE_ADDR: hostAddr,
      OPENKITE_WEB_ROOT: webRoot,
      KUBECONFIG: path.join(scriptDir, 'fixtures', 'kubeconfig'),
      KUBERNETES_SERVICE_HOST: '',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  serverProcess.stdout.on('data', (chunk) => serverLog.push(chunk.toString()));
  serverProcess.stderr.on('data', (chunk) => serverLog.push(chunk.toString()));
  serverProcess.on('error', (error) => errors.push(`web host spawn failed: ${error.message}`));
  browser = spawn(chromium, [
    '--headless=new',
    '--no-sandbox',
    '--disable-gpu',
    '--disable-dev-shm-usage',
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-background-networking',
    '--remote-allow-origins=*',
    '--remote-debugging-address=127.0.0.1',
    `--remote-debugging-port=${browserPort}`,
    `--user-data-dir=${profileDir}`,
    'about:blank',
  ], { stdio: ['ignore', 'ignore', 'pipe'] });
  browser.stderr.on('data', (chunk) => consoleLog.push(chunk.toString()));
  browser.on('error', (error) => errors.push(`browser spawn failed: ${error.message}`));

  await waitForHost(baseUrl, serverProcess, consoleLog);
  const chromeEndpoint = `http://127.0.0.1:${browserPort}`;
  const version = await waitForJson(`${chromeEndpoint}/json/version`, browser, consoleLog);
  console.log(`Browser: ${version.Browser}`);
  const tabs = await waitForJson(`${chromeEndpoint}/json/list`, browser, consoleLog);
  const tab = tabs.find((item) => item.type === 'page');
  assert.ok(tab, 'Chromium DevTools has no page target');
  debugWebSocket = new WebSocket(tab.webSocketDebuggerUrl);
  await connectWebSocket(debugWebSocket);
  debugWebSocket.addEventListener('message', handleMessage);
  await command('Runtime.enable');
  await command('Page.enable');
  await command('Log.enable');
  await command('Network.enable');
  debugWebSocket.addEventListener('message', collectPageEvents);

  console.log(`Host: ${baseUrl}`);
  await command('Page.navigate', { url: `${baseUrl}/workloads` });
  const workloadMarker = '[data-surface="workloads"]';
  await waitForExpression(`location.pathname === '/workloads' && !!document.querySelector('${workloadMarker}')`);

  const before = await evaluate(`({
    pathname: location.pathname,
    title: document.title,
    hydrationIdCount: document.querySelectorAll('[data-node-hydration]').length,
    rootText: document.querySelector('#main')?.innerText ?? '',
    snapshot: JSON.parse(document.querySelector('#openkite-snapshot')?.textContent ?? 'null'),
    clientScript: [...document.scripts].map(script => script.src).filter(src => src.includes('openkite-web-client.js')),
    hydrationDataPresent: typeof window.initial_dioxus_hydration_data === 'string' && window.initial_dioxus_hydration_data.length > 0,
    roots: [...document.querySelectorAll('[data-surface="app"]')].length,
  })`);
  assert.ok(before.title.includes('OpenKite'), `unexpected document title: ${before.title}`);
  assert.ok(before.hydrationIdCount > 0, 'server did not render Dioxus hydration markers');
  assert.equal(before.snapshot?.route, '/workloads', 'SSR snapshot did not resolve workloads deep-link');
  assert.match(before.rootText, /Workloads/, 'SSR markup did not paint the workloads route');
  assert.ok(before.clientScript.length > 0, 'page does not load its wasm-bindgen client module');
  assert.ok(before.hydrationDataPresent, 'page omitted Dioxus hydration payload');
  assert.equal(before.roots, 1, 'SSR page should contain exactly one app root before hydration');
  console.log(`SSR assertions passed (${before.hydrationIdCount} hydration markers)`);

  await waitForExpression(`document.querySelectorAll('[data-surface="app"]').length === 1 && document.querySelector('${workloadMarker}') && performance.getEntriesByType('resource').some(item => item.name.includes('openkite-web-client_bg.wasm') && item.responseEnd > 0)`);
  const afterHydrate = await evaluate(`({
    pathname: location.pathname,
    title: document.title,
    hydrationIdCount: document.querySelectorAll('[data-node-hydration]').length,
    roots: document.querySelectorAll('[data-surface="app"]').length,
    text: document.querySelector('#main')?.innerText ?? '',
    clientReady: performance.getEntriesByType('resource').some(item => item.name.includes('openkite-web-client_bg.wasm') && item.responseEnd > 0),
    buttonCount: document.querySelectorAll('button').length,
  })`);
  assert.equal(afterHydrate.pathname, '/workloads');
  assert.equal(afterHydrate.roots, 1, 'hydration replaced or duplicated the server-rendered app');
  assert.ok(afterHydrate.hydrationIdCount > 0, 'hydration discarded the SSR tree');
  assert.match(afterHydrate.text, /Workloads/, 'hydrated root lost the deep-link route');
  assert.ok(afterHydrate.clientReady, 'real wasm client module did not fetch its wasm payload');
  assert.ok(afterHydrate.buttonCount > 0, 'expected interactive controls after hydration');
  console.log('Wasm hydration assertions passed');

  const button = await evaluate(`(() => {
    const el = document.querySelector('button[data-action="refresh"]');
    if (!el) return null;
    el.click();
    return { label: el.getAttribute('aria-label'), text: el.textContent.trim() };
  })()`);
  assert.ok(button, 'refresh control not found after hydration');
  await waitForExpression(`document.querySelector('[data-round="1"]')`);
  const refresh = await evaluate(`({ round: document.querySelector('[data-round]')?.getAttribute('data-round'), text: document.querySelector('[data-round]')?.textContent ?? '', gatewayErrors: [...document.querySelectorAll('[data-error]')].map(el => el.textContent) })`);
  assert.equal(refresh.round, '1', 'clicking refresh did not drive the hydrated state update');
  assert.deepEqual(refresh.gatewayErrors, [], `fixture gateway returned errors: ${refresh.gatewayErrors.join('; ')}`);
  console.log('Hydrated refresh interaction passed (round=1)');

  const requestReport = await evaluate(`({
    gatewayRequests: performance.getEntriesByType('resource').filter(item => item.name.endsWith('/api/gateway')).map(item => ({name: item.name, initiatorType: item.initiatorType})),
    wasm: performance.getEntriesByType('resource').filter(item => item.name.includes('openkite-web-client_bg.wasm')).map(item => item.name),
  })`);
  assert.ok(requestReport.gatewayRequests.length > 0, 'real hydrated interaction did not call same-origin /api/gateway');
  assert.ok(requestReport.wasm.length > 0, 'no wasm payload resource observed');
  assert.equal(errors.length, 0, errors.join('\n'));

  const report = {
    browser: version.Browser,
    url: `${baseUrl}/workloads`,
    ssr: { hydrationMarkers: before.hydrationIdCount, route: before.snapshot.route, appRoots: before.roots },
    hydrated: { appRoots: afterHydrate.roots, wasmLoaded: afterHydrate.clientReady, refreshRound: refresh.round },
    gatewayRequests: requestReport.gatewayRequests,
    consoleErrors: errors,
    serverLog: serverLog.join('').slice(-8000),
  };
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`PASS: real SSR + wasm hydration + interactive refresh; report ${reportPath}`);
} catch (error) {
  const report = {
    failure: error instanceof Error ? error.stack : String(error),
    consoleErrors: errors,
    serverLog: serverLog.join('').slice(-12000),
    browserLog: consoleLog.join('').slice(-4000),
  };
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`).catch((writeError) => {
    console.error(`Could not write failure report: ${writeError.message}`);
  });
  console.error(`FAIL: ${report.failure}`);
  console.error(`Artifacts: ${artifactDir}`);
  console.error(report.browserLog);
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
  await rm(profileDir, { recursive: true, force: true });
  if (ownsArtifactDir && process.exitCode === 0) await rm(artifactDir, { recursive: true, force: true });
}

async function evaluate(expression) {
  const result = await command('Runtime.evaluate', {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
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
  throw new Error(`timed out waiting for browser expression: ${expression}\nlast result: ${last}`);
}

async function command(method, params = {}) {
  const id = ++sequence;
  const promise = new Promise((resolve, reject) => waiting.set(id, { resolve, reject }));
  debugWebSocket.send(JSON.stringify({ id, method, params }));
  return promise;
}

function handleMessage(event) {
  let message;
  try { message = JSON.parse(event.data); } catch { return; }
  if (!message.id) return;
  const entry = waiting.get(message.id);
  if (!entry) return;
  waiting.delete(message.id);
  if (message.error) entry.reject(new Error(`${message.error.message}: ${message.error.data ?? ''}`));
  else entry.resolve(message.result ?? {});
}

function collectPageEvents(event) {
  let message;
  try { message = JSON.parse(event.data); } catch { return; }
  if (message.method === 'Runtime.exceptionThrown') errors.push(message.params.exceptionDetails?.text ?? 'JavaScript exception');
  if (message.method === 'Runtime.consoleAPICalled' && message.params.type === 'error') {
    errors.push(message.params.args?.map(arg => arg.value ?? arg.description ?? '').join(' ') || 'console.error');
  }
  if (message.method === 'Log.entryAdded' && message.params.entry.level === 'error') {
    errors.push(message.params.entry.text);
  }
}

function waitForExit(child, label) {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      try {
        child.kill('SIGKILL');
      } catch (error) {
        reject(new Error(`Could not kill ${label}: ${error.message}`));
        return;
      }
      resolve();
    }, 5000);
    child.once('exit', () => {
      clearTimeout(timeout);
      resolve();
    });
    child.once('error', (error) => {
      clearTimeout(timeout);
      reject(new Error(`Could not stop ${label}: ${error.message}`));
    });
  }).catch((error) => {
    console.error(error.message);
  });
}

function connectWebSocket(socket) {
  return new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true });
    socket.addEventListener('error', reject, { once: true });
  });
}

async function waitForHost(url, child, logs) {
  const end = Date.now() + 15000;
  while (Date.now() < end) {
    if (child.exitCode !== null || errors.length) {
      throw new Error(`web host exited or failed to spawn: ${logs.join('')}${errors.join('\n')}`);
    }
    try {
      const response = await fetch(url, { redirect: 'manual', signal: AbortSignal.timeout(500) });
      if (response.status === 200 && (response.headers.get('content-type') ?? '').includes('text/html')) return;
    } catch { /* retry until the host binds */ }
    await delay(100);
  }
  throw new Error(`web host did not become ready at ${url}: ${logs.join('')}`);
}

async function waitForJson(url, child, logs) {
  const end = Date.now() + 15000;
  while (Date.now() < end) {
    if (child.exitCode !== null || errors.length) {
      throw new Error(`Chromium exited or failed to spawn: ${logs.join('')}${errors.join('\n')}`);
    }
    try {
      const response = await fetch(url, { signal: AbortSignal.timeout(500) });
      if (response.ok) return await response.json();
    } catch { /* retry until remote debugging is ready */ }
    await delay(100);
  }
  throw new Error(`Chromium DevTools endpoint did not become ready at ${url}: ${logs.join('')}`);
}

async function freePort() {
  return new Promise((resolve, reject) => {
    const socket = net.createServer();
    socket.once('error', reject);
    socket.listen(0, '127.0.0.1', () => {
      const port = socket.address().port;
      socket.close((error) => error ? reject(error) : resolve(port));
    });
  });
}
