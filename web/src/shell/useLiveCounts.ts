import { useCallback, useEffect, useState } from 'react'
import {
  countRows,
  fetchClusterContext,
  resolveBridge,
  type ClusterContext,
} from '../bridge'
import { COUNTED_KINDS } from './nav'

export interface LiveCounts {
  counts: Record<string, number | undefined>
  refresh: () => void
}

export function useClusterContext(): ClusterContext {
  const [context, setContext] = useState<ClusterContext>({
    context: null,
    connected: false,
    version: 'dev',
  })

  useEffect(() => {
    let cancelled = false
    void fetchClusterContext().then((next) => {
      if (!cancelled) setContext(next)
    })
    return () => {
      cancelled = true
    }
  }, [])

  return context
}

export function useLiveCounts(): LiveCounts {
  const [counts, setCounts] = useState<Record<string, number | undefined>>({})
  const [nonce, setNonce] = useState(0)
  const refresh = useCallback(() => setNonce((current) => current + 1), [])

  useEffect(() => {
    const { bridge } = resolveBridge()
    if (typeof bridge.subscribe !== 'function') return

    const unsubscribes: Array<() => void> = []
    const revisions: Record<string, number> = {}

    for (const kind of COUNTED_KINDS) {
      try {
        const unsubscribe = bridge.subscribe({ kind, ns: null }, (msg) => {
          if (typeof msg.revision === 'number' && msg.revision > 0) {
            const previous = revisions[kind] ?? -1
            if (msg.revision <= previous) return
            revisions[kind] = msg.revision
          }
          const next = countRows(msg.rows)
          setCounts((current) =>
            current[kind] === next ? current : { ...current, [kind]: next },
          )
        })
        unsubscribes.push(unsubscribe)
      } catch {
        // A kind the bridge cannot resolve simply keeps no badge.
      }
    }

    return () => {
      for (const unsubscribe of unsubscribes) unsubscribe()
    }
  }, [nonce])

  return { counts, refresh }
}
