import {
  useEffect,
  useId,
  useRef,
  type FormEvent,
  type ReactNode,
  type RefObject,
} from 'react'
import { createPortal } from 'react-dom'
import { useFocusTrap } from '../hooks/useFocusTrap'
import { TestIds, testIdProps } from '../testing'

export type DialogProps = {
  open: boolean
  title: string
  description?: string
  onClose: () => void
  children?: ReactNode
  /** Footer actions (Cancel / Confirm / Create). */
  footer?: ReactNode
  /** When true, wraps body+footer in a <form> so Enter submits. */
  asForm?: boolean
  onSubmit?: (e: FormEvent<HTMLFormElement>) => void
  /** Extra class on the panel. */
  className?: string
  /** Optional labelled panel test id override. */
  panelTestId?: string
  /** Prefer this element for initial focus. */
  initialFocusRef?: RefObject<HTMLElement | null>
}

/**
 * Base modal: overlay, panel, focus trap, Esc closes, body scroll lock,
 * role="dialog" aria-modal labelled by title.
 */
export function Dialog({
  open,
  title,
  description,
  onClose,
  children,
  footer,
  asForm = false,
  onSubmit,
  className,
  panelTestId,
  initialFocusRef,
}: DialogProps) {
  const titleId = useId()
  const descId = useId()
  const panelRef = useRef<HTMLDivElement>(null)

  useFocusTrap(panelRef, open, { onEscape: onClose, initialFocusRef })

  useEffect(() => {
    if (!open) return
    const prev = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    return () => {
      document.body.style.overflow = prev
    }
  }, [open])

  if (!open) return null

  const body = (
    <>
      <header className="app-dialog-header">
        <h2 id={titleId} className="app-dialog-title">
          {title}
        </h2>
        {description ? (
          <p id={descId} className="app-dialog-description muted">
            {description}
          </p>
        ) : null}
      </header>
      {children != null && children !== false ? (
        <div className="app-dialog-body">{children}</div>
      ) : null}
      {footer ? <footer className="app-dialog-footer">{footer}</footer> : null}
    </>
  )

  const panel = (
    <div
      ref={panelRef}
      className={`app-dialog-panel${className ? ` ${className}` : ''}`}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      aria-describedby={description ? descId : undefined}
      {...testIdProps(panelTestId ?? TestIds.dialogPanel)}
    >
      {asForm ? (
        <form
          className="app-dialog-form"
          onSubmit={(e) => {
            e.preventDefault()
            onSubmit?.(e)
          }}
        >
          {body}
        </form>
      ) : (
        body
      )}
    </div>
  )

  return createPortal(
    <div className="app-dialog-overlay" {...testIdProps(TestIds.dialogOverlay)}>
      <button
        type="button"
        className="app-dialog-backdrop"
        aria-label={title}
        tabIndex={-1}
        onClick={onClose}
        {...testIdProps(TestIds.dialogBackdrop)}
      />
      {panel}
    </div>,
    document.body,
  )
}
