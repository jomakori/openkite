import { Icon } from './icons'

export interface BottomNavProps {
  activeSection: string
  logOpen: boolean
  onSelect: (id: string) => void
  onToggleLogs: () => void
  onMenu: () => void
}

export function BottomNav({
  activeSection,
  logOpen,
  onSelect,
  onToggleLogs,
  onMenu,
}: BottomNavProps) {
  const workloadsActive = activeSection === 'workloads'
  const argoActive = activeSection === 'argo'

  return (
    <nav className="bottom-nav" aria-label="Mobile navigation">
      <div className="bottom-tabs">
        <button
          className={workloadsActive ? 'bottom-tab active' : 'bottom-tab'}
          type="button"
          onClick={() => onSelect('pods')}
        >
          <Icon name="workloads" />
          <span>Workloads</span>
        </button>
        <button
          className={argoActive ? 'bottom-tab argo active' : 'bottom-tab argo'}
          type="button"
          onClick={() => onSelect('applications')}
        >
          <Icon name="argo" />
          <span>Argo CD</span>
        </button>
        <button
          className={logOpen ? 'bottom-tab active' : 'bottom-tab'}
          type="button"
          aria-expanded={logOpen}
          onClick={onToggleLogs}
        >
          <Icon name="logs" />
          <span>Logs</span>
        </button>
        <button className="bottom-tab" type="button" aria-label="Open navigation" onClick={onMenu}>
          <Icon name="menu" />
          <span>Menu</span>
        </button>
      </div>
    </nav>
  )
}
