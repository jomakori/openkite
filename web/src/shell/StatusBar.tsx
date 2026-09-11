export function StatusBar({
  version,
  connected,
}: {
  version?: string
  connected: boolean
}) {
  return (
    <div className="sidebar-footer">
      <span className="version">{version ? `v${version}` : ''}</span>
      <span className="status-line">
        <span className={connected ? 'dot ok' : 'dot err'} />
        {connected ? 'Connected' : 'Disconnected'}
      </span>
    </div>
  )
}
