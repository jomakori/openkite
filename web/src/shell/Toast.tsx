import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react'

interface ToastContextValue {
  show: (message: string) => void
}

const ToastContext = createContext<ToastContextValue>({ show: () => {} })

export function useToast(): ToastContextValue {
  return useContext(ToastContext)
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [message, setMessage] = useState('')
  const [visible, setVisible] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const show = useCallback((next: string) => {
    setMessage(next)
    setVisible(true)
    if (timer.current) clearTimeout(timer.current)
    timer.current = setTimeout(() => setVisible(false), 2200)
  }, [])

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current)
    },
    [],
  )

  return (
    <ToastContext.Provider value={{ show }}>
      {children}
      <ToastViewport message={message} show={visible} />
    </ToastContext.Provider>
  )
}

export function ToastViewport({ message, show }: { message: string; show: boolean }) {
  return (
    <div className={show ? 'toast show' : 'toast'} role="status" aria-live="polite">
      {message}
    </div>
  )
}
