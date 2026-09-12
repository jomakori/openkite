import { useEffect, useState } from 'react'
import { resolveBridge, rowsFromPayload, type ResourceRow } from '../bridge'

/**
 * Live rows for a kind through the bridge's subscribe channel, scoped to
 * `namespace` (`all` = every namespace the client can see). The host filters
 * server-side; the shim serves fixtures for the same `ns`. `nonce` forces a
 * fresh subscription for the topbar and pull-to-refresh affordances.
 */
export function useResourceRows(kind: string, namespace: string, nonce: number): ResourceRow[] {
  const [rows, setRows] = useState<ResourceRow[]>([])

  useEffect(() => {
    const bridge = resolveBridge().bridge
    let cancelled = false
    let unsubscribe: (() => void) | null = null
    const ns = namespace === 'all' ? null : namespace
    try {
      unsubscribe = bridge.subscribe({ kind, ns }, (msg) => {
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
  }, [kind, namespace, nonce])

  return rows
}
