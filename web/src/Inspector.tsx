import type { ResourceRow } from './bridge'
import { Icon } from './shell/icons'

export interface InspectorProps {
  row: ResourceRow | null
  open: boolean
  onClose: () => void
  onViewLogs: () => void
  onToast: (message: string) => void
}

export function Inspector({ row, open, onClose, onViewLogs, onToast }: InspectorProps) {
  return (
    <>
      <button
        className={open ? 'inspector-scrim show' : 'inspector-scrim'}
        type="button"
        aria-label="Close resource details"
        onClick={onClose}
      />
      <aside
        className={open ? 'inspector open' : 'inspector'}
        aria-label="Resource details"
        aria-hidden={!open}
      >
        <div className="inspector-header">
          <Icon name="pods" />
          <h2>{row?.name ?? 'Resource details'}</h2>
          <button
            className="icon-btn"
            type="button"
            aria-label="Close resource details"
            onClick={onClose}
          >
            <Icon name="close" />
          </button>
        </div>
        <div className="inspector-body">
          <div className="inspector-eyebrow">Resource summary</div>
          <dl className="kv-list">
            <KvRow label="Namespace" value={row?.namespace} />
            <KvRow label="Status" value={row?.phase} />
            <KvRow label="Ready" value={row?.ready} />
            <KvRow label="Restarts" value={row ? String(row.restarts) : undefined} />
            <KvRow label="Controller" value={row?.controller} />
            <KvRow label="Node" value={row?.node} />
            <KvRow label="QoS" value={row?.qos} />
            <KvRow label="Age" value={row?.age} />
          </dl>
          <div className="inspector-actions">
            <button className="btn btn-secondary" type="button" onClick={onViewLogs}>
              <Icon name="logs" />
              View Logs
            </button>
            <button
              className="btn btn-primary"
              type="button"
              onClick={() => onToast('Terminal attach opens here')}
            >
              <Icon name="terminal" />
              Terminal
            </button>
          </div>
        </div>
      </aside>
    </>
  )
}

function KvRow({ label, value }: { label: string; value?: string }) {
  return (
    <div className="kv-row">
      <dt>{label}</dt>
      <dd>{value ?? '—'}</dd>
    </div>
  )
}
