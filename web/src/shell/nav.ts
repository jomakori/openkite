import type { IconName } from './icons'

export type NavIcon = Extract<
  IconName,
  | 'cluster'
  | 'node'
  | 'pods'
  | 'deploy'
  | 'services'
  | 'config'
  | 'storage'
  | 'network'
  | 'argo'
  | 'projects'
  | 'repo'
>

export interface NavItem {
  id: string
  label: string
  icon: NavIcon
  countKind?: string
  enabled: boolean
}

export interface NavSection {
  id: string
  label: string
  plugin?: boolean
  items: NavItem[]
}

export const NAV_SECTIONS: NavSection[] = [
  {
    id: 'cluster',
    label: 'Cluster',
    items: [
      { id: 'overview', label: 'Overview', icon: 'cluster', enabled: false },
      { id: 'nodes', label: 'Nodes', icon: 'node', countKind: 'nodes', enabled: false },
    ],
  },
  {
    id: 'workloads',
    label: 'Workloads',
    items: [
      { id: 'pods', label: 'Pods', icon: 'pods', countKind: 'pods', enabled: true },
      {
        id: 'deployments',
        label: 'Deployments',
        icon: 'deploy',
        countKind: 'deployments',
        enabled: false,
      },
      {
        id: 'services',
        label: 'Services',
        icon: 'services',
        countKind: 'services',
        enabled: false,
      },
    ],
  },
  {
    id: 'config',
    label: 'Config & Storage',
    items: [
      {
        id: 'configmaps',
        label: 'ConfigMaps',
        icon: 'config',
        countKind: 'configmaps',
        enabled: false,
      },
      { id: 'storage', label: 'Storage', icon: 'storage', enabled: false },
      { id: 'network', label: 'Network', icon: 'network', enabled: false },
    ],
  },
  {
    id: 'argo',
    label: 'Argo CD',
    plugin: true,
    items: [
      {
        id: 'applications',
        label: 'Applications',
        icon: 'argo',
        countKind: 'applications',
        enabled: true,
      },
      {
        id: 'projects',
        label: 'Projects',
        icon: 'projects',
        countKind: 'projects',
        enabled: false,
      },
      {
        id: 'repositories',
        label: 'Repositories',
        icon: 'repo',
        countKind: 'repositories',
        enabled: false,
      },
    ],
  },
]

export const COUNTED_KINDS: string[] = [
  ...new Set(
    NAV_SECTIONS.flatMap((section) =>
      section.items.flatMap((item) => (item.countKind ? [item.countKind] : [])),
    ),
  ),
]

export function findNavItem(
  id: string,
): { item: NavItem; section: NavSection } | undefined {
  for (const section of NAV_SECTIONS) {
    const item = section.items.find((candidate) => candidate.id === id)
    if (item) return { item, section }
  }
  return undefined
}
