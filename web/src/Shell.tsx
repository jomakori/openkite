import { useState } from 'react'
import type { ClusterContext } from './bridge'
import { Sidebar } from './shell/Sidebar'
import { Topbar } from './shell/Topbar'
import { findNavItem } from './shell/nav'
import { useClusterContext, useLiveCounts } from './shell/useLiveCounts'

interface ShellProps {
  /**
   * Pre-resolved count/context for SSR and parity tests. When omitted the
   * shell loads them from the live bridge (the desktop path).
   */
  initialCounts?: Record<string, number | undefined>
  initialContext?: ClusterContext
}

export function Shell({ initialCounts, initialContext }: ShellProps = {}) {
  const [activeId, setActiveId] = useState('pods')
  const [sidebarOpen, setSidebarOpen] = useState(false)
  const live = useLiveCounts()
  const liveContext = useClusterContext()
  const counts = initialCounts ?? live.counts
  const refresh = live.refresh
  const context = initialContext ?? liveContext

  const found = findNavItem(activeId)
  const section = found?.section.label ?? ''
  const title = found?.item.label ?? ''

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
          setActiveId(id)
          setSidebarOpen(false)
        }}
      />

      <button
        className={sidebarOpen ? 'sidebar-backdrop show' : 'sidebar-backdrop'}
        type="button"
        aria-label="Close navigation"
        onClick={() => setSidebarOpen(false)}
      />

      <div className="main">
        <Topbar
          context={context.context}
          section={section}
          current={title}
          onMenu={() => setSidebarOpen(true)}
          onRefresh={refresh}
        />

        <section className="view active">
          <div className="page-head">
            <div>
              <div className="eyebrow">{section}</div>
              <h1>{title}</h1>
            </div>
          </div>
        </section>
      </div>
    </div>
  )
}
