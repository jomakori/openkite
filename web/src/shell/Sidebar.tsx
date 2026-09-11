import { Fragment } from 'react'
import { Icon } from './icons'
import { NAV_SECTIONS, type NavItem, type NavSection } from './nav'
import { StatusBar } from './StatusBar'

interface SidebarProps {
  activeId: string
  counts: Record<string, number | undefined>
  context: string | null
  connected: boolean
  version?: string
  open: boolean
  onSelect: (id: string) => void
}

export function Sidebar({
  activeId,
  counts,
  context,
  connected,
  version,
  open,
  onSelect,
}: SidebarProps) {
  return (
    <aside className={open ? 'sidebar open' : 'sidebar'}>
      <div className="brand">
        <span className="brand-mark">
          <Icon name="kite" size={30} />
        </span>
        <span className="brand-word">
          Open<strong>Kite</strong>
        </span>
      </div>

      <button className="cluster-btn" type="button">
        <span className="status-line">
          <span className={connected ? 'dot ok' : 'dot err'} />
        </span>
        <span className="cluster-text">
          {context ?? 'no cluster'}
          <small>{connected ? 'connected' : 'disconnected'}</small>
        </span>
        <Icon name="chevron" />
      </button>

      <nav className="nav" aria-label="Resource navigation">
        {NAV_SECTIONS.map((section) => (
          <Fragment key={section.id}>
            {section.plugin ? <div className="nav-divider" /> : null}
            <NavSectionView
              section={section}
              activeId={activeId}
              counts={counts}
              onSelect={onSelect}
            />
          </Fragment>
        ))}
      </nav>

      <StatusBar version={version} connected={connected} />
    </aside>
  )
}

function NavSectionView({
  section,
  activeId,
  counts,
  onSelect,
}: {
  section: NavSection
  activeId: string
  counts: Record<string, number | undefined>
  onSelect: (id: string) => void
}) {
  return (
    <div className={section.plugin ? 'nav-section argo' : 'nav-section'}>
      <div className="nav-title">{section.label}</div>
      {section.items.map((item) => (
        <NavItemView
          key={item.id}
          item={item}
          active={item.id === activeId}
          count={item.countKind ? counts[item.countKind] : undefined}
          onSelect={onSelect}
        />
      ))}
    </div>
  )
}

function NavItemView({
  item,
  active,
  count,
  onSelect,
}: {
  item: NavItem
  active: boolean
  count?: number
  onSelect: (id: string) => void
}) {
  const classes = active ? 'nav-item active' : 'nav-item'
  return (
    <button
      type="button"
      className={classes}
      data-disabled={item.enabled ? undefined : true}
      aria-current={active ? 'page' : undefined}
      onClick={() => {
        if (item.enabled) onSelect(item.id)
      }}
    >
      <Icon name={item.icon} />
      <span>{item.label}</span>
      {count !== undefined ? <span className="nav-badge">{count}</span> : null}
    </button>
  )
}
