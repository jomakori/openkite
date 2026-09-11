import { useEffect, useState } from 'react'
import { resolveBridge, rowsFromPayload, type ResourceRow } from '../bridge'

/**
 * Live rows for a kind through the bridge's subscribe channel, with the
 * static/browser fixture fallback handled inside the shim. `nonce` forces a
 * fresh subscription for the topbar and pull-to-refresh affordances.
 */
export function useResourceRows(kind: string, nonce: number): ResourceRow[] {
  const [rows, setRows] = useState<ResourceRow[]>([])

  useEffect(() => {
    const bridge = resolveBridge().bridge
    let cancelled = false
    let unsubscribe: (() => void) | null = null
    try {
      unsubscribe = bridge.subscribe({ kind, ns: null }, (msg) => {
        if (cancelled) return
        setRows(rowsFromPayload(msg.rows, Date.now()))
      })
    } catch {
      // A kind the bridge cannot resolve leaves the table empty.
    }
    return () => {
      cancelled = true
      unsubscribe?.()
    }
  }, [kind, nonce])

  return rows
}
