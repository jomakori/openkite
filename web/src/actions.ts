// Per-row action vocabulary, modelled on Lens.
//
// Every entry is gated on what the backend can honour today: an action the
// bridge cannot serve is disabled with a reason, never a silent no-op. The
// mutation ops (delete/scale/restart) are disabled while `crud::apply_mutation`
// is the Phase-1 placeholder; `capabilities.mutations` flips them on when that
// lands.
import type { ResourceRow } from './bridge'
import type { Capabilities } from './settings'

export type RowActionId = 'view-yaml' | 'logs' | 'scale' | 'restart' | 'delete'

export interface RowAction {
  id: RowActionId
  label: string
  enabled: boolean
  /** Why a disabled entry is disabled, surfaced as the menu item title. */
  reason?: string
  destructive?: boolean
}

const SCALABLE_KINDS = ['deployments', 'statefulsets', 'replicasets']
const RESTARTABLE_KINDS = ['deployments', 'statefulsets', 'daemonsets']

const MUTATIONS_OFF = 'Cluster mutations are not wired to the bridge yet'

/**
 * The kebab menu for one row, in Lens order. `kind` is the console noun
 * (`pods`, `deployments`, …); `capabilities` comes from the host context.
 */
export function rowActions(
  kind: string,
  row: ResourceRow,
  capabilities: Capabilities,
): RowAction[] {
  void row
  const actions: RowAction[] = [{ id: 'view-yaml', label: 'View YAML', enabled: true }]

  if (kind === 'pods') {
    actions.push({ id: 'logs', label: 'Logs', enabled: capabilities.logs })
  }
  if (SCALABLE_KINDS.includes(kind)) {
    actions.push({
      id: 'scale',
      label: 'Scale',
      enabled: capabilities.mutations,
      reason: capabilities.mutations ? undefined : MUTATIONS_OFF,
      destructive: true,
    })
  }
  if (RESTARTABLE_KINDS.includes(kind)) {
    actions.push({
      id: 'restart',
      label: 'Restart',
      enabled: capabilities.mutations,
      reason: capabilities.mutations ? undefined : MUTATIONS_OFF,
    })
  }
  actions.push({
    id: 'delete',
    label: 'Delete',
    enabled: capabilities.mutations,
    reason: capabilities.mutations ? undefined : MUTATIONS_OFF,
    destructive: true,
  })
  return actions
}
