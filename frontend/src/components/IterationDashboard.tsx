import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import {
  DASHBOARD_STATUSES,
  type IterationDashboard as Dashboard,
  type UpdateIteration,
} from '../api'
import { useI18n } from '../i18n'
import {
  TestIds,
  isSafeEntityKey,
  listChildProps,
  listItemProps,
  testIdProps,
} from '../testing'
import { StatusBadge } from './StatusBadge'
import { StatusIcon } from './StatusIcon'

type Props = {
  dashboard: Dashboard
  canEdit?: boolean
  onSaveSchedule?: (body: UpdateIteration) => Promise<void>
}

function countFor(
  counts: Dashboard['status_counts'],
  key: (typeof DASHBOARD_STATUSES)[number],
): number {
  switch (key) {
    case 'Ready':
      return counts.ready
    case 'Running':
      return counts.running
    case 'Passed':
      return counts.passed
    case 'Failed':
      return counts.failed
    case 'Blocked':
      return counts.blocked
    case 'Skipped':
      return counts.skipped
    case 'Untestable':
      return counts.untestable
  }
}

function Donut({ counts }: { counts: Dashboard['status_counts'] }) {
  const { t } = useI18n()
  const total = Math.max(counts.total, 1)
  const size = 140
  const stroke = 22
  const r = (size - stroke) / 2
  const c = 2 * Math.PI * r
  let offset = 0
  const segments = DASHBOARD_STATUSES.map((status) => {
    const value = countFor(counts, status)
    const len = (value / total) * c
    const seg = { status, value, len, offset }
    offset += len
    return seg
  }).filter((s) => s.value > 0)

  return (
    <div className="exec-donut" aria-label={t('executions.statusCounts')}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
        <circle
          cx={size / 2}
          cy={size / 2}
          r={r}
          fill="none"
          stroke="var(--line)"
          strokeWidth={stroke}
        />
        {segments.map((seg) => (
          <circle
            key={seg.status}
            className={`exec-donut-seg status-${seg.status.toLowerCase()}`}
            cx={size / 2}
            cy={size / 2}
            r={r}
            fill="none"
            stroke={`var(--status-${seg.status.toLowerCase()})`}
            strokeWidth={stroke}
            strokeDasharray={`${seg.len} ${c - seg.len}`}
            strokeDashoffset={-seg.offset}
            transform={`rotate(-90 ${size / 2} ${size / 2})`}
          />
        ))}
        <text
          x="50%"
          y="48%"
          textAnchor="middle"
          className="exec-donut-total"
          fill="var(--ink)"
        >
          {counts.total}
        </text>
        <text
          x="50%"
          y="62%"
          textAnchor="middle"
          className="exec-donut-label"
          fill="var(--ink-muted)"
        >
          total
        </text>
      </svg>
      <ul className="exec-donut-legend">
        {DASHBOARD_STATUSES.map((status) => {
          const value = countFor(counts, status)
          return (
            <li key={status}>
              <StatusIcon status={status} />
              <span>{t(`status.${status.toLowerCase()}`)}</span>
              <strong>{value}</strong>
            </li>
          )
        })}
      </ul>
    </div>
  )
}

function PassRateChart({ points }: { points: Dashboard['pass_rate_trend'] }) {
  const { t } = useI18n()
  if (points.length === 0) {
    return <p className="muted">{t('executions.noSlow')}</p>
  }
  const w = 320
  const h = 100
  const pad = 8
  const max = 100
  const xs = points.map((_, i) =>
    points.length === 1
      ? w / 2
      : pad + (i / (points.length - 1)) * (w - pad * 2),
  )
  const ys = points.map(
    (p) => pad + (1 - Math.min(100, Math.max(0, p.pass_rate)) / max) * (h - pad * 2),
  )
  const line = xs.map((x, i) => `${i === 0 ? 'M' : 'L'}${x},${ys[i]}`).join(' ')
  const bars =
    points.length === 1
      ? null
      : points.map((p, i) => {
          const barW = Math.max(4, (w - pad * 2) / points.length - 4)
          const barH = ((p.pass_rate || 0) / 100) * (h - pad * 2)
          return (
            <rect
              key={p.run_id}
              x={xs[i]! - barW / 2}
              y={h - pad - barH}
              width={barW}
              height={barH}
              fill="var(--status-passed)"
              opacity={0.35}
              rx={2}
            />
          )
        })

  return (
    <div className="exec-trend" aria-label={t('executions.passRateTrend')}>
      <svg width="100%" viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none">
        {bars}
        <path
          d={line}
          fill="none"
          stroke="var(--accent)"
          strokeWidth={2}
          strokeLinejoin="round"
          strokeLinecap="round"
        />
        {xs.map((x, i) => (
          <circle
            key={points[i]!.run_id}
            cx={x}
            cy={ys[i]}
            r={3}
            fill="var(--accent)"
          />
        ))}
      </svg>
      <div className="exec-trend-labels muted">
        {points.map((p) => (
          <span key={p.run_id} title={p.run_name}>
            {p.pass_rate.toFixed(0)}%
          </span>
        ))}
      </div>
    </div>
  )
}

function toLocalInput(iso: string | null): string {
  if (!iso) return ''
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`
}

function fromLocalInput(value: string): string | null {
  if (!value) return null
  const d = new Date(value)
  if (Number.isNaN(d.getTime())) return null
  return d.toISOString()
}

export function IterationDashboard({
  dashboard,
  canEdit,
  onSaveSchedule,
}: Props) {
  const { t } = useI18n()
  const { iteration, campaign_path, status_counts } = dashboard
  const [cadence, setCadence] = useState(iteration.cadence)
  const [start, setStart] = useState(toLocalInput(iteration.scheduled_start))
  const [end, setEnd] = useState(toLocalInput(iteration.scheduled_end))
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    setCadence(iteration.cadence)
    setStart(toLocalInput(iteration.scheduled_start))
    setEnd(toLocalInput(iteration.scheduled_end))
  }, [iteration.id, iteration.cadence, iteration.scheduled_start, iteration.scheduled_end])

  const save = async () => {
    if (!onSaveSchedule) return
    setSaving(true)
    try {
      await onSaveSchedule({
        cadence,
        scheduled_start: fromLocalInput(start),
        scheduled_end: fromLocalInput(end),
      })
    } finally {
      setSaving(false)
    }
  }

  return (
    <section
      className="exec-dashboard"
      aria-label={t('executions.dashboard')}
      {...testIdProps(TestIds.iterationDashboard)}
    >
      <header className="exec-dashboard-header">
        <div>
          <p className="muted exec-breadcrumb">
            {campaign_path.join(' / ')}
          </p>
          <h2 style={{ margin: '0.15rem 0' }}>
            {iteration.name}
            {iteration.reference ? (
              <span className="badge" style={{ marginLeft: '0.5rem' }}>
                {iteration.reference}
              </span>
            ) : null}
          </h2>
          <div className="meta-row">
            <StatusBadge status={iteration.status} />
            <span className="muted">{iteration.mode}</span>
            <span className="muted">{iteration.cadence || '—'}</span>
          </div>
        </div>
      </header>

      <div className="exec-dashboard-grid">
        <div className="panel exec-dash-panel">
          <h3>{t('executions.statusCounts')}</h3>
          <Donut counts={status_counts} />
        </div>

        <div className="panel exec-dash-panel">
          <h3>{t('executions.passRateTrend')}</h3>
          <PassRateChart points={dashboard.pass_rate_trend} />
        </div>

        <div className="panel exec-dash-panel">
          <h3>{t('executions.schedule')}</h3>
          <div className="form-col">
            <label>
              <span className="muted">{t('executions.cadence')}</span>
              <input
                className="search-input"
                value={cadence}
                disabled={!canEdit}
                onChange={(e) => setCadence(e.target.value)}
                placeholder="daily / on-push / manual"
                {...testIdProps(TestIds.iterationCadenceInput)}
              />
            </label>
            <label>
              <span className="muted">{t('executions.scheduledStart')}</span>
              <input
                type="datetime-local"
                className="filter-input"
                value={start}
                disabled={!canEdit}
                onChange={(e) => setStart(e.target.value)}
                {...testIdProps(TestIds.iterationScheduledStartInput)}
              />
            </label>
            <label>
              <span className="muted">{t('executions.scheduledEnd')}</span>
              <input
                type="datetime-local"
                className="filter-input"
                value={end}
                disabled={!canEdit}
                onChange={(e) => setEnd(e.target.value)}
                {...testIdProps(TestIds.iterationScheduledEndInput)}
              />
            </label>
            {canEdit && onSaveSchedule ? (
              <button
                type="button"
                className="btn"
                disabled={saving}
                onClick={() => void save()}
                {...testIdProps(TestIds.iterationSaveScheduleButton)}
              >
                {saving ? t('common.loading') : t('executions.saveSchedule')}
              </button>
            ) : null}
          </div>
        </div>

        <div className="panel exec-dash-panel">
          <h3>{t('executions.flaky')}</h3>
          {dashboard.flaky_tests.length === 0 ? (
            <p className="muted" {...testIdProps(TestIds.iterationFlakyEmpty)}>
              {t('executions.noFlaky')}
            </p>
          ) : (
            <ul className="exec-mini-list">
              {dashboard.flaky_tests.map((row, index) => (
                <li
                  key={row.case_id}
                  {...listItemProps(TestIds.iterationFlakyItem, {
                    entityId: row.case_id,
                    entityKey: isSafeEntityKey(row.case_key)
                      ? row.case_key
                      : undefined,
                    index,
                  })}
                >
                  <Link
                    to={`/cases/${row.case_id}`}
                    {...listChildProps(TestIds.iterationFlakyItem, 'link')}
                  >
                    <span className="badge">{row.case_key}</span> {row.case_title}
                  </Link>
                  <span className="muted">flips {row.flips}</span>
                  <StatusBadge status={row.latest_status} />
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="panel exec-dash-panel">
          <h3>{t('executions.slowest')}</h3>
          {dashboard.slowest_tests.length === 0 ? (
            <p className="muted" {...testIdProps(TestIds.iterationSlowEmpty)}>
              {t('executions.noSlow')}
            </p>
          ) : (
            <ul className="exec-mini-list">
              {dashboard.slowest_tests.map((row, index) => (
                <li
                  key={row.case_id}
                  {...listItemProps(TestIds.iterationSlowItem, {
                    entityId: row.case_id,
                    entityKey: isSafeEntityKey(row.case_key)
                      ? row.case_key
                      : undefined,
                    index,
                  })}
                >
                  <Link
                    to={`/cases/${row.case_id}`}
                    {...listChildProps(TestIds.iterationSlowItem, 'link')}
                  >
                    <span className="badge">{row.case_key}</span> {row.case_title}
                  </Link>
                  <span className="muted">
                    {(row.duration_ms / 1000).toFixed(1)}s
                  </span>
                  <StatusBadge status={row.status} />
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </section>
  )
}
