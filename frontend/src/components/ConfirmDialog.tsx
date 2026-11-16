import { useRef } from 'react'
import { useI18n } from '../i18n'
import { TestIds, testIdProps } from '../testing'
import { Dialog } from './Dialog'

export type ConfirmDialogProps = {
  open: boolean
  title: string
  message: string
  confirmLabel?: string
  cancelLabel?: string
  variant?: 'default' | 'danger'
  busy?: boolean
  onConfirm: () => void
  onCancel: () => void
}

export function ConfirmDialog({
  open,
  title,
  message,
  confirmLabel,
  cancelLabel,
  variant = 'default',
  busy = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const { t } = useI18n()
  const confirmRef = useRef<HTMLButtonElement>(null)

  return (
    <Dialog
      open={open}
      title={title}
      description={message}
      onClose={onCancel}
      asForm
      initialFocusRef={confirmRef}
      onSubmit={() => {
        if (!busy) onConfirm()
      }}
      footer={
        <>
          <button
            type="button"
            className="btn secondary"
            onClick={onCancel}
            disabled={busy}
            {...testIdProps(TestIds.confirmDialogCancel)}
          >
            {cancelLabel ?? t('common.cancel')}
          </button>
          <button
            ref={confirmRef}
            type="submit"
            className={variant === 'danger' ? 'btn danger' : 'btn'}
            disabled={busy}
            {...testIdProps(TestIds.confirmDialogConfirm)}
          >
            {confirmLabel ?? t('common.confirm')}
          </button>
        </>
      }
    />
  )
}
