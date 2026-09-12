// Resource events for the detail panel's Events tab.
//
// Events are served by the existing bridge `list` op (`list('events', ns)`);
// this module filters the cluster's event stream down to one involved object.
import { useEffect, useState } from 'react'
import { resolveBridge } from './bridge'

export interface ResourceEvent {
  name: string
  type: string
  reason: string
  message: string
  count: number
  age: string
}

const KIND_BY_NOUN: Record<string, string> = {
  pods: 'Pod',
  deployments: 'Deployment',
  statefulsets: 'StatefulSet',
  daemonsets: 'DaemonSet',
  replicasets: 'ReplicaSet',
  jobs: 'Job',
  cronjobs: 'CronJob',
  services: 'Service',
  configmaps: 'ConfigMap',
  secrets: 'Secret',
  nodes: 'Node',
  namespaces: 'Namespace',
  applications: 'Application',
  projects: 'AppProject',
  repositories: 'Repository',
}

/** Kubernetes Kind for a console noun (`pods` → `Pod`). */
export function resourceKind(noun: string): string {
  const known = KIND_BY_NOUN[noun]
  if (known) return known
  const singular = noun.endsWith('s') ? noun.slice(0, -1) : noun
  return singular.charAt(0).toUpperCase() + singular.slice(1)
}

function formatAge(timestamp: string | undefined): string {
  if (!timestamp) return '—'
  const created = Date.parse(timestamp)
  if (Number.isNaN(created)) return '—'
  const seconds = Math.max(0, Math.floor((Date.now() - created) / 1000))
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h`
  return `${Math.floor(hours / 24)}d`
}

interface RawEvent {
  metadata?: { name?: string; creationTimestamp?: string }
  involvedObject?: { kind?: string; name?: string; namespace?: string }
  type?: string
  reason?: string
  message?: string
  count?: number
}

/** Filter a kube Event list to the events involving one resource. */
export function eventsForResource(
  result: unknown,
  noun: string,
  name: string,
  namespace: string,
): ResourceEvent[] {
  const items =
    result && typeof result === 'object' && Array.isArray((result as { items?: unknown }).items)
      ? ((result as { items: unknown[] }).items as RawEvent[])
      : []
  const kind = resourceKind(noun)
  return items
    .filter((event) => {
      const involved = event.involvedObject
      if (!involved) return false
      if (involved.name !== name) return false
      if (involved.kind !== kind) return false
      return (involved.namespace ?? '') === (namespace ?? '')
    })
    .map((event) => ({
      name: event.metadata?.name ?? '—',
      type: event.type ?? 'Normal',
      reason: event.reason ?? '—',
      message: event.message ?? '',
      count: event.count ?? 1,
      age: formatAge(event.metadata?.creationTimestamp),
    }))
}

/** Load the events for one resource through the bridge (empty on failure). */
export function useResourceEvents(
  noun: string,
  name: string | null,
  namespace: string | null,
): ResourceEvent[] {
  const [events, setEvents] = useState<ResourceEvent[]>([])

  useEffect(() => {
    if (!name) {
      setEvents([])
      return
    }
    let cancelled = false
    void (async () => {
      try {
        const bridge = resolveBridge().bridge
        const result = await bridge.api.list('events', namespace || null)
        if (!cancelled) setEvents(eventsForResource(result, noun, name, namespace ?? ''))
      } catch {
        if (!cancelled) setEvents([])
      }
    })()
    return () => {
      cancelled = true
    }
  }, [noun, name, namespace])

  return events
}
