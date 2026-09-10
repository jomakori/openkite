import type { ResourceRow } from './bridge'

/** Small resource table fed entirely from bridge data (push or list reply). */
export function ResourceTable({
  rows,
  source,
}: {
  rows: ResourceRow[]
  source: string
}) {
  return (
    <div className="overflow-hidden rounded-lg border border-slate-800">
      <table data-testid="resource-table" className="w-full text-left text-sm">
        <thead className="bg-slate-900 text-xs uppercase tracking-wide text-slate-400">
          <tr>
            <th className="px-3 py-2 font-medium">Name</th>
            <th className="px-3 py-2 font-medium">Namespace</th>
            <th className="px-3 py-2 font-medium">Phase</th>
            <th className="px-3 py-2 font-medium">Ready</th>
            <th className="px-3 py-2 font-medium">Age</th>
          </tr>
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr>
              <td
                data-testid="empty-state"
                colSpan={5}
                className="px-3 py-6 text-center text-slate-500"
              >
                waiting for bridge data ({source})
              </td>
            </tr>
          ) : (
            rows.map((row) => (
              <tr
                key={`${row.namespace}/${row.name}`}
                data-testid="resource-row"
                className="border-t border-slate-800/70 even:bg-slate-900/40"
              >
                <td className="px-3 py-2 font-mono text-slate-100">{row.name}</td>
                <td className="px-3 py-2 text-slate-300">{row.namespace}</td>
                <td className="px-3 py-2">
                  <span className="rounded bg-emerald-500/10 px-2 py-0.5 text-emerald-300">
                    {row.phase}
                  </span>
                </td>
                <td className="px-3 py-2 text-slate-300">{row.ready}</td>
                <td className="px-3 py-2 text-slate-500">{row.age}</td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  )
}
