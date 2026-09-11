// Host-route → console-nav bridge.
//
// The React console owns its own navigation: clicking a sidebar item swaps
// the view without touching the host URL. The one thing it cannot know on
// its own is *which* nav item the host booted it onto — the Dioxus shell
// decides that from the URL and passes it down as a `data-console-route`
// attribute plus a `setRoute` call when the route changes.
//
// This module is the tiny store in between: the host writes a nav id, React
// reads it through `useSyncExternalStore` (App.tsx) and re-seeds the shell.
// It lives outside `mount.tsx` so the store and the mount entry do not
// import each other.

let current = 'pods'

const listeners = new Set<() => void>()

/** The nav id the console should currently display. */
export function getConsoleRoute(): string {
  return current
}

/**
 * Point the console at a nav id. No-op for an empty or unchanged id so the
 * host can call it on every render without churning React.
 */
export function setConsoleRoute(route: string): void {
  if (!route || route === current) return
  current = route
  for (const listener of listeners) listener()
}

/** Subscribe to route changes; returns an unsubscribe fn. */
export function subscribeConsoleRoute(listener: () => void): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}
