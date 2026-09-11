// Static fixture data for the browser/staging target.
//
// The desktop host answers `window.openkite.api.*` through the Rust asset
// handler. A plain static host (the PR-preview image, `python3 -m http.server`,
// `vite preview`) has no such handler, so the bridge shim in `bridge.ts` serves
// these fixtures instead — the UI renders content rather than empty states.
//
// Shapes match what the host returns: a kube `List` object (`{ items: [...] }`)
// for `list`/`watch`, and a single object for `get`. `rowsFromList`,
// `countRows`, and every downstream render path are therefore identical on
// desktop and web; only the data source differs.
import type { ClusterContext } from './bridge'

/** Snapshot of a connected staging cluster, shown in the shell chrome. */
export const FIXTURE_CONTEXT: ClusterContext = {
  context: 'staging (fixtures)',
  connected: true,
  version: 'staging',
}

interface PodOwner {
  kind: string
  name: string
}

interface KubeObject {
  apiVersion: string
  kind: string
  metadata: {
    name: string
    namespace?: string
    creationTimestamp: string
    ownerReferences?: PodOwner[]
  }
  spec?: { nodeName?: string }
  status?: {
    phase?: string
    qosClass?: string
    containerStatuses?: Array<{ ready: boolean; restartCount?: number }>
  }
}

/** ISO timestamp `minutes` before process start (keeps fixture ages readable). */
function ago(minutes: number): string {
  return new Date(Date.now() - minutes * 60_000).toISOString()
}

interface PodOptions {
  restarts?: number
  node?: string
  qos?: string
  controller?: PodOwner
}

function pod(
  name: string,
  namespace: string,
  phase: string,
  ready: boolean,
  minutes: number,
  options: PodOptions = {},
): KubeObject {
  return {
    apiVersion: 'v1',
    kind: 'Pod',
    metadata: {
      name,
      namespace,
      creationTimestamp: ago(minutes),
      ...(options.controller ? { ownerReferences: [options.controller] } : {}),
    },
    spec: { nodeName: options.node ?? 'staging-worker-a' },
    status: {
      phase,
      qosClass: options.qos ?? 'Burstable',
      containerStatuses: [
        { ready, restartCount: options.restarts ?? 0 },
        { ready: true, restartCount: 0 },
      ],
    },
  }
}

/** Cluster-scoped objects carry no `metadata.namespace`. */
function object(
  kind: string,
  name: string,
  namespace: string | undefined,
  minutes: number,
  phase?: string,
): KubeObject {
  return {
    apiVersion: 'v1',
    kind,
    metadata: { name, ...(namespace ? { namespace } : {}), creationTimestamp: ago(minutes) },
    ...(phase ? { status: { phase } } : {}),
  }
}

// Fixtures cover every counted kind in the shell nav (`COUNTED_KINDS`) plus
// `pods`, which the OKT-67 spike table lists, so neither surface renders empty.
const FIXTURE_ITEMS: Record<string, KubeObject[]> = {
  nodes: [
    object('Node', 'okt-staging-control', undefined, 9000, 'Running'),
    object('Node', 'okt-staging-worker-a', undefined, 8500, 'Running'),
    object('Node', 'okt-staging-worker-b', undefined, 7000, 'Running'),
  ],
  pods: [
    pod('openkite-api-6d9f4b7c8-2xk4p', 'default', 'Running', true, 240, {
      node: 'staging-worker-a',
      qos: 'Burstable',
      controller: { kind: 'Deployment', name: 'openkite-api' },
    }),
    pod('openkite-api-6d9f4b7c8-7jq2n', 'default', 'Running', true, 240, {
      node: 'staging-worker-a',
      qos: 'Burstable',
      controller: { kind: 'Deployment', name: 'openkite-api' },
    }),
    pod('openkite-web-5c8d7f4b6-9m2vq', 'default', 'Running', true, 180, {
      node: 'staging-worker-b',
      qos: 'Burstable',
      controller: { kind: 'Deployment', name: 'openkite-web' },
    }),
    pod('openkite-web-5c8d7f4b6-p4rtx', 'default', 'Pending', false, 3, {
      node: 'staging-worker-b',
      qos: 'BestEffort',
      controller: { kind: 'Deployment', name: 'openkite-web' },
    }),
    pod('postgres-0', 'default', 'Running', true, 1440, {
      node: 'staging-worker-a',
      qos: 'Guaranteed',
      controller: { kind: 'StatefulSet', name: 'postgres' },
    }),
    pod('redis-7b9c5d6f8-h2klm', 'default', 'CrashLoopBackOff', false, 620, {
      restarts: 14,
      node: 'staging-worker-b',
      qos: 'Burstable',
      controller: { kind: 'Deployment', name: 'redis' },
    }),
    pod('coredns-6f4b8c9d2-wq7zx', 'kube-system', 'Running', true, 9000, {
      node: 'staging-control',
      qos: 'Guaranteed',
      controller: { kind: 'Deployment', name: 'coredns' },
    }),
    pod('coredns-6f4b8c9d2-tz3pl', 'kube-system', 'Running', true, 9000, {
      node: 'staging-control',
      qos: 'Guaranteed',
      controller: { kind: 'Deployment', name: 'coredns' },
    }),
    pod('kube-proxy-9x2mn', 'kube-system', 'Running', true, 9000, {
      node: 'staging-control',
      qos: 'Guaranteed',
      controller: { kind: 'DaemonSet', name: 'kube-proxy' },
    }),
    pod('metrics-server-4d7f9b2c6-q8wnr', 'kube-system', 'Running', true, 540, {
      node: 'staging-worker-a',
      qos: 'Guaranteed',
      controller: { kind: 'Deployment', name: 'metrics-server' },
    }),
    pod('argocd-application-controller-0', 'argocd', 'Running', true, 4320, {
      node: 'staging-worker-a',
      qos: 'Guaranteed',
      controller: { kind: 'StatefulSet', name: 'argocd-application-controller' },
    }),
    pod('argocd-repo-server-7f5c9d3b4-mn2pq', 'argocd', 'Running', true, 4320, {
      node: 'staging-worker-b',
      qos: 'Burstable',
      controller: { kind: 'Deployment', name: 'argocd-repo-server' },
    }),
  ],
  deployments: [
    object('Deployment', 'openkite-api', 'default', 2400, 'Running'),
    object('Deployment', 'openkite-web', 'default', 1800, 'Running'),
    object('Deployment', 'redis', 'default', 620, 'Running'),
    object('Deployment', 'argocd-repo-server', 'argocd', 4320, 'Running'),
  ],
  services: [
    object('Service', 'openkite-api', 'default', 2400),
    object('Service', 'openkite-web', 'default', 1800),
    object('Service', 'redis', 'default', 620),
  ],
  configmaps: [
    object('ConfigMap', 'openkite-config', 'default', 2400),
    object('ConfigMap', 'kube-root-ca.crt', 'default', 9000),
  ],
  applications: [
    object('Application', 'openkite-staging', 'argocd', 4320, 'Synced'),
    object('Application', 'openkite-monitoring', 'argocd', 2880, 'OutOfSync'),
  ],
  projects: [object('AppProject', 'default', 'argocd', 9000)],
  repositories: [
    object('Repository', 'openkite', 'argocd', 9000),
    object('Repository', 'gke-gitops', 'argocd', 8000),
  ],
}

/** Pluralise a bare kind ("pod" → "pods"); kinds already plural pass through. */
function pluralKind(kind: string): string {
  const lower = kind.toLowerCase()
  return lower.endsWith('s') ? lower : `${lower}s`
}

/** Kube `List` shape for `list`/`watch`, filtered to `ns` when one is given. */
export function fixtureList(kind: string, ns?: string | null): unknown {
  const plural = pluralKind(kind)
  const items = FIXTURE_ITEMS[plural] ?? []
  const filtered = ns ? items.filter((item) => item.metadata.namespace === ns) : items
  return {
    apiVersion: 'v1',
    kind: `${plural.charAt(0).toUpperCase()}${plural.slice(1)}List`,
    items: filtered,
  }
}

/** Single-object shape for `get`. */
export function fixtureGet(kind: string, ns: string, name: string): unknown {
  const plural = pluralKind(kind)
  const found = (FIXTURE_ITEMS[plural] ?? []).find(
    (item) => item.metadata.name === name && (item.metadata.namespace ?? '') === ns,
  )
  if (!found) throw new Error(`fixture: ${plural}/${ns}/${name} not found`)
  return found
}

/** The subset of a bridge request the fixture provider answers. */
export interface FixtureRequest {
  op: string
  kind?: string
  ns?: string | null
  name?: string
  [key: string]: unknown
}

/**
 * Serve a bridge request from fixtures. Throws for ops with no fixture
 * (logs/exec), which the shell and the spike table never call.
 */
export function fixtureCall(request: FixtureRequest): unknown {
  switch (request.op) {
    case 'list':
    case 'watch':
      return fixtureList(request.kind ?? '', request.ns ?? null)
    case 'get':
      return fixtureGet(request.kind ?? '', request.ns ?? '', request.name ?? '')
    default:
      throw new Error(`fixture: op '${request.op}' has no fixture`)
  }
}
