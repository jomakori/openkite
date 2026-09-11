export function PullIndicator({ show, text }: { show: boolean; text: string }) {
  return (
    <div className={show ? 'pull-indicator show' : 'pull-indicator'}>
      <span className="spinner" />
      {text || 'Pull to refresh'}
    </div>
  )
}
