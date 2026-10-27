import { useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { api, type Suite, type TestPlan, type TestRun } from '../api'
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
    let cancelled = false
    setLoading(true)
    Promise.all([
      api.listRuns(projectId, RUNS_PAGE_SIZE, 0),
      api.listPlans(projectId),
    ])
      .then(([runPage, planList]) => {
        if (cancelled) return
        applyRunsPage(runPage, false)
        setPlans(planList)
        setError(null)
      })
      .catch((err: Error) => {
        if (!cancelled) setError(err.message)
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
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
        `Ingested ${result.parsed} results (${result.inserted} inserted).`,
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
        Select a project to manage runs.
      </div>
    )
  }

  return (
    <div className="runs-page" {...testIdProps(TestIds.runsPage)}>
      {canExecute && (
      <section className="panel" style={{ marginBottom: '1rem' }}>
        <h2 style={{ marginTop: 0 }}>Start a run</h2>
        <p className="muted" style={{ marginTop: 0 }}>
          Create from a plan, suite filter, or all cases — TestRail-style execution queue.
        </p>
        <div className="form-row">
          <input
            className="search-input"
            placeholder="Run name"
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
            <option value="">No plan</option>
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
            <option value="">All suites</option>
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
            {creating ? 'Creating…' : 'Create run'}
          </button>
        </div>
      </section>
      )}

      {canUpload && (
      <section className="panel" style={{ marginBottom: '1rem' }}>
        <h3 style={{ marginTop: 0 }}>Upload JUnit results</h3>
        <p className="muted" style={{ marginTop: 0 }}>
          Select a run, then choose a JUnit XML file to ingest.
        </p>
        <div className="form-row">
          <select
            className="filter-input"
            value={uploadRunId}
            onChange={(e) => setUploadRunId(e.target.value)}
            aria-label="Upload target run"
            {...testIdProps(TestIds.runsUploadRunSelect)}
          >
            <option value="">Select run…</option>
            {runs.map((r) => (
              <option key={r.id} value={r.id}>
                {r.name}
              </option>
            ))}
          </select>
          <input
            type="file"
            accept=".xml,text/xml,application/xml"
            aria-label="JUnit XML file"
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
        <h3 style={{ marginTop: 0 }}>Plans</h3>
        {canPlans && (
        <div className="form-row">
          <input
            className="search-input"
            placeholder="New plan name"
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
            Add plan
          </button>
        </div>
        )}
        {plans.length === 0 ? (
          <p className="muted">No plans yet.</p>
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
        <h3 style={{ marginTop: 0 }}>Recent runs</h3>
        {error && (
          <p className="error" {...testIdProps(TestIds.runsError)}>
            {error}
          </p>
        )}
        {loading && (
          <p className="muted" {...testIdProps(TestIds.runsLoading)}>
            Loading…
          </p>
        )}
        {!loading && runs.length === 0 && (
          <div className="empty" {...testIdProps(TestIds.runsEmpty)}>
            No runs yet.
          </div>
        )}
        {runs.length > 0 && (
          <>
            <table className="table" {...testIdProps(TestIds.runsList)}>
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Status</th>
                  <th>Started</th>
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
