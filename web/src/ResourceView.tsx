import { useEffect, useMemo, useState } from 'react'
import type { ResourceRow } from './bridge'
import { Icon, type IconName } from './shell/icons'
import { ResourceTable } from './ResourceTable'
import {
  filterRows,
  namespaceCounts,
  pageCount,
  pageItems,
  paginate,
  sortRows,
  toggleSort,
  type SortState,
} from './table'

export interface ResourceViewProps {
  rows: ResourceRow[]
  section: string
  title: string
  noun: string
  source: string
  icon?: IconName
  selectedKey?: string | null
  onSelect: (row: ResourceRow) => void
  onToast: (message: string) => void
}

export function ResourceView({
  rows,
  section,
  title,
  noun,
  source,
  icon,
  selectedKey,
  onSelect,
  onToast,
}: ResourceViewProps) {
  const [namespace, setNamespace] = useState('all')
  const [query, setQuery] = useState('')
  const [sort, setSort] = useState<SortState>({ key: 'name', dir: 'asc' })
  const [dense, setDense] = useState(false)
  const [page, setPage] = useState(1)

  const namespaces = useMemo(() => namespaceCounts(rows), [rows])
  const filtered = useMemo(() => filterRows(rows, namespace, query), [rows, namespace, query])
  const sorted = useMemo(() => sortRows(filtered, sort.key, sort.dir), [filtered, sort])
  const pages = pageCount(sorted.length)
  const current = Math.min(page, pages)
  const visible = paginate(sorted, current)

  useEffect(() => {
    setPage(1)
  }, [namespace, query, rows])

  const singular = noun.endsWith('s') ? noun.slice(0, -1) : noun

  return (
    <>
      <div className="page-head">
        <div>
          <div className="eyebrow">{section}</div>
          <h1>{title}</h1>
          <p className="page-sub">
            Live {noun} inventory across the cluster. Select a row for details, or filter by
            namespace below.
          </p>
        </div>
        <div className="page-actions">
          <button
            className="btn btn-secondary"
            type="button"
            onClick={() => onToast('Terminal attach opens here')}
          >
            <Icon name="terminal" />
            Terminal
          </button>
          <button
            className="btn btn-primary"
            type="button"
            onClick={() => onToast(`Create ${singular} flow opens here`)}
          >
            <Icon name="plus" />
            Create {singular}
          </button>
        </div>
      </div>

      <div className="toolbar">
        <div className="chip-row" role="group" aria-label="Namespace filter">
          <button
            className={namespace === 'all' ? 'chip active' : 'chip'}
            type="button"
            onClick={() => setNamespace('all')}
          >
            All <span className="count">{rows.length}</span>
          </button>
          {namespaces.map((entry) => (
            <button
              key={entry.namespace}
              className={namespace === entry.namespace ? 'chip active' : 'chip'}
              type="button"
              onClick={() => setNamespace(entry.namespace)}
            >
              {entry.namespace} <span className="count">{entry.count}</span>
            </button>
          ))}
        </div>
        <label className="search-field">
          <Icon name="search" />
          <input
            type="search"
            placeholder={`Filter ${noun}…`}
            aria-label={`Filter ${noun}`}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
        <button
          className={dense ? 'chip active' : 'chip'}
          type="button"
          aria-pressed={dense}
          onClick={() => setDense((value) => !value)}
        >
          <Icon name="filter" />
          Compact
        </button>
      </div>

      <ResourceTable
        rows={visible}
        source={source}
        icon={icon}
        selectedKey={selectedKey}
        onSelect={onSelect}
        dense={dense}
        sortKey={sort.key}
        sortDir={sort.dir}
        onSort={(key) => setSort((state) => toggleSort(state, key))}
        footer={
          <Pagination
            page={current}
            pages={pages}
            total={sorted.length}
            shown={visible.length}
            noun={noun}
            onPage={setPage}
          />
        }
      />
    </>
  )
}

interface PaginationProps {
  page: number
  pages: number
  total: number
  shown: number
  noun: string
  onPage: (page: number) => void
}

export function Pagination({ page, pages, total, shown, noun, onPage }: PaginationProps) {
  return (
    <div className="panel-footer">
      <span>
        Showing {shown} of {total} {noun}
      </span>
      <div className="pager" aria-label="Pagination">
        {pageItems(page, pages).map((item, index) =>
          item === 'gap' ? (
            <span key={`gap-${index}`} className="pager-gap">
              …
            </span>
          ) : (
            <button
              key={item}
              type="button"
              className={item === page ? 'active' : undefined}
              aria-current={item === page ? 'page' : undefined}
              onClick={() => onPage(item)}
            >
              {item}
            </button>
          ),
        )}
        <button
          type="button"
          aria-label="Next page"
          disabled={page >= pages}
          onClick={() => onPage(Math.min(pages, page + 1))}
        >
          ›
        </button>
      </div>
    </div>
  )
}
