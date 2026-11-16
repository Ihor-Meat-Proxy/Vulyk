import { useEffect, useId, useState } from 'react'
import { useI18n } from '../i18n'
import { TestIds, testIdProps } from '../testing'
import { Dialog } from './Dialog'

export type PromptDialogProps = {
  open: boolean
  title: string
  message?: string
  label: string
  initialValue?: string
  placeholder?: string
  confirmLabel?: string
  cancelLabel?: string
  /** When set, Confirm stays disabled until the input equals this string. */
  requiredExactValue?: string
  /** Validate before confirm; return error message or null. */
  validate?: (value: string) => string | null
  variant?: 'default' | 'danger'
  busy?: boolean
  onConfirm: (value: string) => void
  onCancel: () => void
  inputTestId?: string
  errorTestId?: string
}

export function PromptDialog({
  open,
  title,
  message,
  label,
  initialValue = '',
  placeholder,
  confirmLabel,
  cancelLabel,
  requiredExactValue,
  validate,
  variant = 'default',
  busy = false,
  onConfirm,
  onCancel,
  inputTestId,
  errorTestId,
}: PromptDialogProps) {
  const { t } = useI18n()
  const inputId = useId()
  const errorId = useId()
  const [value, setValue] = useState(initialValue)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (open) {
      setValue(initialValue)
      setError(null)
    }
  }, [open, initialValue])

  const exactOk = requiredExactValue == null || value === requiredExactValue
  const canSubmit = exactOk && !busy

  const submit = () => {
    const trimmed = value.trim()
    if (requiredExactValue != null) {
      if (value !== requiredExactValue) {
        setError(t('dialog.exactValueRequired'))
        return
      }
      onConfirm(value)
      return
    }
    if (!trimmed) {
      setError(t('dialog.nameRequired'))
      return
    }
    const custom = validate?.(trimmed) ?? null
    if (custom) {
      setError(custom)
      return
    }
    onConfirm(trimmed)
  }

  return (
    <Dialog
      open={open}
      title={title}
      description={message}
      onClose={onCancel}
      asForm
      onSubmit={submit}
      footer={
        <>
          <button
            type="button"
            className="btn secondary"
            onClick={onCancel}
            disabled={busy}
            {...testIdProps(TestIds.promptDialogCancel)}
          >
            {cancelLabel ?? t('common.cancel')}
          </button>
          <button
            type="submit"
            className={variant === 'danger' ? 'btn danger' : 'btn'}
            disabled={!canSubmit}
            {...testIdProps(TestIds.promptDialogConfirm)}
          >
            {confirmLabel ?? t('common.confirm')}
          </button>
        </>
      }
    >
      <label className="app-dialog-field" htmlFor={inputId}>
        <span>{label}</span>
        <input
          id={inputId}
          type="text"
          value={value}
          placeholder={placeholder}
          autoComplete="off"
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? errorId : undefined}
          onChange={(e) => {
            setValue(e.target.value)
            if (error) setError(null)
          }}
          {...testIdProps(inputTestId ?? TestIds.promptDialogInput)}
        />
      </label>
      {error ? (
        <p
          id={errorId}
          className="error app-dialog-error"
          role="alert"
          {...testIdProps(errorTestId ?? TestIds.promptDialogError)}
        >
          {error}
        </p>
      ) : null}
    </Dialog>
  )
}
