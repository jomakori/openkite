import { useSyncExternalStore } from 'react'
import { Shell } from './Shell'
import { getConsoleRoute, subscribeConsoleRoute } from './consoleRoute'

export function App() {
  // The host owns the URL; the console mirrors the nav item it maps to.
  const route = useSyncExternalStore(
    subscribeConsoleRoute,
    getConsoleRoute,
    getConsoleRoute,
  )
  return <Shell route={route} />
}
