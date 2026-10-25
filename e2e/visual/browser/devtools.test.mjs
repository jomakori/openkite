import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { portFromLogs, waitForDevTools } from './devtools.mjs';

async function devtoolsServer(t, body = () => ({ Browser: 'stub/1.0' })) {
  const server = http.createServer((request, response) => {
    if (request.url === '/json/version') {
      response.writeHead(200, { 'content-type': 'application/json' });
      response.end(JSON.stringify(body()));
      return;
    }
    response.writeHead(404).end();
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const { port } = server.address();
  t.after(() => new Promise((resolve) => server.close(resolve)));
  return port;
}

async function profile(t) {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'openkite-devtools-test-'));
  t.after(() => rm(dir, { recursive: true, force: true }));
  return dir;
}

function stubBrowser(t, source) {
  const child = spawn(process.execPath, ['-e', source], { stdio: ['ignore', 'pipe', 'pipe'] });
  const logs = [];
  child.stdout.on('data', (chunk) => logs.push(chunk.toString()));
  child.stderr.on('data', (chunk) => logs.push(chunk.toString()));
  t.after(() => {
    if (child.exitCode === null) child.kill('SIGKILL');
  });
  return { child, logs };
}

test('portFromLogs takes the last announcement out of noisy stderr', () => {
  const logs = [
    '[123:456:ERROR:dbus] Failed to connect to the bus\n',
    'DevTools listening on ws://127.0.0.1:41001/devtools/browser/aaaa\n',
    '[123:456:ERROR:dbus] Retrying\n',
    'DevTools listening on ws://127.0.0.1:41002/devtools/browser/bbbb\n',
  ];
  assert.equal(portFromLogs(logs), 41002);
});

test('portFromLogs returns null when the browser announced nothing', () => {
  assert.equal(portFromLogs(['[123:456:ERROR:dbus] Failed to connect to the bus\n']), null);
  assert.equal(portFromLogs([]), null);
});

test('discovery reads the port from the profile marker', async (t) => {
  const port = await devtoolsServer(t);
  const dir = await profile(t);
  await writeFile(path.join(dir, 'DevToolsActivePort'), `${port}\n/devtools/browser/aaaa\n`);
  const { child, logs } = stubBrowser(t, 'setTimeout(() => {}, 60000)');

  const found = await waitForDevTools({ profileDir: dir, child, logs, timeoutMs: 5000 });
  assert.deepEqual(found, { endpoint: `http://127.0.0.1:${port}`, source: 'profile marker' });
});

test('discovery falls back to the stderr announcement when the marker is absent', async (t) => {
  const port = await devtoolsServer(t);
  const dir = await profile(t);
  const { child, logs } = stubBrowser(
    t,
    `console.error('DevTools listening on ws://127.0.0.1:${port}/devtools/browser/bbbb'); setTimeout(() => {}, 60000)`,
  );

  const found = await waitForDevTools({ profileDir: dir, child, logs, timeoutMs: 5000 });
  assert.deepEqual(found, { endpoint: `http://127.0.0.1:${port}`, source: 'stderr announcement' });
});

test('discovery fails fast when the browser exited', async (t) => {
  const dir = await profile(t);
  const { child, logs } = stubBrowser(t, 'process.exit(3)');
  await new Promise((resolve) => child.once('exit', resolve));

  await assert.rejects(
    waitForDevTools({ profileDir: dir, child, logs, timeoutMs: 5000 }),
    /browser exited \(3\) before DevTools came up/,
  );
});

test('discovery reports both empty sources on timeout', async (t) => {
  const dir = await profile(t);
  const { child, logs } = stubBrowser(t, 'setTimeout(() => {}, 60000)');

  await assert.rejects(
    waitForDevTools({ profileDir: dir, child, logs, timeoutMs: 400 }),
    /announced no DevTools port within 400ms.*no "DevTools listening on/,
  );
});
