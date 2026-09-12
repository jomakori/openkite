import type { ResourceRow } from './bridge'
import type { ResourceEvent } from './events'
import { LogLineList } from './LogDock'
import type { LogLine } from './logs'
import { Icon } from './shell/icons'

export type DetailTab = 'details' | 'events' | 'logs'

const TABS: Array<{ id: DetailTab; label: string }> = [
  { id: 'details', label: 'Details' },
  { id: 'events', label: 'Events' },
  { id: 'logs', label: 'Logs' },
]

export interface InspectorProps {
  row: ResourceRow | null
  kind: string
  open: boolean
  tab: DetailTab
  onTab: (tab: DetailTab) => void
  onClose: () => void
  onViewLogs: () => void
  onViewYaml: (row: ResourceRow) => void
  onToast: (message: string) => void
  lines: LogLine[]
  events: ResourceEvent[]
}

export function Inspector({
  row,
  kind,
  open,
  tab,
  onTab,
  onClose,
  onViewLogs,
  onViewYaml,
  onToast,
  lines,
  events,
}: InspectorProps) {
  const logsAvailable = kind === 'pods'
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

        <div className="inspector-tabs" role="tablist" aria-label="Resource detail tabs">
          {TABS.map((entry) => (
            <button
              key={entry.id}
              type="button"
              role="tab"
              aria-selected={tab === entry.id}
              className={tab === entry.id ? 'inspector-tab active' : 'inspector-tab'}
              onClick={() => onTab(entry.id)}
            >
              {entry.label}
            </button>
          ))}
        </div>

        <div className="inspector-body">
          {tab === 'details' && row ? (
            <>
              <div className="inspector-eyebrow">Resource summary</div>
              <dl className="kv-list">
                <KvRow label="Namespace" value={row.namespace} />
                <KvRow label="Status" value={row.phase} />
                <KvRow label="Ready" value={row.ready} />
                <KvRow label="Restarts" value={String(row.restarts)} />
                <KvRow label="Controller" value={row.controller} />
                <KvRow label="Node" value={row.node} />
                <KvRow label="QoS" value={row.qos} />
                <KvRow label="Age" value={row.age} />
              </dl>
              <div className="inspector-actions">
                <button
                  className="btn btn-secondary"
                  type="button"
                  onClick={() => onViewYaml(row)}
                >
                  <Icon name="config" />
                  View YAML
                </button>
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
            </>
          ) : null}

          {tab === 'details' && !row ? (
            <div className="detail-empty">Select a resource to see its details.</div>
          ) : null}

          {tab === 'events' ? (
            events.length === 0 ? (
              <div className="detail-empty">No events for this resource.</div>
            ) : (
              <ul className="event-list">
                {events.map((event) => (
                  <li key={`${event.name}-${event.reason}`} className="event-row">
                    <div className="event-head">
                      <span className={`event-type ${event.type.toLowerCase()}`}>{event.type}</span>
                      <span className="event-reason">{event.reason}</span>
                      <span className="event-count">×{event.count}</span>
                      <span className="event-age">{event.age}</span>
                    </div>
                    <div className="event-message">{event.message}</div>
                  </li>
                ))}
              </ul>
            )
          ) : null}

          {tab === 'logs' ? (
            logsAvailable ? (
              <div className="inspector-logs">
                <LogLineList lines={lines} />
              </div>
            ) : (
              <div className="detail-empty">Logs are available for pods only.</div>
            )
          ) : null}
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
