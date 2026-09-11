import { StrictMode } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { App } from './App'
import { Spike, controller } from './Spike'
import './index.css'

let root: Root | null = null

/** Mount the console shell into `container` (idempotent). */
export function mount(container: HTMLElement): void {
  if (root) return
  // `?measure=1` renders the spike surface the measurement harness reads;
  // every other entry renders the console shell.
  const measure = new URLSearchParams(window.location.search).has('measure')
  root = createRoot(container)
  root.render(
    <StrictMode>{measure ? <Spike /> : <App />}</StrictMode>,
  )
}

/** Tear the React tree down (host route exit / hot swap). */
export function unmount(): void {
  root?.unmount()
  root = null
}

// Test + host surface: `window.__openkite_react_spike`.
window.__openkite_react_spike = controller

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
    mount(container)
    return
  }
  if (attempts++ < 60) requestAnimationFrame(autoMount)
}

autoMount()
