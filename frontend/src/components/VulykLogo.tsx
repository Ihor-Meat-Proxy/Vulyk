import { Link } from 'react-router-dom'
import { useI18n } from '../i18n'
import { TestIds, testIdProps } from '../testing'

/** Honeycomb hex fills — overridden per theme via CSS vars when needed. */
const HEXES: { points: string; fillVar: string; fallback: string }[] = [
  {
    points:
      '-25.98,-8.80 -18.36,-4.40 -18.36,4.40 -25.98,8.80 -33.60,4.40 -33.60,-4.40',
    fillVar: '--brand-honey-1',
    fallback: '#E3BB72',
  },
  {
    points:
      '25.98,-8.80 33.60,-4.40 33.60,4.40 25.98,8.80 18.36,4.40 18.36,-4.40',
    fillVar: '--brand-honey-2',
    fallback: '#D9A55A',
  },
  {
    points:
      '-17.32,6.20 -9.70,10.60 -9.70,19.40 -17.32,23.80 -24.94,19.40 -24.94,10.60',
    fillVar: '--brand-honey-3',
    fallback: '#DDB067',
  },
  {
    points:
      '17.32,6.20 24.94,10.60 24.94,19.40 17.32,23.80 9.70,19.40 9.70,10.60',
    fillVar: '--brand-honey-3',
    fallback: '#DDB067',
  },
  {
    points:
      '-8.66,21.20 -1.04,25.60 -1.04,34.40 -8.66,38.80 -16.28,34.40 -16.28,25.60',
    fillVar: '--brand-honey-4',
    fallback: '#D6A256',
  },
  {
    points:
      '8.66,21.20 16.28,25.60 16.28,34.40 8.66,38.80 1.04,34.40 1.04,25.60',
    fillVar: '--brand-honey-4',
    fallback: '#D6A256',
  },
  {
    points:
      '0.00,36.20 7.62,40.60 7.62,49.40 0.00,53.80 -7.62,49.40 -7.62,40.60',
    fillVar: '--brand-honey-5',
    fallback: '#CF9849',
  },
]

type Props = {
  /** Hide the wordmark (mobile / compact). */
  markOnly?: boolean
  /** When true, wrap in a home Link. */
  linkHome?: boolean
  className?: string
  /** Override accessible name (defaults to brand.name). */
  label?: string
  id?: string
  testId?: string
}

/** Honeycomb-V mark + optional wordmark (currentColor → theme ink). */
export function VulykLogo({
  markOnly = false,
  linkHome = false,
  className,
  label,
  id,
  testId = TestIds.brandLogo,
}: Props) {
  const { t } = useI18n()
  const name = label ?? t('brand.name')
  const viewBox = markOnly
    ? '-35.98 -10.00 71.96 65.00'
    : '-35.98 -10.00 244.67 65.00'

  const svg = (
    <svg
      className={`vulyk-logo${markOnly ? ' mark-only' : ''}${className ? ` ${className}` : ''}`}
      xmlns="http://www.w3.org/2000/svg"
      viewBox={viewBox}
      role={linkHome ? 'presentation' : 'img'}
      aria-hidden={linkHome ? true : undefined}
      aria-label={linkHome ? undefined : name}
      id={id}
      {...testIdProps(testId)}
    >
      {!linkHome ? <title>{name}</title> : null}
      {HEXES.map((hex) => (
        <polygon
          key={hex.points}
          points={hex.points}
          fill={`var(${hex.fillVar}, ${hex.fallback})`}
        />
      ))}
      {!markOnly ? (
        <text
          className="vulyk-logo-wordmark"
          x="48.93"
          y="38.10"
          fontFamily="var(--font-body), Inter, system-ui, sans-serif"
          fontWeight="600"
          fontSize="46.80"
          fill="currentColor"
        >
          {t('brand.name')}
        </text>
      ) : null}
    </svg>
  )

  if (!linkHome) return svg

  return (
    <Link
      to="/"
      className="vulyk-logo-link"
      aria-label={t('a11y.brandHome')}
      {...testIdProps(TestIds.brandLogoLink)}
    >
      {svg}
    </Link>
  )
}
