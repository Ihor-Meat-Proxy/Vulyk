import { useCallback, useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import {
  api,
  type FlakyReport,
  type PassFailTrends,
  type ProjectDashboard,
  type RequirementCoverageReport,
  type RunSummaryReport,
} from '../api'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { useI18n } from '../i18n'
import { TestIds, listItemProps, testIdProps } from '../testing'

type Props = {
  projectId: string | null
}

function pct(n: number) {
  return `${n.toFixed(1)}%`
}

function CountsBar({
  passed,
  failed,
  other,
  runId,
  onSegmentClick,
  segPassedId,
  segFailedId,
  segOtherId,
}: {
  passed: number
  failed: number
  other: number
  runId?: string | null
  onSegmentClick?: (
    status: 'passed' | 'failed' | 'other',
    runId: string,
  ) => void
  segPassedId: string
  segFailedId: string
  segOtherId: string
}) {
  const { t } = useI18n()
  const total = Math.max(passed + failed + other, 1)
  const clickable = Boolean(runId && onSegmentClick)

  const seg = (
    status: 'passed' | 'failed' | 'other',
    count: number,
    className: string,
    testId: string,
  ) => {
    const width = count === 0 ? '0%' : `${(count / total) * 100}%`
    const entityProps =
      runId != null
        ? listItemProps(testId, { entityId: runId, index: 0 })
        : testIdProps(testId)
    if (!clickable || !runId || count === 0) {
      return (
        <span
          className={`trend-seg ${className}`}
          style={{ width }}
          {...entityProps}
        />
      )
    }
    return (
      <button
        type="button"
        className={`trend-seg ${className} clickable`}
        style={{ width }}
        aria-label={`${status} ${count}`}
        {...entityProps}
        onClick={(e) => {
          e.stopPropagation()
          onSegmentClick?.(status, runId)
        }}
      />
    )
  }

  return (
    <div>
      <div
        className="trend-bar"
        role="img"
        aria-label={`${t('status.passed')} ${passed}, ${t('status.failed')} ${failed}, ${t('reports.other')} ${other}`}
      >
        {seg('passed', passed, 'passed', segPassedId)}
        {seg('failed', failed, 'failed', segFailedId)}
        {seg('other', other, 'other', segOtherId)}
      </div>
      <div className="progress-legend">
        <span className="progress-legend-item">
          <span className="progress-legend-swatch passed" aria-hidden />
          {t('status.passed')} {passed}
        </span>
        <span className="progress-legend-item">
          <span className="progress-legend-swatch failed" aria-hidden />
          {t('status.failed')} {failed}
        </span>
        <span className="progress-legend-item">
          <span className="progress-legend-swatch skipped" aria-hidden />
          {t('reports.other')} {other}
        </span>
      </div>
    </div>
  )
}

export function Reports({ projectId }: Props) {
  const { t } = useI18n()
  const navigate = useNavigate()
  const { can } = useProjectPermissions(projectId)
  const canExport = can('export_reports')
  const [dash, setDash] = useState<ProjectDashboard | null>(null)
  const [trends, setTrends] = useState<PassFailTrends | null>(null)
  const [coverage, setCoverage] = useState<RequirementCoverageReport | null>(
    null,
  )
  const [flaky, setFlaky] = useState<FlakyReport | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const openRun = useCallback(
    (runId: string, status?: string) => {
      const qs = status ? `?status=${encodeURIComponent(status)}` : ''
      navigate(`/reports/runs/${runId}${qs}`)
    },
    [navigate],
  )

  const onSegmentClick = useCallback(
    (bucket: 'passed' | 'failed' | 'other', runId: string) => {
      if (bucket === 'other') {
        openRun(runId)
        return
      }
      openRun(runId, bucket)
    },
    [openRun],
  )

  const refresh = useCallback(async () => {
    if (!projectId) {
      setDash(null)
      setTrends(null)
      setCoverage(null)
      setFlaky(null)
      return
    }
    setBusy(true)
    try {
      const [d, tnd, c, f] = await Promise.all([
        api.getDashboard(projectId),
        api.getTrends(projectId, 'run', 12),
        api.getCoverage(projectId),
        api.getFlaky(projectId),
      ])
      setDash(d)
      setTrends(tnd)
      setCoverage(c)
      setFlaky(f)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }, [projectId])

  useEffect(() => {
    void refresh()
  }, [refresh])

  if (!projectId) {
    return <p className="muted">{t('reports.selectProject')}</p>
  }

  const onExport = async (
    format: 'csv' | 'pdf',
    report = 'dashboard',
    runId?: string,
  ) => {
    try {
      await api.downloadReportExport(projectId, { format, report, runId })
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <div className="reports-page" {...testIdProps(TestIds.reportsPage)}>
      <div className="panel-header">
        <div>
          <h2 {...testIdProps(TestIds.reportsTitle)}>{t('reports.title')}</h2>
          <p className="muted">{t('reports.subtitle')}</p>
        </div>
        <div className="topbar-actions">
          <button
            type="button"
            className="btn secondary"
            onClick={() => void refresh()}
            disabled={busy}
            {...testIdProps(TestIds.reportsRefreshButton)}
          >
            {t('reports.refresh')}
          </button>
          {canExport && (
            <button
              type="button"
              className="btn secondary"
              onClick={() => void onExport('csv')}
              {...testIdProps(TestIds.reportsExportButton)}
            >
              {t('reports.exportCsv')}
            </button>
          )}
          {canExport && (
            <button
              type="button"
              className="btn"
              onClick={() => void onExport('pdf')}
              {...testIdProps(TestIds.reportsExportPdfButton)}
            >
              {t('reports.exportPdf')}
            </button>
          )}
        </div>
      </div>

      {error && (
        <p className="error" {...testIdProps(TestIds.reportsError)}>
          {error}
        </p>
      )}

      {busy && !dash && (
        <p className="muted" {...testIdProps(TestIds.reportsLoading)}>
          {t('common.loading')}
        </p>
      )}

      {dash && (
        <section
          className="report-kpis"
          aria-label={t('reports.kpis')}
          {...testIdProps(TestIds.reportsKpis)}
        >
          <Link
            to="/"
            className="kpi kpi-link"
            {...testIdProps(TestIds.reportsKpiCases)}
          >
            <span className="kpi-label">{t('reports.kpiCases')}</span>
            <span className="kpi-value">{dash.total_cases}</span>
          </Link>
          <Link
            to="/runs"
            className="kpi kpi-link"
            {...testIdProps(TestIds.reportsKpiRuns)}
          >
            <span className="kpi-label">{t('reports.kpiRuns')}</span>
            <span className="kpi-value">{dash.total_runs}</span>
          </Link>
          <Link
            to="/reports/coverage"
            className="kpi kpi-link"
            {...testIdProps(TestIds.reportsKpiRequirements)}
          >
            <span className="kpi-label">{t('reports.kpiRequirements')}</span>
            <span className="kpi-value">{dash.total_requirements}</span>
          </Link>
          <Link
            to="/reports/coverage"
            className="kpi kpi-link"
            {...testIdProps(TestIds.reportsKpiCoverage)}
          >
            <span className="kpi-label">{t('reports.kpiCoverage')}</span>
            <span className="kpi-value">
              {pct(dash.coverage.coverage_percent)}
            </span>
          </Link>
        </section>
      )}

      <div className="reports-grid">
        <section className="panel" {...testIdProps(TestIds.reportsSummaries)}>
          <h3>{t('reports.recentSummaries')}</h3>
          {!dash?.recent_run_summaries.length && (
            <p className="muted" {...testIdProps(TestIds.reportsEmpty)}>
              {t('reports.noRuns')}
            </p>
          )}
          <ul className="report-list">
            {dash?.recent_run_summaries.map((s: RunSummaryReport, idx) => (
              <li key={s.run_id}>
                <button
                  type="button"
                  {...listItemProps(TestIds.reportsRunSummaryItem, {
                    entityId: s.run_id,
                    index: idx,
                  })}
                  onClick={() => openRun(s.run_id)}
                >
                  <strong>{s.name}</strong>
                  <span className="muted">
                    {pct(s.pass_rate)} · {s.counts.passed}P / {s.counts.failed}F
                  </span>
                  <CountsBar
                    passed={s.counts.passed}
                    failed={s.counts.failed + s.counts.error}
                    other={
                      s.counts.skipped + s.counts.blocked + s.counts.untested
                    }
                    runId={s.run_id}
                    onSegmentClick={onSegmentClick}
                    segPassedId={TestIds.reportsSummarySegPassed}
                    segFailedId={TestIds.reportsSummarySegFailed}
                    segOtherId={TestIds.reportsSummarySegOther}
                  />
                </button>
              </li>
            ))}
          </ul>
        </section>

        <section className="panel" {...testIdProps(TestIds.reportsTrends)}>
          <h3>{t('reports.trends')}</h3>
          {!trends?.points.length && (
            <p className="muted">{t('reports.noTrends')}</p>
          )}
          <ul className="trend-list">
            {trends?.points.map((p, idx) => (
              <li key={`${p.bucket}-${p.run_id ?? idx}`}>
                <button
                  type="button"
                  className="trend-row-btn"
                  disabled={!p.run_id}
                  {...listItemProps(TestIds.reportsTrendItem, {
                    entityId: p.run_id ?? `bucket-${idx}`,
                    index: idx,
                  })}
                  onClick={() => p.run_id && openRun(p.run_id)}
                >
                  <div className="trend-row">
                    <span>{p.run_name ?? p.bucket}</span>
                    <span className="muted">{pct(p.pass_rate)}</span>
                  </div>
                  <CountsBar
                    passed={p.counts.passed}
                    failed={p.counts.failed + p.counts.error}
                    other={p.counts.skipped + p.counts.blocked}
                    runId={p.run_id}
                    onSegmentClick={onSegmentClick}
                    segPassedId={TestIds.reportsTrendSegPassed}
                    segFailedId={TestIds.reportsTrendSegFailed}
                    segOtherId={TestIds.reportsTrendSegOther}
                  />
                </button>
              </li>
            ))}
          </ul>
          <button
            type="button"
            className="btn secondary"
            onClick={() => void onExport('csv', 'trends')}
            {...testIdProps(TestIds.reportsExportTrendsButton)}
          >
            {t('reports.exportTrendsCsv')}
          </button>
        </section>

        <section
          className="panel"
          {...testIdProps(TestIds.reportsCoveragePreview)}
        >
          <h3>{t('reports.coverageByRequirement')}</h3>
          {!coverage?.rows.length && (
            <p className="muted">{t('reports.noRequirements')}</p>
          )}
          <table className="data-table">
            <thead>
              <tr>
                <th>{t('reports.colRequirement')}</th>
                <th>{t('reports.colLinked')}</th>
                <th>{t('status.passed')}</th>
                <th>{t('status.failed')}</th>
                <th>{t('reports.passRate')}</th>
              </tr>
            </thead>
            <tbody>
              {coverage?.rows.map((r) => (
                <tr key={r.requirement_id}>
                  <td>
                    {r.external_key ? `${r.external_key} · ` : ''}
                    {r.title}
                  </td>
                  <td>{r.linked_cases}</td>
                  <td>{r.cases_passed}</td>
                  <td>{r.cases_failed}</td>
                  <td>{pct(r.pass_percent)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="topbar-actions">
            <Link
              to="/reports/coverage"
              className="btn secondary"
              {...testIdProps(TestIds.reportsCoverageDrilldown)}
            >
              {t('reports.openCoverage')}
            </Link>
            <button
              type="button"
              className="btn secondary"
              onClick={() => void onExport('csv', 'coverage')}
              {...testIdProps(TestIds.reportsExportCoverageButton)}
            >
              {t('reports.exportCoverageCsv')}
            </button>
          </div>
        </section>

        <section className="panel" {...testIdProps(TestIds.reportsFlaky)}>
          <h3>{t('reports.flaky')}</h3>
          <p className="muted">
            {t('reports.flakyHelp', {
              count: String(flaky?.lookback_runs ?? '—'),
            })}
          </p>
          {!flaky?.cases.length && (
            <p className="muted">{t('reports.noFlaky')}</p>
          )}
          <ul className="report-list">
            {flaky?.cases.map((c) => (
              <li key={c.case_id}>
                <strong>
                  <Link to={`/cases/${encodeURIComponent(c.case_key)}`}>
                    {c.case_key}
                  </Link>{' '}
                  — {c.title}
                </strong>
                <span className="muted">
                  flake {c.flake_score.toFixed(2)} · {c.passes}P / {c.failures}F
                  over {c.runs_seen} runs · {c.last_statuses.join(' → ')}
                </span>
              </li>
            ))}
          </ul>
          <button
            type="button"
            className="btn secondary"
            onClick={() => void onExport('csv', 'flaky')}
            {...testIdProps(TestIds.reportsExportFlakyButton)}
          >
            {t('reports.exportFlakyCsv')}
          </button>
        </section>
      </div>
    </div>
  )
}
