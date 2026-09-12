import type { ReactNode } from 'react'
import { rowActions, type RowActionId } from './actions'
import type { ResourceRow } from './bridge'
import { RowActionsMenu } from './ResourceActions'
import type { Capabilities } from './settings'
import { Icon, type IconName } from './shell/icons'
import { SORT_COLUMNS, statusTone, toneDot, type SortDir, type SortKey } from './table'

export interface ResourceTableProps {
  rows: ResourceRow[]
  source?: string
  icon?: IconName
  kind?: string
  selectedKey?: string | null
  onSelect?: (row: ResourceRow) => void
  onAction?: (row: ResourceRow, action: RowActionId) => void
  capabilities?: Capabilities
  dense?: boolean
  sortKey?: SortKey
  sortDir?: SortDir
  onSort?: (key: SortKey) => void
  footer?: ReactNode
}

export function rowKey(row: ResourceRow): string {
  return `${row.namespace}/${row.name}`
}

export function StatusPill({ phase }: { phase: string }) {
  const tone = statusTone(phase)
  const dot = toneDot(tone)
  return (
    <span className={tone === 'neutral' ? 'pill' : `pill ${tone}`}>
      {dot ? <span className={`dot ${dot}`} /> : null}
      {phase}
    </span>
  )
}

export function ResourceTable({
  rows,
  source,
  icon = 'pods',
  kind = 'pods',
  selectedKey,
  onSelect,
  onAction,
  capabilities,
  dense,
  sortKey,
  sortDir,
  onSort,
  footer,
}: ResourceTableProps) {
  const caps = capabilities ?? { mutations: false, logs: true, events: true }
  return (
    <div className="panel">
      <div className="table-wrap">
        <table
          data-testid="resource-table"
          className={dense ? 'resource-table compact' : 'resource-table'}
        >
          <thead>
            <tr>
              {SORT_COLUMNS.map((column) => {
                const active = column.key !== null && column.key === sortKey
                return (
                  <th
                    key={column.label}
                    aria-sort={
                      active ? (sortDir === 'desc' ? 'descending' : 'ascending') : undefined
                    }
                  >
                    {column.key ? (
                      <button
                        type="button"
                        className="th-sort"
                        onClick={() => onSort?.(column.key as SortKey)}
                      >
                        {column.label}
                        {active ? (
                          <span className="sort-mark">{sortDir === 'desc' ? '↓' : '↑'}</span>
                        ) : null}
                      </button>
                    ) : (
                      column.label
                    )}
                  </th>
                )
              })}
              <th className="actions-head" aria-label="Actions" />
            </tr>
          </thead>
          <tbody>
            {rows.length === 0 ? (
              <tr>
                <td
                  data-testid="empty-state"
                  className="empty-state"
                  colSpan={SORT_COLUMNS.length + 1}
                >
                  waiting for bridge data{source ? ` (${source})` : ''}
                </td>
              </tr>
            ) : (
              rows.map((row) => {
                const key = rowKey(row)
                const selected = selectedKey === key
                return (
                  <tr
                    key={key}
                    data-testid="resource-row"
                    tabIndex={0}
                    className={selected ? 'selected' : undefined}
                    aria-selected={selected || undefined}
                    onClick={() => onSelect?.(row)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault()
                        onSelect?.(row)
                      }
                    }}
                  >
                    <td data-label="Name">
                      <span className="cell-value resource-name">
                        <Icon name={icon} />
                        {row.name}
                      </span>
                    </td>
                    <td data-label="Namespace">
                      <span className="cell-value namespace">{row.namespace || '—'}</span>
                    </td>
                    <td data-label="Health">
                      <span className="cell-value health-dots">
                        {row.health.length ? (
                          row.health.map((ok, index) => (
                            <span key={index} className={ok ? 'dot ok' : 'dot err'} />
                          ))
                        ) : (
                          <span className="qos">—</span>
                        )}
                      </span>
                    </td>
                    <td data-label="Restarts">
                      <span
                        className={
                          row.restarts > 0 ? 'cell-value restarts warn' : 'cell-value restarts'
                        }
                      >
                        {row.restarts}
                      </span>
                    </td>
                    <td data-label="Controller">
                      <span className="cell-value controller">{row.controller}</span>
                    </td>
                    <td data-label="Node">
                      <span className="cell-value node">{row.node}</span>
                    </td>
                    <td data-label="QoS">
                      <span className="cell-value qos">{row.qos}</span>
                    </td>
                    <td data-label="Age">
                      <span className="cell-value qos">{row.age}</span>
                    </td>
                    <td data-label="Status">
                      <span className="cell-value">
                        <StatusPill phase={row.phase} />
                      </span>
                    </td>
                    <td
                      data-label="Actions"
                      className="row-actions"
                      onClick={(event) => event.stopPropagation()}
                    >
                      <RowActionsMenu
                        actions={rowActions(kind, row, caps)}
                        onAction={(action) => onAction?.(row, action)}
                      />
                    </td>
                  </tr>
                )
              })
            )}
          </tbody>
        </table>
      </div>
      {footer}
    </div>
  )
}
