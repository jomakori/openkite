import { useState } from 'react'
import { Sidebar } from './shell/Sidebar'
import { Topbar } from './shell/Topbar'
import { findNavItem } from './shell/nav'
import { useClusterContext, useLiveCounts } from './shell/useLiveCounts'

export function Shell() {
  const [activeId, setActiveId] = useState('pods')
  const [sidebarOpen, setSidebarOpen] = useState(false)
  const { counts, refresh } = useLiveCounts()
  const context = useClusterContext()

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
