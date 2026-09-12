import { useEffect, useState } from 'react'
import { resolveBridge } from '../bridge'

/** Namespace names for the topbar filter, served by the bridge list op. */
export function useNamespaces(): string[] {
  const [namespaces, setNamespaces] = useState<string[]>([])

  useEffect(() => {
    let cancelled = false
    void (async () => {
      try {
        const result = await resolveBridge().bridge.api.list('namespaces', null)
        if (!cancelled) setNamespaces(namespacesFromList(result))
      } catch {
        if (!cancelled) setNamespaces([])
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])

  return namespaces
}

/** Extract `metadata.name` from a kube namespace list, sorted. */
export function namespacesFromList(result: unknown): string[] {
  const items =
    result && typeof result === 'object' && Array.isArray((result as { items?: unknown }).items)
      ? ((result as { items: Array<{ metadata?: { name?: string } }> }).items ?? [])
      : []
  return items
    .map((item) => item.metadata?.name ?? '')
    .filter(Boolean)
    .sort((a, b) => a.localeCompare(b))
}
