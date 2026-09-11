import type { ResourceRow } from './bridge'

export type SortDir = 'asc' | 'desc'
export type SortKey =
  | 'name'
  | 'namespace'
  | 'phase'
  | 'restarts'
  | 'controller'
  | 'node'
  | 'qos'
  | 'age'

export interface SortState {
  key: SortKey
  dir: SortDir
}

export interface TableColumn {
  key: SortKey | null
  label: string
}

export const SORT_COLUMNS: TableColumn[] = [
  { key: 'name', label: 'Name' },
  { key: 'namespace', label: 'Namespace' },
  { key: null, label: 'Health' },
  { key: 'restarts', label: 'Restarts' },
  { key: 'controller', label: 'Controller' },
  { key: 'node', label: 'Node' },
  { key: 'qos', label: 'QoS' },
  { key: 'age', label: 'Age' },
  { key: 'phase', label: 'Status' },
]

export const PAGE_SIZE = 8

export type Tone = 'success' | 'warn' | 'danger' | 'neutral'

/** Map a kube phase/state string onto the render's pill tone vocabulary. */
export function statusTone(status: string): Tone {
  const value = status.trim().toLowerCase()
  if (!value || value === '—' || value === 'unknown') return 'neutral'
  if (/(running|succeeded|active|synced|healthy|ready|bound|completed)/.test(value)) {
    return 'success'
  }
  if (/(pending|progressing|outofsync|warning|terminating|creating|init|suspended)/.test(value)) {
    return 'warn'
  }
  if (/(failed|crashloopbackoff|error|degraded|evicted|notready|unhealthy|erroring)/.test(value)) {
    return 'danger'
  }
  return 'neutral'
}

/** Dot modifier inside a status pill, matching the render's health dots. */
export function toneDot(tone: Tone): 'ok' | 'warn' | 'err' | null {
  if (tone === 'success') return 'ok'
  if (tone === 'warn') return 'warn'
  if (tone === 'danger') return 'err'
  return null
}

/** Click a sortable header: same column flips direction, new column ascends. */
export function toggleSort(current: SortState, key: SortKey): SortState {
  if (current.key === key) {
    return { key, dir: current.dir === 'asc' ? 'desc' : 'asc' }
  }
  return { key, dir: 'asc' }
}

/** Namespace/text filter shared by the toolbar chips and search field. */
export function filterRows(rows: ResourceRow[], namespace: string, query: string): ResourceRow[] {
  const needle = query.trim().toLowerCase()
  return rows.filter((row) => {
    if (namespace !== 'all' && row.namespace !== namespace) return false
    if (!needle) return true
    return `${row.name} ${row.namespace} ${row.phase}`.toLowerCase().includes(needle)
  })
}

/** Stable sort over the table's sortable columns. */
export function sortRows(rows: ResourceRow[], key: SortKey, dir: SortDir): ResourceRow[] {
  const factor = dir === 'asc' ? 1 : -1
  const value = (row: ResourceRow): number | string => {
    switch (key) {
      case 'restarts':
        return row.restarts
      case 'age':
        return row.ageSeconds
      case 'namespace':
        return row.namespace.toLowerCase()
      case 'phase':
        return row.phase.toLowerCase()
      case 'controller':
        return row.controller.toLowerCase()
      case 'node':
        return row.node.toLowerCase()
      case 'qos':
        return row.qos.toLowerCase()
      case 'name':
      default:
        return row.name.toLowerCase()
    }
  }
  return [...rows].sort((a, b) => {
    const left = value(a)
    const right = value(b)
    if (typeof left === 'number' && typeof right === 'number') {
      return (left - right) * factor
    }
    return String(left).localeCompare(String(right)) * factor
  })
}

/** Namespace chips with per-namespace counts, sorted alphabetically. */
export function namespaceCounts(rows: ResourceRow[]): Array<{ namespace: string; count: number }> {
  const counts = new Map<string, number>()
  for (const row of rows) {
    if (!row.namespace) continue
    counts.set(row.namespace, (counts.get(row.namespace) ?? 0) + 1)
  }
  return [...counts.entries()]
    .map(([namespace, count]) => ({ namespace, count }))
    .sort((a, b) => a.namespace.localeCompare(b.namespace))
}

export function pageCount(total: number, size: number = PAGE_SIZE): number {
  return Math.max(1, Math.ceil(total / size))
}

export function paginate(rows: ResourceRow[], page: number, size: number = PAGE_SIZE): ResourceRow[] {
  const start = (page - 1) * size
  return rows.slice(start, start + size)
}

/** Windowed page numbers for a pager (first/last always, gaps as 'gap'). */
export function pageItems(page: number, pages: number): Array<number | 'gap'> {
  const items: Array<number | 'gap'> = []
  const push = (item: number | 'gap') => {
    if (!items.includes(item)) items.push(item)
  }
  push(1)
  if (page - 1 > 2) push('gap')
  for (let n = Math.max(2, page - 1); n <= Math.min(pages - 1, page + 1); n += 1) {
    push(n)
  }
  if (pages - page > 2) push('gap')
  if (pages > 1) push(pages)
  return items
}
