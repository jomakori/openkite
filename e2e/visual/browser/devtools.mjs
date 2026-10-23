// Finds the DevTools endpoint of a headless browser that was started with
// `--remote-debugging-port=0`.
//
// The browser publishes the port it bound in two independent places: the
// `DevToolsActivePort` file inside its profile directory, and a
// `DevTools listening on ws://…` line on stderr. Either one can be the only
// one available — a confined browser build writes the marker into a private
// /tmp the caller cannot read, and a wrapper script can swallow stderr — so
// both are read and whichever appears first is used.
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

const ANNOUNCEMENT = /DevTools listening on ws:\/\/(\[[^\]]+\]|[^:/\s]+):(\d+)\//g;
const DISCOVERY_TIMEOUT_MS = 60_000;

export function portFromLogs(logs) {
  const match = [...logs.join('').matchAll(ANNOUNCEMENT)].pop();
  if (!match) return null;
  const port = Number(match[2]);
  return Number.isInteger(port) && port > 0 && port < 65536 ? port : null;
}

export async function portFromMarker(profileDir) {
  try {
    const port = Number((await readFile(path.join(profileDir, 'DevToolsActivePort'), 'utf8')).split('\n')[0].trim());
    return Number.isInteger(port) && port > 0 && port < 65536 ? port : null;
  } catch {
    return null;
  }
}

export async function waitForDevTools({ profileDir, child, logs, timeoutMs = DISCOVERY_TIMEOUT_MS }) {
  const end = Date.now() + timeoutMs;
  const tail = () => logs.join('').slice(-2000);
  let port = null;
  let source = '';

  while (Date.now() < end) {
    if (child.exitCode !== null) {
      throw new Error(`browser exited (${child.exitCode}) before DevTools came up: ${tail()}`);
    }
    if (port === null) {
      const fromMarker = await portFromMarker(profileDir);
      if (fromMarker !== null) {
        port = fromMarker;
        source = 'profile marker';
      } else {
        const fromLogs = portFromLogs(logs);
        if (fromLogs !== null) {
          port = fromLogs;
          source = 'stderr announcement';
        }
      }
    }
    if (port !== null) {
      for (const base of [`http://127.0.0.1:${port}`, `http://[::1]:${port}`]) {
        try {
          const response = await fetch(`${base}/json/version`, { signal: AbortSignal.timeout(500) });
          if (response.ok) return { endpoint: base, source };
        } catch {
          /* the other loopback family, or not listening yet */
        }
      }
    }
    await delay(100);
  }

  if (port === null) {
    throw new Error(
      `browser announced no DevTools port within ${timeoutMs}ms ` +
        `(no ${path.join(profileDir, 'DevToolsActivePort')} and no "DevTools listening on …" line): ${tail()}`,
    );
  }
  throw new Error(`browser announced DevTools on port ${port} via its ${source}, but nothing answered within ${timeoutMs}ms: ${tail()}`);
}
