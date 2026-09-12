import { useState, type ReactNode } from 'react'
import type {
  AppSettings,
  MenuBarSetting,
  SettingsSnapshot,
  TitleBarThemeSetting,
} from '../settings'
import { Icon } from './icons'

export type CategoryId = 'appearance' | 'kubernetes' | 'terminal' | 'application'

const CATEGORIES: Array<{ id: CategoryId; label: string }> = [
  { id: 'appearance', label: 'Appearance' },
  { id: 'kubernetes', label: 'Kubernetes' },
  { id: 'terminal', label: 'Terminal' },
  { id: 'application', label: 'Application' },
]

const FONT_SIZES = [11, 12, 13, 14, 15, 16, 17, 18]
const TITLE_BAR_THEMES: TitleBarThemeSetting[] = ['system', 'light', 'dark']

export interface SettingsProps {
  open: boolean
  snapshot: SettingsSnapshot
  context: string | null
  onClose: () => void
  onChange: (patch: Partial<AppSettings>) => void
  initialCategory?: CategoryId
}

export function Settings({
  open,
  snapshot,
  context,
  onClose,
  onChange,
  initialCategory,
}: SettingsProps) {
  const [category, setCategory] = useState<CategoryId>(initialCategory ?? 'appearance')
  if (!open) return null

  const active = CATEGORIES.find((entry) => entry.id === category) ?? CATEGORIES[0]

  return (
    <div className="settings-backdrop" role="presentation" onClick={onClose}>
      <div
        className="settings-modal"
        role="dialog"
        aria-label="Settings"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="settings-sidebar">
          <div className="settings-title">Settings</div>
          <nav className="settings-nav" aria-label="Settings categories">
            {CATEGORIES.map((entry) => (
              <button
                key={entry.id}
                type="button"
                className={entry.id === category ? 'settings-nav-item active' : 'settings-nav-item'}
                aria-current={entry.id === category ? 'page' : undefined}
                onClick={() => setCategory(entry.id)}
              >
                {entry.label}
              </button>
            ))}
          </nav>
        </div>
        <div className="settings-panel">
          <div className="settings-header">
            <h2>{active.label}</h2>
            <button
              className="icon-btn"
              type="button"
              aria-label="Close settings"
              onClick={onClose}
            >
              <Icon name="close" />
            </button>
          </div>
          <div className="settings-body">
            {category === 'appearance' ? (
              <AppearancePanel snapshot={snapshot} onChange={onChange} />
            ) : null}
            {category === 'kubernetes' ? (
              <KubernetesPanel
                snapshot={snapshot}
                context={context}
                onChange={onChange}
              />
            ) : null}
            {category === 'terminal' ? <TerminalPanel snapshot={snapshot} /> : null}
            {category === 'application' ? (
              <ApplicationPanel snapshot={snapshot} onChange={onChange} />
            ) : null}
          </div>
        </div>
      </div>
    </div>
  )
}

function SettingRow({
  label,
  hint,
  children,
}: {
  label: string
  hint?: string
  children: ReactNode
}) {
  return (
    <div className="setting-row">
      <div className="setting-copy">
        <div className="setting-label">{label}</div>
        {hint ? <div className="setting-hint">{hint}</div> : null}
      </div>
      <div className="setting-control">{children}</div>
    </div>
  )
}

function AppearancePanel({
  snapshot,
  onChange,
}: {
  snapshot: SettingsSnapshot
  onChange: (patch: Partial<AppSettings>) => void
}) {
  const themeOptions = [{ id: 'default', name: 'Default (SilkCircuit Neon)' }].concat(
    snapshot.themes.filter((theme) => theme.id !== 'default'),
  )
  return (
    <>
      <SettingRow label="Theme" hint="The app's opaline color theme.">
        <select
          className="settings-select"
          aria-label="Theme"
          value={snapshot.theme ?? 'default'}
          onChange={(event) =>
            onChange({ theme: event.target.value === 'default' ? null : event.target.value })
          }
        >
          {themeOptions.map((theme) => (
            <option key={theme.id} value={theme.id}>
              {theme.name}
            </option>
          ))}
        </select>
      </SettingRow>
      <SettingRow label="Font size" hint="Base UI font size in pixels.">
        <select
          className="settings-select"
          aria-label="Font size"
          value={snapshot.fontSize ?? ''}
          onChange={(event) =>
            onChange({ fontSize: event.target.value ? Number(event.target.value) : null })
          }
        >
          <option value="">Default</option>
          {FONT_SIZES.map((size) => (
            <option key={size} value={size}>
              {size}px
            </option>
          ))}
        </select>
      </SettingRow>
      <SettingRow
        label="Title bar theme"
        hint={
          snapshot.titleBarOverridable
            ? 'OS window decoration theme, independent of the app theme.'
            : 'This platform controls its own title bar.'
        }
      >
        <div className="segmented" role="group" aria-label="Title bar theme">
          {TITLE_BAR_THEMES.map((theme) => (
            <button
              key={theme}
              type="button"
              className={snapshot.titleBarTheme === theme ? 'segment active' : 'segment'}
              disabled={!snapshot.titleBarOverridable}
              onClick={() => onChange({ titleBarTheme: theme })}
            >
              {theme}
            </button>
          ))}
        </div>
      </SettingRow>
    </>
  )
}

function KubernetesPanel({
  snapshot,
  context,
  onChange,
}: {
  snapshot: SettingsSnapshot
  context: string | null
  onChange: (patch: Partial<AppSettings>) => void
}) {
  return (
    <>
      <SettingRow
        label="Metrics columns"
        hint="Show CPU/memory columns in the native resource views."
      >
        <input
          type="checkbox"
          className="settings-checkbox"
          aria-label="Metrics columns"
          checked={snapshot.metricsEnabled}
          onChange={(event) => onChange({ metricsEnabled: event.target.checked })}
        />
      </SettingRow>
      <SettingRow label="Current context" hint="The active kubeconfig context.">
        <span className="settings-value">{context ?? 'no cluster'}</span>
      </SettingRow>
    </>
  )
}

function TerminalPanel({ snapshot }: { snapshot: SettingsSnapshot }) {
  return (
    <>
      <SettingRow label="Default shell" hint="Resolved from $SHELL at launch.">
        <span className="settings-value">$SHELL</span>
      </SettingRow>
      <SettingRow label="Terminal font" hint="Terminals inherit the base UI font size.">
        <span className="settings-value">
          {snapshot.fontSize ? `${snapshot.fontSize}px` : 'Default'}
        </span>
      </SettingRow>
    </>
  )
}

function ApplicationPanel({
  snapshot,
  onChange,
}: {
  snapshot: SettingsSnapshot
  onChange: (patch: Partial<AppSettings>) => void
}) {
  const menuBarOptions: MenuBarSetting[] = ['show', 'hide']
  return (
    <>
      <SettingRow
        label="Menu bar"
        hint={
          snapshot.menuBarHideable
            ? 'Hide or show the OS window menu bar.'
            : 'This platform does not allow hiding its menu bar.'
        }
      >
        <select
          className="settings-select"
          aria-label="Menu bar"
          value={snapshot.menuBar}
          disabled={!snapshot.menuBarHideable}
          title={snapshot.menuBarHideable ? undefined : 'Not supported on this platform'}
          onChange={(event) => onChange({ menuBar: event.target.value as MenuBarSetting })}
        >
          {menuBarOptions.map((option) => (
            <option key={option} value={option}>
              {option === 'show' ? 'Show' : 'Hide'}
            </option>
          ))}
        </select>
      </SettingRow>
      <SettingRow label="Version">
        <span className="settings-value">{snapshot.version}</span>
      </SettingRow>
    </>
  )
}
