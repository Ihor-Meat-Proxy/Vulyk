import { statusClass } from '../api'
import { useI18n } from '../i18n'
import { StatusIcon } from './StatusIcon'

type Props = {
  status: string
  className?: string
  /** Larger control style for execute buttons */
  asButton?: boolean
}

const STATUS_I18N: Record<string, string> = {
  untested: 'status.untested',
  passed: 'status.passed',
  failed: 'status.failed',
  blocked: 'status.blocked',
  skipped: 'status.skipped',
  retest: 'status.retest',
}

export function statusLabelKey(status: string): string {
  return STATUS_I18N[status.toLowerCase()] ?? ''
}

/** Colour-blind-safe status chip: always icon + label (never colour alone). */
export function StatusBadge({ status, className }: Props) {
  const { t } = useI18n()
  const key = statusLabelKey(status)
  const label = key ? t(key) : status

  return (
    <span
      className={`status-pill ${statusClass(status)}${className ? ` ${className}` : ''}`}
      data-status={status}
    >
      <StatusIcon status={status} />
      <span className="status-label">{label}</span>
    </span>
  )
}

type ButtonProps = {
  status: string
  disabled?: boolean
  onClick?: () => void
  className?: string
}

export function StatusActionButton({
  status,
  disabled,
  onClick,
  className,
}: ButtonProps) {
  const { t } = useI18n()
  const key = statusLabelKey(status)
  const label = key ? t(key) : status

  return (
    <button
      type="button"
      className={`btn status-btn ${statusClass(status)} thumb-btn${className ? ` ${className}` : ''}`}
      disabled={disabled}
      onClick={onClick}
      data-status={status}
    >
      <StatusIcon status={status} />
      <span className="status-label">{label}</span>
    </button>
  )
}
