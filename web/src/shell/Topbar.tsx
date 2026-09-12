import { Icon } from './icons'

export function Topbar({
  context,
  section,
  current,
  namespaces,
  namespace,
  onNamespace,
  onMenu,
  onRefresh,
  onOpenSettings,
}: {
  context: string | null
  section: string
  current: string
  namespaces: string[]
  namespace: string
  onNamespace: (namespace: string) => void
  onMenu: () => void
  onRefresh: () => void
  onOpenSettings: () => void
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

      <label className="namespace-filter">
        <Icon name="filter" />
        <select
          aria-label="Namespace filter"
          value={namespace}
          onChange={(event) => onNamespace(event.target.value)}
        >
          <option value="all">All namespaces</option>
          {namespaces.map((entry) => (
            <option key={entry} value={entry}>
              {entry}
            </option>
          ))}
        </select>
      </label>

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
        <button
          className="icon-btn"
          type="button"
          aria-label="Settings"
          onClick={onOpenSettings}
        >
          <Icon name="settings" />
        </button>
        <button className="avatar" type="button" aria-label="User menu">
          EK
        </button>
      </div>
    </header>
  )
}
