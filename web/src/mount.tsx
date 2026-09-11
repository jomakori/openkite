import { StrictMode } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { App } from './App'
import { Spike, controller } from './Spike'
import { getConsoleRoute, setConsoleRoute, subscribeConsoleRoute } from './consoleRoute'
import './index.css'

/** Host-facing mount API, evaluated under `window.__openkite_react_console`. */
export interface ConsoleApi {
  mount: (container: HTMLElement, route?: string) => void
  unmount: () => void
  setRoute: (route: string) => void
}

declare global {
  interface Window {
    /**
     * The live React root. Kept on `window`, not in module scope: the host
     * re-evaluates this bundle when the console remounts on a route change,
     * which resets module state — the root would otherwise leak into a
     * detached container with no way to unmount it.
     */
    __openkite_react_root?: Root | null
    __openkite_react_container?: HTMLElement | null
    __openkite_react_console?: ConsoleApi
  }
}

/**
 * Mount the console into `container` (idempotent per container). A different
 * container means the host re-rendered the mount point; the stale root is
 * torn down first. `route` seeds the shell's active nav item.
 */
export function mount(container: HTMLElement, route?: string): void {
  const current = window.__openkite_react_root ?? null
  if (current && window.__openkite_react_container === container) {
    setConsoleRoute(route ?? getConsoleRoute())
    return
  }
  if (current) current.unmount()

  setConsoleRoute(route ?? getConsoleRoute())

  const root = createRoot(container)
  window.__openkite_react_root = root
  window.__openkite_react_container = container
  // `?measure=1` renders the spike measurement surface; every other entry
  // renders the console shell.
  const measure = new URLSearchParams(window.location.search).has('measure')
  root.render(<StrictMode>{measure ? <Spike /> : <App />}</StrictMode>)
}

/** Tear the React tree down (host route exit). */
export function unmount(): void {
  window.__openkite_react_root?.unmount()
  window.__openkite_react_root = null
  window.__openkite_react_container = null
}

// Test + host surface: `window.__openkite_react_spike` drives the measurement
// harness, `window.__openkite_react_console` drives normal console mounting.
window.__openkite_react_spike = controller
window.__openkite_react_console = {
  mount,
  unmount,
  setRoute: setConsoleRoute,
}

// Auto-mount when the host injected its container inside the wry webview
// (`#openkite-react-spike-root`) or when running under Vite (`#root`). The
// host evaluates this bundle from a `use_effect`, which can race the Dioxus
// DOM mutation that inserts the container — retry a few frames before giving
// up, so a miss degrades to "no UI" rather than a hard error.
const HOST_CONTAINER = 'openkite-react-spike-root'
let attempts = 0

function autoMount(): void {
  const container =
    document.getElementById(HOST_CONTAINER) ?? document.getElementById('root')
  if (container) {
    mount(container, container.dataset.consoleRoute || getConsoleRoute())
    return
  }
  if (attempts++ < 60) requestAnimationFrame(autoMount)
}

autoMount()

// Re-export for any host/test consumer that imports the API directly.
export { getConsoleRoute, setConsoleRoute, subscribeConsoleRoute }
