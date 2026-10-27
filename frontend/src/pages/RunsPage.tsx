import { useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { api, isAbortError, type Suite, type TestPlan, type TestRun } from '../api'
import { useAuth } from '../auth'
import { StatusBadge } from '../components/StatusBadge'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { useI18n } from '../i18n'
import { TestIds, listChildProps, listItemProps, testIdProps } from '../testing'
import { formatTimestamp } from '../timezone'

/** Page size for runs list + load-more (review #46). */
export const RUNS_PAGE_SIZE = 50

type Props = {
  projectId: string | null
  suites: Suite[]
}

export function RunsPage({ projectId, suites }: Props) {
  const navigate = useNavigate()
  const { t } = useI18n()
  const { user, status } = useAuth()
  const { can } = useProjectPermissions(projectId)
  const canExecute = can('execute_runs')
  const canUpload = can('upload_results')
  const canPlans = can('manage_plans')
  const fmt = (value: string | null | undefined) =>
    formatTimestamp(value, {
      timeZone: user?.timezone,
      instanceTimeZone: status?.default_timezone,
      locale: user?.locale ?? status?.default_locale,
    })

  const [runs, setRuns] = useState<TestRun[]>([])
  const [runsTotal, setRunsTotal] = useState(0)
  const [plans, setPlans] = useState<TestPlan[]>([])
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [loadingMore, setLoadingMore] = useState(false)
  const [name, setName] = useState('')
  const [planId, setPlanId] = useState('')
  const [suiteId, setSuiteId] = useState('')
  const [creating, setCreating] = useState(false)
  const [planName, setPlanName] = useState('')
  const [uploadRunId, setUploadRunId] = useState('')
  const [uploading, setUploading] = useState(false)
  const [uploadMessage, setUploadMessage] = useState<string | null>(null)

  const applyRunsPage = (runPage: { items: TestRun[]; total: number }, append: boolean) => {
    setRuns((prev) => (append ? [...prev, ...runPage.items] : runPage.items))
    setRunsTotal(runPage.total)
  }

  useEffect(() => {
    if (!projectId) {
      setRuns([])
      setRunsTotal(0)
      setPlans([])
      return
    }
    const ac = new AbortController()
    setLoading(true)
    Promise.all([
      api.listRuns(projectId, RUNS_PAGE_SIZE, 0, { signal: ac.signal }),
      api.listPlans(projectId),
    ])
      .then(([runPage, planList]) => {
        if (ac.signal.aborted) return
        applyRunsPage(runPage, false)
        setPlans(planList)
        setError(null)
      })
      .catch((err: Error) => {
        if (ac.signal.aborted || isAbortError(err)) return
        setError(err.message)
      })
      .finally(() => {
        if (!ac.signal.aborted) setLoading(false)
      })
    return () => {
      ac.abort()
    }
  }, [projectId])

  const loadMoreRuns = async () => {
    if (!projectId || loadingMore || runs.length >= runsTotal) return
    setLoadingMore(true)
    try {
      const runPage = await api.listRuns(
        projectId,
        RUNS_PAGE_SIZE,
        runs.length,
      )
      applyRunsPage(runPage, true)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoadingMore(false)
    }
  }

  const createRun = async () => {
    if (!projectId || !name.trim()) return
    setCreating(true)
    try {
      const body: Record<string, unknown> = {
        name: name.trim(),
        start: true,
      }
      if (planId) body.plan_id = planId
      if (suiteId) body.suite_ids = [suiteId]
      if (!planId && !suiteId) body.include_all_cases = true
      const detail = await api.createRun(projectId, body)
      setName('')
      const runPage = await api.listRuns(projectId, RUNS_PAGE_SIZE, 0)
      applyRunsPage(runPage, false)
      navigate(`/runs/${detail.run.id}`)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setCreating(false)
    }
  }

  const uploadJunit = async (file: File) => {
    if (!projectId || !uploadRunId) return
    setUploading(true)
    setUploadMessage(null)
    try {
      const content = await file.text()
      const result = await api.ingestRun(uploadRunId, {
        format: 'junit',
        content,
        source: 'ui',
      })
      setUploadMessage(
        t('runs.ingestedResults', {
          parsed: result.parsed,
          inserted: result.inserted,
        }),
      )
      const runPage = await api.listRuns(projectId, RUNS_PAGE_SIZE, 0)
      applyRunsPage(runPage, false)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setUploading(false)
    }
  }

  const createPlan = async () => {
    if (!projectId || !planName.trim()) return
    try {
      await api.createPlan(projectId, { name: planName.trim() })
      setPlanName('')
      setPlans(await api.listPlans(projectId))
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  if (!projectId) {
    return (
      <div className="runs-page empty" {...testIdProps(TestIds.runsPage)}>
        {t('runs.selectProject')}
      </div>
    )
  }

  return (
    <div className="runs-page" {...testIdProps(TestIds.runsPage)}>
      {canExecute && (
      <section className="panel" style={{ marginBottom: '1rem' }}>
        <h2 style={{ marginTop: 0 }}>{t('runs.startTitle')}</h2>
        <p className="muted" style={{ marginTop: 0 }}>
          {t('runs.startHelp')}
        </p>
        <div className="form-row">
          <input
            className="search-input"
            placeholder={t('runs.namePlaceholder')}
            value={name}
            onChange={(e) => setName(e.target.value)}
            {...testIdProps(TestIds.runsCreateNameInput)}
          />
          <select
            className="filter-input"
            value={planId}
            onChange={(e) => setPlanId(e.target.value)}
            {...testIdProps(TestIds.runsPlanSelect)}
          >
            <option value="">{t('runs.noPlan')}</option>
            {plans.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
          <select
            className="filter-input"
            value={suiteId}
            onChange={(e) => setSuiteId(e.target.value)}
            {...testIdProps(TestIds.runsSuiteSelect)}
          >
            <option value="">{t('runs.allSuites')}</option>
            {suites.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
          <button
            type="button"
            className="btn"
            disabled={creating || !name.trim()}
            onClick={() => void createRun()}
            {...testIdProps(TestIds.runsCreateButton)}
          >
            {creating ? t('runs.creating') : t('runs.create')}
          </button>
        </div>
      </section>
      )}

      {canUpload && (
      <section className="panel" style={{ marginBottom: '1rem' }}>
        <h3 style={{ marginTop: 0 }}>{t('runs.uploadJunit')}</h3>
        <p className="muted" style={{ marginTop: 0 }}>
          {t('runs.uploadHelp')}
        </p>
        <div className="form-row">
          <select
            className="filter-input"
            value={uploadRunId}
            onChange={(e) => setUploadRunId(e.target.value)}
            aria-label={t('a11y.uploadTargetRun')}
            {...testIdProps(TestIds.runsUploadRunSelect)}
          >
            <option value="">{t('runs.selectRun')}</option>
            {runs.map((r) => (
              <option key={r.id} value={r.id}>
                {r.name}
              </option>
            ))}
          </select>
          <input
            type="file"
            accept=".xml,text/xml,application/xml"
            aria-label={t('a11y.junitXmlFile')}
            disabled={!uploadRunId || uploading}
            onChange={(e) => {
              const file = e.target.files?.[0]
              if (file) void uploadJunit(file)
              e.target.value = ''
            }}
            {...testIdProps(TestIds.runsUploadFileInput)}
          />
        </div>
        {uploadMessage && <p className="muted">{uploadMessage}</p>}
      </section>
      )}

      <section className="panel" style={{ marginBottom: '1rem' }}>
        <h3 style={{ marginTop: 0 }}>{t('runs.plans')}</h3>
        {canPlans && (
        <div className="form-row">
          <input
            className="search-input"
            placeholder={t('runs.planNamePlaceholder')}
            value={planName}
            onChange={(e) => setPlanName(e.target.value)}
            {...testIdProps(TestIds.runsPlanNameInput)}
          />
          <button
            type="button"
            className="btn secondary"
            onClick={() => void createPlan()}
            {...testIdProps(TestIds.runsPlanAddButton)}
          >
            {t('runs.addPlan')}
          </button>
        </div>
        )}
        {plans.length === 0 ? (
          <p className="muted">{t('runs.noPlans')}</p>
        ) : (
          <ul className="history-list">
            {plans.map((p) => (
              <li key={p.id}>
                <strong>{p.name}</strong>{' '}
                <span className="muted">· {fmt(p.updated_at)}</span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="panel">
        <h3 style={{ marginTop: 0 }}>{t('runs.recent')}</h3>
        {error && (
          <p className="error" {...testIdProps(TestIds.runsError)}>
            {error}
          </p>
        )}
        {loading && (
          <p className="muted" {...testIdProps(TestIds.runsLoading)}>
            {t('common.loading')}
          </p>
        )}
        {!loading && runs.length === 0 && (
          <div className="empty" {...testIdProps(TestIds.runsEmpty)}>
            {t('runs.empty')}
          </div>
        )}
        {runs.length > 0 && (
          <>
            <table className="table" {...testIdProps(TestIds.runsList)}>
              <thead>
                <tr>
                  <th>{t('runs.colName')}</th>
                  <th>{t('runs.colStatus')}</th>
                  <th>{t('runs.colStarted')}</th>
                </tr>
              </thead>
              <tbody>
                {runs.map((r, index) => (
                  <tr
                    key={r.id}
                    {...listItemProps(TestIds.runRow, {
                      // UUID only — run names are freeform (spaces) and must not be entity keys
                      entityId: r.id,
                      index,
                    })}
                  >
                    <td {...testIdProps(TestIds.runRowName)}>
                      <Link
                        to={`/runs/${r.id}`}
                        {...listChildProps(TestIds.runRow, 'link')}
                      >
                        {r.name}
                      </Link>
                    </td>
                    <td {...testIdProps(TestIds.runRowStatus)}>
                      <StatusBadge status={r.status} testId="runs-page-status-badge" />
                    </td>
                    <td className="muted">{fmt(r.started_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {runs.length < runsTotal && (
              <div style={{ marginTop: '0.75rem' }}>
                <button
                  type="button"
                  className="btn secondary"
                  disabled={loadingMore}
                  onClick={() => void loadMoreRuns()}
                  {...testIdProps(TestIds.runsLoadMoreButton)}
                >
                  {loadingMore
                    ? t('common.loading')
                    : `${t('runs.loadMore')} (${runs.length} / ${runsTotal})`}
                </button>
              </div>
            )}
          </>
        )}
      </section>
    </div>
  )
}
