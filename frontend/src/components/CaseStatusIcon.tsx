/** Colour-blind-safe glyphs for case lifecycle status (always paired with a label). */

type Props = {
  status: string
  className?: string
}

export function CaseStatusIcon({ status, className }: Props) {
  const kind = status.trim().toLowerCase()
  const common = {
    className: `case-status-icon ${className ?? ''}`.trim(),
    width: 14,
    height: 14,
    viewBox: '0 0 16 16',
    'aria-hidden': true as const,
    focusable: false as const,
  }

  switch (kind) {
    case 'ready':
      return (
        <svg {...common}>
          <path
            fill="currentColor"
            d="M6.5 11.2 3.3 8l1.1-1.1 2.1 2.1 4.6-4.6L12.2 5.5 6.5 11.2z"
          />
        </svg>
      )
    case 'deprecated':
      return (
        <svg {...common}>
          <path
            fill="currentColor"
            d="M3.2 7.1h9.6v1.8H3.2z"
          />
        </svg>
      )
    case 'draft':
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
            strokeDasharray="2.2 1.8"
          />
        </svg>
      )
  }
}
