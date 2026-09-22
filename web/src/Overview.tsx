import { useState, type ReactNode } from 'react'
import type { ResourceRow } from './bridge'
import type { NavViewProps } from './Shell'
import { Icon } from './shell/icons'
import { NAV_SECTIONS } from './shell/nav'
import { useResourceRows } from './shell/useResourceRows'
import { toneDot, type Tone } from './table'

/** Every nav item that carries a live badge, in nav order. */
const COUNTED_ITEMS: Array<{ kind: string; label: string }> = NAV_SECTIONS.flatMap((section) =>
  section.items.flatMap((item) =>
    item.countKind ? [{ kind: item.countKind, label: item.label }] : [],
  ),
)

/** Pod phases a pod leaves behind once it has finished starting. */
const SETTLED_PHASES = new Set(['Running', 'Succeeded'])

/** Pod phases the kubelet is retrying or has given up on. */
const FAILING_PHASES = new Set(['CrashLoopBackOff', 'Error', 'Failed'])

export interface ClusterHealth {
  tone: Tone
  label: string
  running: number
  total: number
  restarts: number
}

/**
 * Pod health for the cluster summary: `neutral` until the bridge delivers its
 * first snapshot, `danger` when a pod is failing, `warn` while pods are still
 * starting.
 */
export function clusterHealth(pods: ResourceRow[]): ClusterHealth {
  const total = pods.length
  const restarts = pods.reduce((sum, pod) => sum + pod.restarts, 0)
  if (total === 0) {
    return { tone: 'neutral', label: 'Waiting for pod data', running: 0, total, restarts }
  }
  const running = pods.filter((pod) => SETTLED_PHASES.has(pod.phase)).length
  if (pods.some((pod) => FAILING_PHASES.has(pod.phase))) {
    return { tone: 'danger', label: 'Degraded', running, total, restarts }
  }
  if (running < total) {
    return { tone: 'warn', label: 'Starting', running, total, restarts }
  }
  return { tone: 'success', label: 'Healthy', running, total, restarts }
}

/**
 * Cluster summary: the count behind every nav badge, the connection the topbar
 * reports, and pod health. Not a table — counts come from the shell's watches
 * and the health rows from a cluster-wide pod watch.
 */
export function Overview({ section, title, counts, context, onRefresh }: NavViewProps) {
  const [nonce, setNonce] = useState(0)
  const pods = useResourceRows('pods', 'all', nonce)
  const health = clusterHealth(pods)
  const healthDot = toneDot(health.tone)

  const refresh = () => {
    setNonce((value) => value + 1)
    onRefresh()
  }

  return (
    <>
      <div className="page-head">
        <div>
          <div className="eyebrow">{section}</div>
          <h1>{title}</h1>
          <p className="page-sub">
            Cluster-wide inventory, connection state and pod health, read from the console's
            live watches.
          </p>
        </div>
        <div className="page-actions">
          <button className="btn btn-secondary" type="button" onClick={refresh}>
            <Icon name="refresh" />
            Refresh
          </button>
        </div>
      </div>

      <div className="toolbar">
        <div className="chip-row" role="group" aria-label="Resource counts">
          {COUNTED_ITEMS.map((item) => (
            <span className="chip" key={item.kind}>
              {item.label} <span className="count">{counts[item.kind] ?? '—'}</span>
            </span>
          ))}
        </div>
      </div>

      <div className="inspector-eyebrow">Connection &amp; health</div>
      <dl className="kv-list">
        <SummaryRow label="Context">{context.context ?? 'no cluster'}</SummaryRow>
        <SummaryRow label="Version">{context.version ?? 'unknown'}</SummaryRow>
        <SummaryRow label="Connection">
          <span className={context.connected ? 'pill success' : 'pill danger'}>
            <span className={context.connected ? 'dot ok' : 'dot err'} />
            {context.connected ? 'Connected' : 'Disconnected'}
          </span>
        </SummaryRow>
        <SummaryRow label="Mutations">{context.mutations ? 'Enabled' : 'Read-only'}</SummaryRow>
        <SummaryRow label="Health">
          <span className={health.tone === 'neutral' ? 'pill' : `pill ${health.tone}`}>
            {healthDot ? <span className={`dot ${healthDot}`} /> : null}
            {health.label}
          </span>
        </SummaryRow>
        <SummaryRow label="Pods">
          {health.total === 0 ? '—' : `${health.running}/${health.total} running`}
        </SummaryRow>
        <SummaryRow label="Restarts">
          <span className={health.restarts > 0 ? 'restarts warn' : 'restarts'}>
            {health.restarts}
          </span>
        </SummaryRow>
      </dl>
    </>
  )
}

function SummaryRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="kv-row">
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  )
}
