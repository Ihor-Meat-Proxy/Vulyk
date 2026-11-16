import {
  createElement,
  useCallback,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { PromptDialog } from '../components/PromptDialog'

export type PromptOptions = {
  title: string
  message?: string
  label: string
  initialValue?: string
  placeholder?: string
  confirmLabel?: string
  cancelLabel?: string
  requiredExactValue?: string
  validate?: (value: string) => string | null
  variant?: 'default' | 'danger'
  inputTestId?: string
  errorTestId?: string
}

type Pending = {
  options: PromptOptions
  resolve: (value: string | null) => void
}

/**
 * Promise-based prompt that renders an in-app PromptDialog.
 * Mount `{promptDialog}` once in the component tree.
 * Resolves to trimmed string on confirm, or null on cancel.
 */
export function usePromptDialog(): {
  promptAsync: (options: PromptOptions) => Promise<string | null>
  promptDialog: ReactNode
} {
  const [pending, setPending] = useState<Pending | null>(null)
  const pendingRef = useRef<Pending | null>(null)

  const close = useCallback((result: string | null) => {
    const current = pendingRef.current
    pendingRef.current = null
    setPending(null)
    current?.resolve(result)
  }, [])

  const promptAsync = useCallback((options: PromptOptions) => {
    return new Promise<string | null>((resolve) => {
      if (pendingRef.current) {
        pendingRef.current.resolve(null)
      }
      const next: Pending = { options, resolve }
      pendingRef.current = next
      setPending(next)
    })
  }, [])

  const promptDialog = pending
    ? createElement(PromptDialog, {
        open: true,
        title: pending.options.title,
        message: pending.options.message,
        label: pending.options.label,
        initialValue: pending.options.initialValue,
        placeholder: pending.options.placeholder,
        confirmLabel: pending.options.confirmLabel,
        cancelLabel: pending.options.cancelLabel,
        requiredExactValue: pending.options.requiredExactValue,
        validate: pending.options.validate,
        variant: pending.options.variant,
        inputTestId: pending.options.inputTestId,
        errorTestId: pending.options.errorTestId,
        onConfirm: (value) => close(value),
        onCancel: () => close(null),
      })
    : null

  return { promptAsync, promptDialog }
}
