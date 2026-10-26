import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from 'react'
import { TestIds, listItemProps, testIdProps } from '../testing'

type Toast = {
  id: number
  message: string
}

type ToastContextValue = {
  toast: (message: string) => void
}

const ToastContext = createContext<ToastContextValue | null>(null)

let nextId = 1

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<Toast[]>([])

  const toast = useCallback((message: string) => {
    const id = nextId++
    setItems((prev) => [...prev, { id, message }])
    window.setTimeout(() => {
      setItems((prev) => prev.filter((t) => t.id !== id))
    }, 2200)
  }, [])

  const value = useMemo(() => ({ toast }), [toast])

  return (
    <ToastContext.Provider value={value}>
      {children}
      <div
        className="app-toasts"
        role="status"
        aria-live="polite"
        {...testIdProps(TestIds.appToasts)}
      >
        {items.map((item, index) => (
          <div
            key={item.id}
            className="app-toast"
            {...listItemProps(TestIds.appToast, {
              entityId: String(item.id),
              index,
            })}
          >
            {item.message}
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  )
}

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext)
  if (!ctx) {
    return {
      toast: (message: string) => {
        if (typeof window !== 'undefined') {
          // Fallback when provider is missing (tests).
          console.info(message)
        }
      },
    }
  }
  return ctx
}
