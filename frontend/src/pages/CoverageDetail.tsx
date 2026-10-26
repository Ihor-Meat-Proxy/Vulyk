import { useCallback, useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { api, type RequirementCoverageDetail } from '../api'
import { StatusBadge } from '../components/StatusBadge'
import { useBackspaceNavigate } from '../hooks/useBackspaceNavigate'
import { useI18n } from '../i18n'
import { TestIds, listItemProps, testIdProps } from '../testing'

type Props = {
  projectId: string | null
}

export function CoverageDetail({ projectId }: Props) {
  const { t } = useI18n()
  const navigate = useNavigate()
  const [data, setData] = useState<RequirementCoverageDetail | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const goBack = useCallback(() => {
    navigate('/reports')
  }, [navigate])
  useBackspaceNavigate(goBack)

  const refresh = useCallback(async () => {
    if (!projectId) {
      setData(null)
      return
    }
    setBusy(true)
    try {
      setData(await api.getCoverageDetail(projectId))
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

  return (
    <div
      className="reports-page coverage-detail-page"
      {...testIdProps(TestIds.reportsCoveragePage)}
    >
      <nav
        className="breadcrumbs"
        aria-label={t('reports.breadcrumbs')}
        {...testIdProps(TestIds.reportsCoverageBreadcrumbs)}
      >
        <Link
          to="/reports"
          {...testIdProps(TestIds.reportsCoverageBreadcrumbReports)}
        >
          {t('reports.title')}
        </Link>
        <span aria-hidden>/</span>
        <span>{t('reports.coverageTitle')}</span>
      </nav>

      <div className="panel-header">
        <div>
          <h2 {...testIdProps(TestIds.reportsCoverageTitle)}>
            {t('reports.coverageTitle')}
          </h2>
          <p className="muted">{t('reports.coverageSubtitle')}</p>
        </div>
        <div className="topbar-actions">
          <button
            type="button"
            className="btn secondary"
            onClick={goBack}
            {...testIdProps(TestIds.reportsCoverageBack)}
          >
            {t('reports.back')}
          </button>
          <button
            type="button"
            className="btn secondary"
            onClick={() => void refresh()}
            disabled={busy}
            {...testIdProps(TestIds.reportsCoverageRefresh)}
          >
            {t('reports.refresh')}
          </button>
        </div>
      </div>

      {error && <p className="error">{error}</p>}

      {data && (
        <section className="report-kpis" aria-label={t('reports.kpis')}>
          <div className="kpi">
            <span className="kpi-label">{t('reports.kpiRequirements')}</span>
            <span className="kpi-value">{data.total_requirements}</span>
          </div>
          <div className="kpi">
            <span className="kpi-label">{t('reports.covered')}</span>
            <span className="kpi-value">{data.covered_requirements}</span>
          </div>
          <div className="kpi">
            <span className="kpi-label">{t('reports.uncovered')}</span>
            <span className="kpi-value">{data.uncovered_requirements}</span>
          </div>
          <div className="kpi">
            <span className="kpi-label">{t('reports.kpiCoverage')}</span>
            <span className="kpi-value">
              {data.coverage_percent.toFixed(1)}%
            </span>
          </div>
        </section>
      )}

      <div className="reports-grid">
        <section
          className="panel"
          {...testIdProps(TestIds.reportsCoverageCovered)}
        >
          <h3>{t('reports.covered')}</h3>
          {!data?.covered.length && (
            <p className="muted">{t('reports.noCoverageRows')}</p>
          )}
          <ul className="coverage-req-list">
            {data?.covered.map((r, idx) => (
              <li
                key={r.requirement_id}
                {...listItemProps(TestIds.reportsCoverageReq, {
                  entityId: r.requirement_id,
                  index: idx,
                })}
              >
                <strong>
                  {r.external_key ? `${r.external_key} · ` : ''}
                  {r.title}
                </strong>
                <span className="muted">
                  {r.linked_cases.length} · {r.pass_percent.toFixed(1)}%
                </span>
                <ul className="coverage-case-list">
                  {r.linked_cases.map((c) => (
                    <li key={c.case_id}>
                      <Link to={`/cases/${encodeURIComponent(c.case_key)}`}>
                        {c.case_key}
                      </Link>{' '}
                      {c.title}{' '}
                      {c.latest_status ? (
                        <StatusBadge status={c.latest_status} />
                      ) : (
                        <span className="muted">
                          {t('reports.noLatestStatus')}
                        </span>
                      )}
                    </li>
                  ))}
                </ul>
              </li>
            ))}
          </ul>
        </section>

        <section
          className="panel"
          {...testIdProps(TestIds.reportsCoverageUncovered)}
        >
          <h3>{t('reports.uncovered')}</h3>
          {!data?.uncovered.length && (
            <p className="muted">{t('reports.noUncovered')}</p>
          )}
          <ul className="coverage-req-list">
            {data?.uncovered.map((r, idx) => (
              <li
                key={r.requirement_id}
                {...listItemProps(TestIds.reportsCoverageReq, {
                  entityId: r.requirement_id,
                  index: idx,
                })}
              >
                <strong>
                  {r.external_key ? `${r.external_key} · ` : ''}
                  {r.title}
                </strong>
                <span className="muted">{r.source}</span>
              </li>
            ))}
          </ul>
        </section>
      </div>
    </div>
  )
}
