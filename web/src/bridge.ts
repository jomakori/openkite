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

import { FIXTURE_CONTEXT, fixtureCall, type FixtureRequest } from './fixtures'

export interface ResourceRow {
  name: string
  namespace: string
  phase: string
  ready: string
  age: string
  /** Age in seconds, so the Age column sorts numerically. */
  ageSeconds: number
  /** Per-container readiness, rendered as the render's health dots. */
  health: boolean[]
  restarts: number
  controller: string
  node: string
  qos: string
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

/**
 * Transport-level failure: no host asset handler, non-2xx, or a non-JSON
 * reply (a static host's SPA fallback answers `index.html`). Distinct from a
 * bridge-envelope error, which means the host IS present and answered.
 */
class BridgeTransportError extends Error {}

let nextId = 1

async function fetchJson(path: string, body: unknown): Promise<unknown> {
  let res: Response
  try {
    res = await fetch(path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    })
  } catch (err) {
    throw new BridgeTransportError(`${path} unreachable: ${String(err)}`)
  }
  if (!res.ok) throw new BridgeTransportError(`${path} HTTP ${res.status}`)
  let envelope: ApiResponse
  try {
    envelope = (await res.json()) as ApiResponse
  } catch {
    throw new BridgeTransportError(`${path} returned a non-JSON body`)
  }
  if (envelope.status === 'error') throw new Error(envelope.error)
  return envelope.result
}

// The host bridge's `call()` body, reproduced for wire parity. When the host
// transport is absent (static/staging host) the shim falls back to fixtures so
// the UI renders content instead of empty states. A bridge-envelope error is a
// real host answer and propagates unchanged.
function call(request: FixtureRequest): Promise<unknown> {
  return fetchJson('/openkite', {
    id: nextId++,
    plugin: window.__openkite_plugin || 'openkite-react-spike',
    request,
  }).catch((err: unknown) => {
    if (err instanceof BridgeTransportError) return fixtureCall(request)
    throw err
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

/** Marks the shim so a second `resolveBridge()` does not mistake it for a host. */
const SHIM_MARK = Symbol('openkite.shim')

let installed: { bridge: OpenKiteBridge; mode: BridgeMode } | null = null

function isHostBridge(bridge: OpenKiteBridge | undefined): bridge is OpenKiteBridge {
  return Boolean(
    bridge &&
      bridge.api &&
      typeof bridge.api.list === 'function' &&
      (bridge as unknown as Record<symbol, unknown>)[SHIM_MARK] !== true,
  )
}

/**
 * The live host bridge inside the wry webview, else the shim over
 * `fetch('/openkite')` (measurement harness) with a fixture fallback
 * (static/staging host). Cached so repeated calls return the same shim.
 */
export function resolveBridge(): { bridge: OpenKiteBridge; mode: BridgeMode } {
  const existing = window.openkite
  if (isHostBridge(existing)) {
    return { bridge: existing, mode: 'host' }
  }
  if (installed) return installed
  const bridge = installShim()
  ;(bridge as unknown as Record<symbol, unknown>)[SHIM_MARK] = true
  installed = { bridge, mode: 'shim' }
  return installed
}

/**
 * Ask the host for the active kubeconfig context (spike-only read endpoint).
 *
 * Never rejects: a static/staging host has no `/openkite-spike` handler, so a
 * transport failure serves [`FIXTURE_CONTEXT`] (content, not an empty shell);
 * a host error preserves the disconnected shell rather than inventing a
 * cluster.
 */
export async function fetchClusterContext(): Promise<ClusterContext> {
  try {
    const result = await fetchJson('/openkite-spike', { op: 'context' })
    return result as ClusterContext
  } catch (err) {
    if (err instanceof BridgeTransportError) return FIXTURE_CONTEXT
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

interface KubeOwnerReference {
  kind?: string
  name?: string
}
interface KubeMeta {
  name?: string
  namespace?: string
  creationTimestamp?: string
  ownerReferences?: KubeOwnerReference[]
}
interface KubeContainerStatus {
  ready?: boolean
  restartCount?: number
}
interface KubeObject {
  metadata?: KubeMeta
  spec?: { nodeName?: string }
  status?: {
    phase?: string
    containerStatuses?: KubeContainerStatus[]
    qosClass?: string
  }
}

/** Map one kube object into a flat table row (shared by list and push paths). */
export function rowFromObject(obj: KubeObject, now: number): ResourceRow {
  const containers = obj.status?.containerStatuses ?? []
  const health = containers.map((container) => container.ready === true)
  const ready = containers.length
    ? `${health.filter(Boolean).length}/${containers.length}`
    : '—'
  const owner = obj.metadata?.ownerReferences?.[0]
  const controller = owner?.kind && owner.name ? `${owner.kind}/${owner.name}` : '—'
  return {
    name: obj.metadata?.name ?? '<unnamed>',
    namespace: obj.metadata?.namespace ?? '',
    phase: obj.status?.phase ?? 'Unknown',
    ready,
    age: formatAge(obj.metadata?.creationTimestamp, now),
    ageSeconds: ageSeconds(obj.metadata?.creationTimestamp, now),
    health,
    restarts: containers.reduce((sum, container) => sum + (container.restartCount ?? 0), 0),
    controller,
    node: obj.spec?.nodeName ?? '—',
    qos: obj.status?.qosClass ?? '—',
  }
}

/** Map a kube `List` of core objects into flat table rows. */
export function rowsFromList(result: unknown, now: number): ResourceRow[] {
  if (!result || typeof result !== 'object' || !('items' in result)) return []
  const items = (result as { items?: unknown }).items
  if (!Array.isArray(items)) return []
  return items.map((raw) => rowFromObject(raw as KubeObject, now))
}

/**
 * Rows for either push payload shape: the initial kube `List` object or a
 * later bare array of serialised objects (see {@link PushUpdate}).
 */
export function rowsFromPayload(payload: unknown, now: number): ResourceRow[] {
  if (Array.isArray(payload)) return payload.map((raw) => rowFromObject(raw as KubeObject, now))
  return rowsFromList(payload, now)
}

function ageSeconds(timestamp: string | undefined, now: number): number {
  if (!timestamp) return Number.MAX_SAFE_INTEGER
  const created = Date.parse(timestamp)
  if (Number.isNaN(created)) return Number.MAX_SAFE_INTEGER
  return Math.max(0, Math.floor((now - created) / 1000))
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
