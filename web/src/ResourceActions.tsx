import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import type { RowAction, RowActionId } from './actions'
import { Icon } from './shell/icons'

export function RowActionsMenu({
  actions,
  onAction,
}: {
  actions: RowAction[]
  onAction: (id: RowActionId) => void
}) {
  const [open, setOpen] = useState(false)
  const [position, setPosition] = useState<{ top: number; left: number } | null>(null)
  const wrap = useRef<HTMLDivElement>(null)
  const menu = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const onDocumentMouseDown = (event: MouseEvent) => {
      const target = event.target as Node
      if (wrap.current?.contains(target) || menu.current?.contains(target)) return
      setOpen(false)
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false)
    }
    document.addEventListener('mousedown', onDocumentMouseDown)
    document.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('mousedown', onDocumentMouseDown)
      document.removeEventListener('keydown', onKeyDown)
    }
  }, [open])

  return (
    <div className="row-menu-wrap" ref={wrap}>
      <button
        className="icon-btn row-menu-btn"
        type="button"
        aria-label="Resource actions"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={(event) => {
          event.stopPropagation()
          if (open) {
            setOpen(false)
            return
          }
          const rect = event.currentTarget.getBoundingClientRect()
          setPosition({ top: rect.bottom + 4, left: Math.max(8, rect.right - 176) })
          setOpen(true)
        }}
      >
        <Icon name="kebab" />
      </button>
      {open && position && typeof document !== 'undefined'
        ? createPortal(
            <div
              className="row-menu"
              role="menu"
              ref={menu}
              style={{ position: 'fixed', top: position.top, left: position.left }}
            >
              {actions.map((action) => (
                <button
                  key={action.id}
                  type="button"
                  role="menuitem"
                  className={action.destructive ? 'row-menu-item destructive' : 'row-menu-item'}
                  disabled={!action.enabled}
                  title={action.reason}
                  onClick={(event) => {
                    event.stopPropagation()
                    if (!action.enabled) return
                    setOpen(false)
                    onAction(action.id)
                  }}
                >
                  {action.label}
                </button>
              ))}
            </div>,
            document.body,
          )
        : null}
    </div>
  )
}

export function YamlModal({
  title,
  text,
  onClose,
}: {
  title: string
  text: string
  onClose: () => void
}) {
  return (
    <div className="yaml-backdrop" role="presentation" onClick={onClose}>
      <div
        className="yaml-modal"
        role="dialog"
        aria-label={`${title} YAML`}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="yaml-header">
          <h2>{title}</h2>
          <button className="icon-btn" type="button" aria-label="Close YAML" onClick={onClose}>
            <Icon name="close" />
          </button>
        </div>
        <pre className="yaml-body">{text}</pre>
      </div>
    </div>
  )
}
