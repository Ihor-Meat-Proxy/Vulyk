import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import {
  api,
  type AutomatedSuite,
  type AutomatedSuitesParams,
} from '../api'
import { useAuth } from '../auth'
import { useI18n } from '../i18n'
import { formatTimestamp } from '../timezone'
import { StatusBadge } from './StatusBadge'

type Props = {
  projectId: string
  iterationId: string
  canDelete?: boolean
  canRerun?: boolean
  onOpenDetails: (suite: AutomatedSuite) => void
  refreshKey?: number
}

function formatDuration(ms: number | null): string {
  if (ms == null) return '—'
  if (ms < 1000) return `${ms}ms`
  const s = ms / 1000
  if (s < 60) return `${s.toFixed(1)}s`
  return `${Math.floor(s / 60)}m ${Math.round(s % 60)}s`
}

export function AutomatedSuitesTable({
  projectId,
  iterationId,
  canDelete,
  canRerun,
  onOpenDetails,
  refreshKey = 0,
}: Props) {
  const { t } = useI18n()
  const { user, status: authStatus } = useAuth()
  const [items, setItems] = useState<AutomatedSuite[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [q, setQ] = useState('')
  const [status, setStatus] = useState('')
  const [createdBy, setCreatedBy] = useState('')
  const [sort, setSort] = useState('created_at_desc')
  const [offset, setOffset] = useState(0)
  const limit = 25
  const [busyId, setBusyId] = useState<string | null>(null)

  const fmt = (value: string | null | undefined) =>
    formatTimestamp(value, {
      timeZone: user?.timezone,
      instanceTimeZone: authStatus?.default_timezone,
      locale: user?.locale ?? authStatus?.default_locale,
    })

  const load = async (nextOffset = offset) => {
    setLoading(true)
    try {
      const params: AutomatedSuitesParams = {
        limit,
        offset: nextOffset,
        sort,
        q: q.trim() || undefined,
        status: status || undefined,
        created_by: createdBy.trim() || undefined,
      }
      const page = await api.listIterationSuites(projectId, iterationId, params)
      setItems(page.items)
      setTotal(page.total)
      setOffset(nextOffset)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load(0)
    // eslint-disable-next-line react-hooks/exhaustive-deps -- reload on filters / iteration
  }, [projectId, iterationId, sort, refreshKey])

  const onDelete = async (suite: AutomatedSuite) => {
    if (!canDelete) return
    if (!window.confirm(t('executions.deleteConfirm'))) return
    setBusyId(suite.id)
    try {
      await api.deleteSuiteRun(projectId, suite.id)
      await load(offset)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusyId(null)
    }
  }

  const onRerun = async (suite: AutomatedSuite) => {
    if (!canRerun) return
    setBusyId(suite.id)
    try {
      await api.rerunSuite(projectId, suite.id)
      await load(0)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusyId(null)
    }
  }

  const reportHref = (suite: AutomatedSuite) => {
    const art = suite.artifacts as unknown
    if (Array.isArray(art)) {
      const report = art.find(
        (a) =>
          a &&
          typeof a === 'object' &&
          ('kind' in a
            ? (a as { kind?: string }).kind === 'report' ||
              (a as { kind?: string }).kind === 'html'
            : false) &&
          typeof (a as { url?: string }).url === 'string',
      ) as { url?: string } | undefined
      if (report?.url) return report.url
      const anyUrl = art.find(
        (a) => a && typeof a === 'object' && typeof (a as { url?: string }).url === 'string',
      ) as { url?: string } | undefined
      return anyUrl?.url ?? null
    }
    if (art && typeof art === 'object') {
      const obj = art as Record<string, unknown>
      return (
        (typeof obj.html_report === 'string' && obj.html_report) ||
        (typeof obj.report_url === 'string' && obj.report_url) ||
        (typeof obj.url === 'string' && obj.url) ||
        null
      )
    }
    return null
  }

  return (
    <section className="panel" aria-label={t('executions.suites')}>
      <div className="panel-header">
        <h2>{t('executions.suites')}</h2>
      </div>

      <div className="form-row" style={{ marginBottom: '0.75rem' }}>
        <input
          className="search-input"
          placeholder={t('executions.search')}
          value={q}
          onChange={(e) => setQ(e.target.value)}
          aria-label={t('executions.search')}
        />
        <select
          className="filter-input"
          value={status}
          onChange={(e) => setStatus(e.target.value)}
          aria-label={t('executions.filterStatus')}
        >
          <option value="">{t('cases.allStatuses')}</option>
          <option value="Ready">Ready</option>
          <option value="Running">Running</option>
          <option value="Passed">Passed</option>
          <option value="Failed">Failed</option>
          <option value="Blocked">Blocked</option>
          <option value="Skipped">Skipped</option>
          <option value="Untestable">Untestable</option>
        </select>
        <input
          className="filter-input"
          placeholder={t('executions.filterCreatedBy')}
          value={createdBy}
          onChange={(e) => setCreatedBy(e.target.value)}
          aria-label={t('executions.filterCreatedBy')}
        />
        <select
          className="filter-input"
          value={sort}
          onChange={(e) => setSort(e.target.value)}
          aria-label="Sort"
        >
          <option value="created_at_desc">Newest</option>
          <option value="created_at_asc">Oldest</option>
          <option value="name_asc">Name A–Z</option>
          <option value="name_desc">Name Z–A</option>
          <option value="status_asc">Status</option>
        </select>
        <button
          type="button"
          className="btn secondary"
          disabled={loading}
          onClick={() => void load(0)}
        >
          {t('executions.applyFilters')}
        </button>
      </div>

      {error ? <p className="error">{error}</p> : null}
      {loading && items.length === 0 ? (
        <p className="muted">{t('common.loading')}</p>
      ) : items.length === 0 ? (
        <p className="muted">{t('executions.noSuites')}</p>
      ) : (
        <>
          <div className="table-wrap">
            <table className="table">
              <thead>
                <tr>
                  <th>{t('executions.colName')}</th>
                  <th>{t('executions.colStatus')}</th>
                  <th>{t('executions.colCreatedBy')}</th>
                  <th>{t('executions.colCreated')}</th>
                  <th>{t('executions.colDuration')}</th>
                  <th>{t('executions.colCounts')}</th>
                  <th>{t('executions.colActions')}</th>
                </tr>
              </thead>
              <tbody>
                {items.map((suite) => {
                  const report = reportHref(suite)
                  return (
                    <tr key={suite.id}>
                      <td>
                        <Link to={`/executions/runs/${suite.id}`}>
                          {suite.name}
                        </Link>
                      </td>
                      <td>
                        <StatusBadge status={suite.status} />
                      </td>
                      <td>{suite.created_by_name || '—'}</td>
                      <td>{fmt(suite.created_at)}</td>
                      <td>{formatDuration(suite.duration_ms)}</td>
                      <td className="muted">
                        {suite.pass_count}/{suite.fail_count}/{suite.skip_count}/
                        {suite.blocked_count}
                      </td>
                      <td>
                        <div className="exec-row-actions">
                          <button
                            type="button"
                            className="btn secondary"
                            onClick={() => onOpenDetails(suite)}
                          >
                            {t('executions.details')}
                          </button>
                          {report ? (
                            <a
                              className="btn secondary"
                              href={report}
                              target="_blank"
                              rel="noreferrer"
                            >
                              {t('executions.report')}
                            </a>
                          ) : null}
                          {canRerun ? (
                            <button
                              type="button"
                              className="btn secondary"
                              disabled={busyId === suite.id}
                              onClick={() => void onRerun(suite)}
                            >
                              {t('executions.rerun')}
                            </button>
                          ) : null}
                          {canDelete ? (
                            <button
                              type="button"
                              className="btn secondary"
                              disabled={busyId === suite.id}
                              onClick={() => void onDelete(suite)}
                            >
                              {t('executions.delete')}
                            </button>
                          ) : null}
                        </div>
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>
          <div className="pager">
            <button
              type="button"
              className="btn secondary"
              disabled={offset <= 0 || loading}
              onClick={() => void load(Math.max(0, offset - limit))}
            >
              ←
            </button>
            <span className="muted">
              {offset + 1}–{Math.min(offset + limit, total)} / {total}
            </span>
            <button
              type="button"
              className="btn secondary"
              disabled={loading || offset + limit >= total}
              onClick={() => void load(offset + limit)}
            >
              →
            </button>
          </div>
        </>
      )}
    </section>
  )
}
