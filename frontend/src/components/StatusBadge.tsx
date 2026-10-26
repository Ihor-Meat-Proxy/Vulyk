import { statusClass } from '../api'
import { useI18n } from '../i18n'
import { TestIds, listItemProps, testIdProps } from '../testing'
import { StatusIcon } from './StatusIcon'

type Props = {
  status: string
  className?: string
  /** Override default `status-badge` test id. */
  testId?: string
}

/**
 * i18n keys for result, run, and CI dispatch statuses.
 * Unknown values fall back to the raw string only as a last resort.
 */
const STATUS_I18N: Record<string, string> = {
  untested: 'status.untested',
  ready: 'status.ready',
  running: 'status.running',
  passed: 'status.passed',
  failed: 'status.failed',
  blocked: 'status.blocked',
  skipped: 'status.skipped',
  retest: 'status.retest',
  untestable: 'status.untestable',
  // Run / suite lifecycle
  draft: 'status.draft',
  completed: 'status.completed',
  in_progress: 'status.inProgress',
  'in-progress': 'status.inProgress',
  cancelled: 'status.cancelled',
  canceled: 'status.cancelled',
  // CI run open/closed + dispatch
  open: 'status.open',
  closed: 'status.closed',
  queued: 'status.queued',
  dispatched: 'status.dispatched',
  succeeded: 'status.succeeded',
}

export function statusLabelKey(status: string): string {
  const normalized = status.trim().toLowerCase().replace(/-/g, '_')
  return (
    STATUS_I18N[normalized] ||
    STATUS_I18N[status.trim().toLowerCase()] ||
    ''
  )
}

/** Colour-blind-safe status chip: always icon + translated label (never colour alone). */
export function StatusBadge({ status, className, testId }: Props) {
  const { t } = useI18n()
  const key = statusLabelKey(status)
  const label = key ? t(key) : status

  return (
    <span
      className={`status-pill ${statusClass(status)}${className ? ` ${className}` : ''}`}
      data-status={status}
      data-testid={testId ?? 'status-badge'}
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
  'data-testid'?: string
}

export function StatusActionButton({
  status,
  disabled,
  onClick,
  className,
  'data-testid': dataTestId,
}: ButtonProps) {
  const { t } = useI18n()
  const key = statusLabelKey(status)
  const label = key ? t(key) : status
  const statusKey = status.toLowerCase()

  return (
    <button
      type="button"
      className={`btn status-btn ${statusClass(status)} thumb-btn${className ? ` ${className}` : ''}`}
      disabled={disabled}
      onClick={onClick}
      data-status={status}
      {...(dataTestId
        ? testIdProps(dataTestId)
        : listItemProps(TestIds.statusBadge, {
            entityId: statusKey,
            entityKey: statusKey,
            index: 0,
          }))}
    >
      <StatusIcon status={status} />
      <span className="status-label">{label}</span>
    </button>
  )
}
