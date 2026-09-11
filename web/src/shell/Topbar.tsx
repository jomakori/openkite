import { Icon } from './icons'

export function Topbar({
  context,
  section,
  current,
  onMenu,
  onRefresh,
}: {
  context: string | null
  section: string
  current: string
  onMenu: () => void
  onRefresh: () => void
}) {
  return (
    <header className="topbar">
      <button
        className="icon-btn menu-toggle"
        type="button"
        aria-label="Open navigation"
        onClick={onMenu}
      >
        <Icon name="menu" />
      </button>

      <div className="breadcrumbs">
        <span>{context ?? 'no cluster'}</span>
        <Icon name="chevron" />
        <span>{section}</span>
        <Icon name="chevron" />
        <span className="current">{current}</span>
      </div>

      <div className="topbar-actions">
        <button className="icon-btn" type="button" aria-label="Search">
          <Icon name="search" />
        </button>
        <button
          className="icon-btn"
          type="button"
          aria-label="Refresh resources"
          onClick={onRefresh}
        >
          <Icon name="refresh" />
        </button>
        <button className="icon-btn" type="button" aria-label="Settings">
          <Icon name="settings" />
        </button>
        <button className="avatar" type="button" aria-label="User menu">
          EK
        </button>
      </div>
    </header>
  )
}
