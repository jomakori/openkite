import { useEffect, useRef, useState, type TouchEvent } from 'react'
import type { ClusterContext, ResourceRow } from './bridge'
import { Inspector } from './Inspector'
import { LogDock } from './LogDock'
import { ResourceView } from './ResourceView'
import { rowKey } from './ResourceTable'
import { BottomNav } from './shell/BottomNav'
import { PullIndicator } from './shell/PullIndicator'
import { Sidebar } from './shell/Sidebar'
import { ToastProvider, useToast } from './shell/Toast'
import { Topbar } from './shell/Topbar'
import { findNavItem } from './shell/nav'
import { useClusterContext, useLiveCounts } from './shell/useLiveCounts'
import { useLogLines } from './shell/useLogLines'
import { useResourceRows } from './shell/useResourceRows'

interface ShellProps {
  /**
   * The nav id the host booted the console onto (e.g. `pods` for
   * `/workloads`, `configmaps` for `/config`). Defaults to `pods`.
   */
  route?: string
  /**
   * Pre-resolved count/context for SSR and parity tests. When omitted the
   * shell loads them from the live bridge (the desktop path).
   */
  initialCounts?: Record<string, number | undefined>
  initialContext?: ClusterContext
}

export function Shell({ route, initialCounts, initialContext }: ShellProps = {}) {
  return (
    <ToastProvider>
      <ShellChrome
        route={route}
        initialCounts={initialCounts}
        initialContext={initialContext}
      />
    </ToastProvider>
  )
}

function ShellChrome({ route, initialCounts, initialContext }: ShellProps) {
  const toast = useToast()
  const [activeId, setActiveId] = useState(route ?? 'pods')
  const [sidebarOpen, setSidebarOpen] = useState(false)
  const [selected, setSelected] = useState<ResourceRow | null>(null)
  const [logOpen, setLogOpen] = useState(true)
  const [logsPaused, setLogsPaused] = useState(false)
  const [logsCollapsed, setLogsCollapsed] = useState(false)
  const [cleared, setCleared] = useState(false)
  const [rowNonce, setRowNonce] = useState(0)
  const [pull, setPull] = useState<{ show: boolean; text: string }>({ show: false, text: '' })
  const touchStart = useRef<number | null>(null)

  const live = useLiveCounts()
  const liveContext = useClusterContext()
  const counts = initialCounts ?? live.counts
  const context = initialContext ?? liveContext

  const found = findNavItem(activeId)
  const section = found?.section.label ?? ''
  const title = found?.item.label ?? ''
  const kind = found?.item.countKind ?? found?.item.id ?? 'pods'
  const activeSection = found?.section.id ?? ''
  const rows = useResourceRows(kind, rowNonce)
  const logTarget = selected ?? rows[0] ?? null
  const logLines = useLogLines(logTarget, rowNonce)

  const refresh = () => {
    live.refresh()
    setRowNonce((value) => value + 1)
    toast.show('Resources refreshed')
  }

  const selectNav = (id: string) => {
    setActiveId(id)
    setSelected(null)
  }

  // The host can re-point the console (native palette navigation, deep link)
  // without remounting it; follow that here instead of resetting on every
  // render, so an in-console nav click is not undone by an unrelated update.
  useEffect(() => {
    if (route) {
      setActiveId(route)
      setSelected(null)
    }
  }, [route])

  useEffect(() => {
    setCleared(false)
  }, [logTarget?.name, logTarget?.namespace])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      setSidebarOpen(false)
      setSelected(null)
      setLogOpen(false)
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [])

  const onTouchStart = (event: TouchEvent<HTMLDivElement>) => {
    if (event.currentTarget.scrollTop <= 0) touchStart.current = event.touches[0].clientY
  }
  const onTouchMove = (event: TouchEvent<HTMLDivElement>) => {
    if (touchStart.current === null) return
    const delta = event.touches[0].clientY - touchStart.current
    if (delta > 24 && event.currentTarget.scrollTop <= 0) {
      setPull({ show: true, text: delta > 90 ? 'Release to refresh' : 'Pull to refresh' })
    }
  }
  const onTouchEnd = () => {
    if (pull.show) {
      setPull({ show: true, text: 'Release to refresh' })
      refresh()
      setTimeout(() => setPull({ show: false, text: '' }), 600)
    }
    touchStart.current = null
  }

  return (
    <div className="app">
      <Sidebar
        activeId={activeId}
        counts={counts}
        context={context.context}
        connected={context.connected}
        version={context.version}
        open={sidebarOpen}
        onSelect={(id) => {
          selectNav(id)
          setSidebarOpen(false)
        }}
      />

      <button
        className={sidebarOpen ? 'sidebar-backdrop show' : 'sidebar-backdrop'}
        type="button"
        aria-label="Close navigation"
        onClick={() => setSidebarOpen(false)}
      />

      <div
        className="main"
        onTouchStart={onTouchStart}
        onTouchMove={onTouchMove}
        onTouchEnd={onTouchEnd}
      >
        <Topbar
          context={context.context}
          section={section}
          current={title}
          onMenu={() => setSidebarOpen(true)}
          onRefresh={refresh}
        />
        <PullIndicator show={pull.show} text={pull.text} />

        <section className="view active">
          {found?.item.enabled ? (
            <ResourceView
              rows={rows}
              section={section}
              title={title}
              noun={kind}
              source={`openkite.api.list(${kind})`}
              icon={found.item.icon}
              selectedKey={selected ? rowKey(selected) : null}
              onSelect={setSelected}
              onToast={toast.show}
            />
          ) : (
            <div className="page-head">
              <div>
                <div className="eyebrow">{section}</div>
                <h1>{title}</h1>
              </div>
            </div>
          )}

          {found?.item.enabled ? (
            <LogDock
              open={logOpen}
              paused={logsPaused}
              collapsed={logsCollapsed}
              pod={logTarget?.name ?? 'no pod selected'}
              lines={cleared ? [] : logLines}
              onTogglePause={() => setLogsPaused((value) => !value)}
              onToggleCollapse={() => setLogsCollapsed((value) => !value)}
              onClear={() => setCleared(true)}
              onClose={() => setLogOpen(false)}
            />
          ) : null}
        </section>
      </div>

      <BottomNav
        activeSection={activeSection}
        logOpen={logOpen}
        onSelect={selectNav}
        onToggleLogs={() => setLogOpen((value) => !value)}
        onMenu={() => setSidebarOpen(true)}
      />

      <Inspector
        row={selected}
        open={selected !== null}
        onClose={() => setSelected(null)}
        onViewLogs={() => setLogOpen(true)}
        onToast={toast.show}
      />
    </div>
  )
}
