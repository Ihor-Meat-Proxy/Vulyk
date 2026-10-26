type Props = {
  status: string
  className?: string
}

/** Normalized visual kind for glyphs (result + run + CI vocabularies). */
export type StatusVisualKind =
  | 'passed'
  | 'failed'
  | 'blocked'
  | 'skipped'
  | 'retest'
  | 'ready'
  | 'running'
  | 'untestable'
  | 'untested'
  | 'cancelled'
  | 'open'
  | 'unknown'

/** Map any status string to a visual icon kind. */
export function statusVisualKind(status: string): StatusVisualKind {
  const s = status.trim().toLowerCase().replace(/-/g, '_')
  switch (s) {
    case 'passed':
    case 'completed':
    case 'succeeded':
    case 'closed':
      return 'passed'
    case 'failed':
      return 'failed'
    case 'blocked':
      return 'blocked'
    case 'skipped':
      return 'skipped'
    case 'retest':
      return 'retest'
    case 'ready':
    case 'queued':
    case 'dispatched':
    case 'draft':
      return 'ready'
    case 'running':
    case 'in_progress':
      return 'running'
    case 'untestable':
      return 'untestable'
    case 'untested':
      return 'untested'
    case 'cancelled':
    case 'canceled':
      return 'cancelled'
    case 'open':
      return 'open'
    default:
      return 'unknown'
  }
}

/** Decorative status glyph — always paired with a visible label. */
export function StatusIcon({ status, className }: Props) {
  const kind = statusVisualKind(status)
  const common = {
    className: `status-icon${kind === 'running' ? ' status-icon-spin' : ''}${className ? ` ${className}` : ''}`.trim(),
    width: 14,
    height: 14,
    viewBox: '0 0 16 16',
    'aria-hidden': true as const,
    focusable: false as const,
  }

  switch (kind) {
    case 'passed':
      return (
        <svg {...common}>
          <path
            fill="currentColor"
            d="M6.5 11.2 3.3 8l1.1-1.1 2.1 2.1 4.6-4.6L12.2 5.5 6.5 11.2z"
          />
        </svg>
      )
    case 'failed':
      return (
        <svg {...common}>
          <path
            fill="currentColor"
            d="M4.2 3.1 8 6.9l3.8-3.8 1.1 1.1L9.1 8l3.8 3.8-1.1 1.1L8 9.1l-3.8 3.8-1.1-1.1L6.9 8 3.1 4.2l1.1-1.1z"
          />
        </svg>
      )
    case 'blocked':
      return (
        <svg {...common}>
          <path
            fill="currentColor"
            fillRule="evenodd"
            d="M8 1.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13zM3.7 8a4.3 4.3 0 0 1 7.1-3.2L4.8 10.8A4.27 4.27 0 0 1 3.7 8zm1.5 3.2L11.2 5.2A4.3 4.3 0 0 1 5.2 11.2z"
          />
        </svg>
      )
    case 'skipped':
      return (
        <svg {...common}>
          <path
            fill="currentColor"
            d="M3.2 3.5h2.2v9H3.2v-9zm3.3 0 6.3 4.5-6.3 4.5v-9z"
          />
        </svg>
      )
    case 'retest':
      return (
        <svg {...common}>
          <path
            fill="currentColor"
            d="M8 2.2a5.8 5.8 0 0 1 5.5 4h1.7L12.5 9.9 9.8 6.2h1.5A4.2 4.2 0 1 0 12 10.4l1.1 1.2A5.8 5.8 0 1 1 8 2.2z"
          />
        </svg>
      )
    case 'ready':
      return (
        <svg {...common}>
          <circle
            cx="8"
            cy="8"
            r="5.2"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
          />
          <circle cx="8" cy="8" r="2.2" fill="currentColor" />
        </svg>
      )
    case 'open':
      return (
        <svg {...common}>
          <circle
            cx="8"
            cy="8"
            r="5.2"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
          />
        </svg>
      )
    case 'running':
      return (
        <svg {...common}>
          <path
            fill="currentColor"
            d="M8 1.6a6.4 6.4 0 0 1 6.2 4.6h-1.7A4.8 4.8 0 1 0 8 12.8V14.4A6.4 6.4 0 1 1 8 1.6z"
          />
          <path fill="currentColor" d="M10.2 6.8 14 8.8l-3.8 2V6.8z" />
        </svg>
      )
    case 'cancelled':
      return (
        <svg {...common}>
          <path
            fill="currentColor"
            fillRule="evenodd"
            d="M8 1.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13zm3.1 3.4L9.1 8l2 2.1-1.1 1.1L8 9.1l-2 2.1-1.1-1.1L6.9 8l-2-2.1L6 4.8 8 6.9l2-2.1 1.1 1.1z"
          />
        </svg>
      )
    case 'untestable':
      return (
        <svg {...common}>
          <path
            fill="currentColor"
            fillRule="evenodd"
            d="M8 1.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13zM4.2 5.1l6.7 6.7A4.3 4.3 0 0 1 4.2 5.1zm1.1-1.1a4.3 4.3 0 0 1 6.6 5.5L5.3 4z"
          />
        </svg>
      )
    case 'untested':
    case 'unknown':
    default:
      return (
        <svg {...common}>
          <circle
            cx="8"
            cy="8"
            r="5.2"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
          />
        </svg>
      )
  }
}
