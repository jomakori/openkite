// OKT-67 spike — bridge client for the EXISTING host transport.
//
// JS -> Rust rides the host bridge that already ships in `src/plugin_api.rs`
// (`OPENKITE_BRIDGE_JS`): `window.openkite.api.*` POSTs a
// `{id, plugin, request}` envelope to the `/openkite` dioxus asset handler
// (`src/router.rs`), which dispatches to kube-rs. This spike invents no new
// transport; it calls that bridge unmodified.
//
// The one host addition is a tiny read-only `/openkite-spike` endpoint that
// answers the active kubeconfig context — there is no existing op for it.
// It uses the exact same same-origin-fetch asset-handler seam.
//
// Outside the real wry webview (Vite preview / Playwright measurement) no
// host globals exist, so `resolveBridge` installs a wire-compatible shim over
// `fetch('/openkite')`; the measurement harness fulfils both endpoints with
// `page.route`.

export interface ResourceRow {
  name: string
  namespace: string
  phase: string
  ready: string
  age: string
}

export interface ClusterContext {
  context: string | null
  connected: boolean
  version?: string
}

/** One delivery from the host's additive Rust → JS push channel. */
export interface PushUpdate {
  sub: number
  kind: string
  ns: string | null
  /**
   * The initial snapshot is the host's full kube `List` object; later pushes
   * carry a bare array of serialised objects. `countRows` normalises both.
   */
  rows: unknown
  revision: number
  initial?: boolean
}

type ApiResponse =
  | { status: 'ok'; result: unknown }
  | { status: 'error'; error: string }

export interface OpenKiteApi {
  list(kind: string, ns?: string | null): Promise<unknown>
  get(kind: string, ns: string, name: string): Promise<unknown>
  watch(kind: string, ns?: string | null): Promise<unknown>
  logs(name: string, ns: string, container?: string | null): Promise<unknown>
  exec(name: string, ns: string, container?: string | null, cmd?: string[]): Promise<unknown>
}

export interface OpenKiteBridge {
  registerSidebar(item: unknown): void
  registerRoute(route: unknown): void
  registerStatusItem(item: unknown): void
  registerRouteRenderer(path: string): void
  _renderRoute(path: string, container: HTMLElement): () => void
  /** Live updates for a kind; returns an unsubscribe fn. */
  subscribe(
    opts: string | { kind: string; ns?: string | null },
    handler: (msg: PushUpdate) => void,
  ): () => void
  _pushHandlers?: Map<number, (msg: PushUpdate) => void>
  _pushState?: (msg: PushUpdate) => void
  api: OpenKiteApi
}

declare global {
  interface Window {
    openkite?: OpenKiteBridge
    __openkite_plugin?: string | null
    __okt67ScriptStart?: number
    __openkite_react_spike?: unknown
  }
}

export type BridgeMode = 'host' | 'shim'

let nextId = 1

async function fetchJson(path: string, body: unknown): Promise<unknown> {
  const res = await fetch(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  if (!res.ok) throw new Error(`${path} HTTP ${res.status}`)
  const envelope = (await res.json()) as ApiResponse
  if (envelope.status === 'error') throw new Error(envelope.error)
  return envelope.result
}

// The host bridge's `call()` body, reproduced for wire parity.
function call(request: unknown): Promise<unknown> {
  return fetchJson('/openkite', {
    id: nextId++,
    plugin: window.__openkite_plugin || 'openkite-react-spike',
    request,
  })
}

function installShim(): OpenKiteBridge {
  const handlers = new Map<number, (msg: PushUpdate) => void>()
  const bridge: OpenKiteBridge = {
    registerSidebar: () => {},
    registerRoute: () => {},
    registerStatusItem: () => {},
    registerRouteRenderer: () => {},
    _renderRoute: () => () => {},
    _pushHandlers: handlers,
    _pushState: (msg) => {
      handlers.get(msg.sub)?.(msg)
    },
    subscribe: (opts, handler) => {
      const kind = typeof opts === 'string' ? opts : opts.kind
      const ns = typeof opts === 'string' ? null : (opts.ns ?? null)
      let cancelled = false
      void (async () => {
        try {
          const result = await bridge.api.list(kind, ns)
          if (!cancelled) {
            handler({ sub: 0, kind, ns, rows: result, revision: 0, initial: true })
          }
        } catch {
          // Unknown kind or no cluster: leave the count undefined.
        }
      })()
      return () => {
        cancelled = true
      }
    },
    api: {
      list: (kind, ns) => call({ op: 'list', kind, ns: ns ?? null }),
      get: (kind, ns, name) => call({ op: 'get', kind, ns, name }),
      watch: (kind, ns) => call({ op: 'watch', kind, ns: ns ?? null }),
      logs: (name, ns, container) =>
        call({ op: 'logs', name, ns, container: container ?? null }),
      exec: (name, ns, container, cmd) =>
        call({ op: 'exec', name, ns, container: container ?? null, cmd: cmd ?? [] }),
    },
  }
  window.openkite = bridge
  return bridge
}

/** The live host bridge inside the wry webview, else the measurement shim. */
export function resolveBridge(): { bridge: OpenKiteBridge; mode: BridgeMode } {
  const existing = window.openkite
  if (existing && existing.api && typeof existing.api.list === 'function') {
    return { bridge: existing, mode: 'host' }
  }
  return { bridge: installShim(), mode: 'shim' }
}

/** Ask the host for the active kubeconfig context (spike-only read endpoint). */
export async function fetchClusterContext(): Promise<ClusterContext> {
  try {
    const result = await fetchJson('/openkite-spike', { op: 'context' })
    return result as ClusterContext
  } catch {
    return { context: null, connected: false, version: 'dev' }
  }
}

/** Item count for either push payload shape (a kube `List` or a bare array). */
export function countRows(rows: unknown): number {
  if (Array.isArray(rows)) return rows.length
  if (rows && typeof rows === 'object' && Array.isArray((rows as { items?: unknown }).items)) {
    return (rows as { items: unknown[] }).items.length
  }
  return 0
}

/** Number of items in a kube `List` result (0 for anything else). */
export function countItems(result: unknown): number {
  if (result && typeof result === 'object' && 'items' in result) {
    const items = (result as { items?: unknown }).items
    if (Array.isArray(items)) return items.length
  }
  return 0
}

interface KubeMeta {
  name?: string
  namespace?: string
  creationTimestamp?: string
}
interface KubeObject {
  metadata?: KubeMeta
  status?: { phase?: string; containerStatuses?: Array<{ ready?: boolean }> }
}

/** Map a kube `List` of core objects into flat table rows. */
export function rowsFromList(result: unknown, now: number): ResourceRow[] {
  if (!result || typeof result !== 'object' || !('items' in result)) return []
  const items = (result as { items?: unknown }).items
  if (!Array.isArray(items)) return []
  return items.map((raw) => {
    const obj = raw as KubeObject
    const containers = obj.status?.containerStatuses ?? []
    const ready = containers.length
      ? `${containers.filter((c) => c.ready).length}/${containers.length}`
      : '—'
    return {
      name: obj.metadata?.name ?? '<unnamed>',
      namespace: obj.metadata?.namespace ?? '',
      phase: obj.status?.phase ?? 'Unknown',
      ready,
      age: formatAge(obj.metadata?.creationTimestamp, now),
    }
  })
}

function formatAge(timestamp: string | undefined, now: number): string {
  if (!timestamp) return '—'
  const created = Date.parse(timestamp)
  if (Number.isNaN(created)) return '—'
  const seconds = Math.max(0, Math.floor((now - created) / 1000))
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h`
  return `${Math.floor(hours / 24)}d`
}
