import { Icon } from './shell/icons'
import type { LogLine } from './logs'

export interface LogDockProps {
  open: boolean
  paused: boolean
  collapsed: boolean
  pod: string
  lines: LogLine[]
  onTogglePause: () => void
  onToggleCollapse: () => void
  onClear: () => void
  onClose: () => void
}

export function LogDock({
  open,
  paused,
  collapsed,
  pod,
  lines,
  onTogglePause,
  onToggleCollapse,
  onClear,
  onClose,
}: LogDockProps) {
  const classes = ['log-panel']
  if (open) classes.push('open')
  if (paused) classes.push('paused')
  if (collapsed) classes.push('collapsed')

  return (
    <section className={classes.join(' ')} aria-label="Pod logs">
      <button className="log-handle" type="button" aria-expanded={open} onClick={onClose}>
        Logs
      </button>
      <div className="log-header">
        <div className="log-title">
          <Icon name="terminal" />
          <span>Logs</span>
          <span className="log-pod">{pod}</span>
        </div>
        <div className="log-actions">
          <button
            className="icon-btn"
            type="button"
            aria-label="Pause logs"
            aria-pressed={paused}
            onClick={onTogglePause}
          >
            <Icon name="pause" />
          </button>
          <button
            className="icon-btn"
            type="button"
            aria-label="Collapse logs"
            aria-pressed={collapsed}
            onClick={onToggleCollapse}
          >
            <Icon name="chevron" />
          </button>
          <button className="icon-btn" type="button" aria-label="Clear logs" onClick={onClear}>
            <Icon name="clear" />
          </button>
        </div>
      </div>
      <div className="log-paused" role="status">
        <Icon name="pause" />
        Log stream paused
      </div>
      <div className="log-body">
        {lines.length === 0 ? (
          <div className="log-line">
            <span className="log-time">--:--:--.---</span>
            <span className="log-level">INFO</span>
            <span className="log-msg">log buffer cleared</span>
          </div>
        ) : (
          lines.map((line, index) => (
            <div className="log-line" key={index}>
              <span className="log-time">{line.time}</span>
              <span className={`log-level ${line.level.toLowerCase()}`}>{line.level}</span>
              {line.method ? <span className="log-method">{line.method}</span> : null}
              <span
                className={line.level.toLowerCase() === 'error' ? 'log-msg error' : 'log-msg'}
              >
                {line.message}
              </span>
            </div>
          ))
        )}
      </div>
    </section>
  )
}
