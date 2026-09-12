// Persisted app settings surface.
//
// Reads/writes the host's `OpenKiteConfig` through the `/openkite-spike`
// settings ops, mirroring the field names of `src/config.rs`. In a static
// browser host (no asset handler) the same values round-trip through
// `localStorage` so the preferences dialog and the parity render still work.
import { BridgeTransportError, spikeRequest } from './bridge'

export type MenuBarSetting = 'show' | 'hide'
export type TitleBarThemeSetting = 'system' | 'light' | 'dark'

export interface ThemeOption {
  id: string
  name: string
  variant: string
  isDefault: boolean
}

export interface Capabilities {
  mutations: boolean
  logs: boolean
  events: boolean
}

/** The persisted fields the settings dialog edits. */
export interface AppSettings {
  theme: string | null
  fontSize: number | null
  metricsEnabled: boolean
  menuBar: MenuBarSetting
  titleBarTheme: TitleBarThemeSetting
}

/** Persisted settings plus the derived metadata the surface renders. */
export interface SettingsSnapshot extends AppSettings {
  themeVars: Record<string, string>
  themes: ThemeOption[]
  menuBarHideable: boolean
  titleBarOverridable: boolean
  version: string
  capabilities: Capabilities
}

export const FALLBACK_THEMES: ThemeOption[] = [
  { id: 'default', name: 'Default (SilkCircuit Neon)', variant: 'dark', isDefault: true },
  { id: 'one-light', name: 'One Light', variant: 'light', isDefault: true },
  { id: 'one-dark', name: 'One Dark', variant: 'dark', isDefault: true },
  { id: 'catppuccin-mocha', name: 'Catppuccin Mocha', variant: 'dark', isDefault: true },
  { id: 'tokyo-night', name: 'Tokyo Night', variant: 'dark', isDefault: true },
  { id: 'rose-pine', name: 'Rosé Pine', variant: 'dark', isDefault: true },
]

export const DEFAULT_SETTINGS: SettingsSnapshot = {
  theme: null,
  fontSize: null,
  metricsEnabled: true,
  menuBar: 'show',
  titleBarTheme: 'system',
  themeVars: {},
  themes: FALLBACK_THEMES,
  menuBarHideable: true,
  titleBarOverridable: true,
  version: 'dev',
  capabilities: { mutations: false, logs: true, events: true },
}

const STORAGE_KEY = 'openkite.settings'

/** Map the host's opaline variable contract onto the console's own tokens. */
export function consoleThemeTokens(vars: Record<string, string>): Record<string, string> {
  const map: Record<string, string> = {
    '--bg': '--bg-0',
    '--surface': '--bg-1',
    '--surface-solid': '--bg-1',
    '--fg': '--fg-0',
    '--muted': '--fg-1',
    '--subtle': '--fg-2',
    '--border': '--border',
    '--accent': '--accent',
    '--progress': '--accent',
    '--success': '--green',
    '--warn': '--yellow',
    '--danger': '--red',
    '--violet': '--violet',
    '--terminal-bg': '--bg-2',
    '--terminal-fg': '--fg-0',
    '--log-info': '--term-cyan',
    '--log-method': '--term-blue',
    '--log-error': '--term-red',
  }
  const out: Record<string, string> = {}
  for (const [token, source] of Object.entries(map)) {
    const value = vars[source]
    if (value) out[token] = value
  }
  return out
}

function browserGet(): AppSettings {
  if (typeof localStorage === 'undefined') return DEFAULT_SETTINGS
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return DEFAULT_SETTINGS
    const parsed = JSON.parse(raw) as Partial<AppSettings>
    return {
      theme: parsed.theme ?? DEFAULT_SETTINGS.theme,
      fontSize: parsed.fontSize ?? DEFAULT_SETTINGS.fontSize,
      metricsEnabled: parsed.metricsEnabled ?? DEFAULT_SETTINGS.metricsEnabled,
      menuBar: parsed.menuBar ?? DEFAULT_SETTINGS.menuBar,
      titleBarTheme: parsed.titleBarTheme ?? DEFAULT_SETTINGS.titleBarTheme,
    }
  } catch {
    return DEFAULT_SETTINGS
  }
}

function browserSet(settings: AppSettings): void {
  if (typeof localStorage === 'undefined') return
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
  } catch {
    // Storage disabled/full: the in-memory value is still applied this session.
  }
}

function withFallback(settings: AppSettings): SettingsSnapshot {
  return { ...DEFAULT_SETTINGS, ...settings, themes: FALLBACK_THEMES }
}

function asSettings(value: unknown): AppSettings {
  const candidate = (value ?? {}) as Partial<AppSettings>
  return {
    theme: candidate.theme ?? null,
    fontSize: candidate.fontSize ?? null,
    metricsEnabled: candidate.metricsEnabled ?? true,
    menuBar: candidate.menuBar ?? 'show',
    titleBarTheme: candidate.titleBarTheme ?? 'system',
  }
}

/** Load persisted settings; browser hosts fall back to `localStorage`. */
export async function fetchSettings(): Promise<SettingsSnapshot> {
  try {
    const result = await spikeRequest({ op: 'settings_get' })
    return { ...DEFAULT_SETTINGS, ...(result as Partial<SettingsSnapshot>) }
  } catch (err) {
    if (err instanceof BridgeTransportError) return withFallback(browserGet())
    throw err
  }
}

/** Persist the settings; returns the host's refreshed snapshot. */
export async function saveSettings(settings: AppSettings): Promise<SettingsSnapshot> {
  try {
    const result = await spikeRequest({ op: 'settings_set', ...settings })
    return { ...DEFAULT_SETTINGS, ...(result as Partial<SettingsSnapshot>) }
  } catch (err) {
    if (err instanceof BridgeTransportError) {
      browserSet(settings)
      return withFallback(settings)
    }
    throw err
  }
}

/** Apply theme vars + font size to the console root; no-op without a DOM. */
export function applySettingsToDom(snapshot: SettingsSnapshot): void {
  if (typeof document === 'undefined') return
  const root = document.getElementById('openkite-react-spike-root')
  if (!root) return
  for (const [token, value] of Object.entries(consoleThemeTokens(snapshot.themeVars))) {
    root.style.setProperty(token, value)
  }
  root.style.fontSize = snapshot.fontSize ? `${snapshot.fontSize}px` : ''
}

export { asSettings }
