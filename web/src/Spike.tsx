import { useCallback, useEffect, useRef, useState } from 'react'
import { flushSync } from 'react-dom'
import {
  fetchClusterContext,
  resolveBridge,
  rowsFromList,
  type BridgeMode,
  type ClusterContext,
  type ResourceRow,
} from './bridge'
import { ResourceTable } from './ResourceTable'

/**
 * Measurement surface read from the page by `measure/measure.mjs`.
 * Every timing is taken in-page so it describes the real JS path, not a
 * harness approximation.
 */
export interface SpikeController {
  ready: boolean
  loaded: boolean
  bridgeMode: BridgeMode | null
  /** performance.now() when the bundle began evaluating. */
  bundleStartMs: number
  /** performance.now() at the first post-mount effect. */
  mountMs: number | null
  /** payload-free first paint, relative to bundleStartMs. */
  firstPaintMs: number | null
  /** first paint WITH bridge data, relative to bundleStartMs. */
  loadedMs: number | null
  contextMs: number | null
  listMs: number | null
  renderMs: number | null
  renderCount: number
  rowCount: number
  context: ClusterContext | null
  error: string | null
  /** Commit `rows` synchronously and resolve once the browser has painted. */
  renderRows: (rows: ResourceRow[]) => Promise<number>
  reload: () => Promise<void>
}

const bundleStart = () => window.__okt67ScriptStart ?? performance.now()

export const controller: SpikeController = {
  ready: false,
  loaded: false,
  bridgeMode: null,
  bundleStartMs: bundleStart(),
  mountMs: null,
  firstPaintMs: null,
  loadedMs: null,
  contextMs: null,
  listMs: null,
  renderMs: null,
  renderCount: 0,
  rowCount: 0,
  context: null,
  error: null,
  renderRows: async () => {
    throw new Error('spike not mounted')
  },
  reload: async () => {
    throw new Error('spike not mounted')
  },
}

export function Spike() {
  const [rows, setRows] = useState<ResourceRow[]>([])
  const [context, setContext] = useState<ClusterContext | null>(null)
  const [mode, setMode] = useState<BridgeMode | null>(null)
  const [listMs, setListMs] = useState<number | null>(null)
  const [contextMs, setContextMs] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)
  const renderCount = useRef(0)
  renderCount.current += 1

  const applyRows = useCallback((next: ResourceRow[]): Promise<number> => {
    const t0 = performance.now()
    flushSync(() => setRows(next))
    controller.rowCount = next.length
    return new Promise((resolve) => {
      requestAnimationFrame(() => {
        const ms = performance.now() - t0
        controller.renderMs = ms
        resolve(ms)
      })
    })
  }, [])

  const load = useCallback(async () => {
    const { mode: resolved } = resolveBridge()
    setMode(resolved)
    controller.bridgeMode = resolved
    try {
      const t0 = performance.now()
      const ctx = await fetchClusterContext()
      const ms = performance.now() - t0
      setContext(ctx)
      setContextMs(ms)
      controller.context = ctx
      controller.contextMs = ms
    } catch (err) {
      setError(String(err))
    }
    try {
      const bridge = window.openkite
      if (!bridge) throw new Error('bridge unavailable')
      const t0 = performance.now()
      const result = await bridge.api.list('pods', 'default')
      const ms = performance.now() - t0
      setListMs(ms)
      controller.listMs = ms
      const next = rowsFromList(result, Date.now())
      await applyRows(next)
      controller.loadedMs = performance.now() - controller.bundleStartMs
      controller.loaded = true
    } catch (err) {
      setError(String(err))
    }
  }, [applyRows])

  useEffect(() => {
    void load()
  }, [load])

  useEffect(() => {
    controller.renderRows = applyRows
    controller.reload = load
    controller.mountMs = performance.now()
    requestAnimationFrame(() => {
      controller.firstPaintMs = performance.now() - controller.bundleStartMs
      controller.ready = true
    })
  }, [applyRows, load])

  useEffect(() => {
    controller.renderCount = renderCount.current
  }, [rows])

  return (
    <div data-testid="spike-root" className="mx-auto max-w-3xl p-6 font-sans">
      <header className="mb-4 flex items-baseline justify-between">
        <div>
          <h1 className="text-lg font-semibold text-slate-100">
            React 19 + Tailwind 4 · phase&nbsp;3 spike
          </h1>
          <p className="text-xs text-slate-500">
            mounted in the existing wry webview over the existing /openkite bridge
          </p>
        </div>
        <span
          data-testid="bridge-mode"
          className="rounded-full border border-slate-700 px-2 py-0.5 text-xs text-slate-300"
        >
          bridge: {mode ?? '…'}
        </span>
      </header>

      <section className="mb-4 rounded-lg border border-slate-800 bg-slate-900/60 p-4">
        <div className="text-xs uppercase tracking-wide text-slate-500">Cluster context</div>
        <div
          data-testid="cluster-label"
          className="mt-1 text-xl font-semibold text-sky-300"
        >
          {context?.context ?? 'no cluster connected'}
        </div>
        <div className="text-xs text-slate-500">
          connected: {context ? String(context.connected) : '—'}
          {contextMs !== null && ` · context round-trip: ${contextMs.toFixed(2)} ms`}
        </div>
      </section>

      <ResourceTable rows={rows} source="openkite.api.list(pods, default)" />

      <footer className="mt-4 flex flex-wrap items-center gap-3 text-sm">
        <button
          type="button"
          data-testid="reload"
          onClick={() => {
            void load()
          }}
          className="rounded-md bg-sky-600 px-3 py-1.5 font-medium text-white hover:bg-sky-500 active:bg-sky-700"
        >
          Reload via bridge (JS → Rust)
        </button>
        <span data-testid="list-ms" className="text-slate-400">
          {listMs === null ? 'list: —' : `list: ${listMs.toFixed(2)} ms`}
        </span>
        <span className="text-slate-600">renders: {renderCount.current}</span>
        <span className="text-slate-600">rows: {rows.length}</span>
        {error && (
          <span data-testid="error" className="text-rose-400">
            {error}
          </span>
        )}
      </footer>
    </div>
  )
}
