import { useCallback, useRef, useState, type ReactNode } from 'react'
import { createElement } from 'react'
import { ConfirmDialog } from '../components/ConfirmDialog'
import { useI18n } from '../i18n'

export type ConfirmOptions = {
  title?: string
  message: string
  confirmLabel?: string
  cancelLabel?: string
  variant?: 'default' | 'danger'
}

type Pending = {
  options: ConfirmOptions
  resolve: (value: boolean) => void
}

/**
 * Promise-based confirm that renders an in-app ConfirmDialog.
 * Mount `{confirmDialog}` once in the component tree.
 */
export function useConfirmDialog(): {
  confirmAsync: (options: ConfirmOptions) => Promise<boolean>
  confirmDialog: ReactNode
} {
  const { t } = useI18n()
  const [pending, setPending] = useState<Pending | null>(null)
  const pendingRef = useRef<Pending | null>(null)

  const close = useCallback((result: boolean) => {
    const current = pendingRef.current
    pendingRef.current = null
    setPending(null)
    current?.resolve(result)
  }, [])

  const confirmAsync = useCallback((options: ConfirmOptions) => {
    return new Promise<boolean>((resolve) => {
      // Resolve any prior pending as cancelled (should be rare).
      if (pendingRef.current) {
        pendingRef.current.resolve(false)
      }
      const next: Pending = { options, resolve }
      pendingRef.current = next
      setPending(next)
    })
  }, [])

  const confirmDialog = pending
    ? createElement(ConfirmDialog, {
        open: true,
        title: pending.options.title ?? t('dialog.confirmTitle'),
        message: pending.options.message,
        confirmLabel: pending.options.confirmLabel,
        cancelLabel: pending.options.cancelLabel,
        variant: pending.options.variant,
        onConfirm: () => close(true),
        onCancel: () => close(false),
      })
    : null

  return { confirmAsync, confirmDialog }
}
