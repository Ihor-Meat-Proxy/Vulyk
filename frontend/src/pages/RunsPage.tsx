import { useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { api, type Suite, type TestPlan, type TestRun } from '../api'

type Props = {
  projectId: string | null
  suites: Suite[]
}

export function RunsPage({ projectId, suites }: Props) {
  const navigate = useNavigate()
  const [runs, setRuns] = useState<TestRun[]>([])
  const [plans, setPlans] = useState<TestPlan[]>([])
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [name, setName] = useState('')
  const [planId, setPlanId] = useState('')
  const [suiteId, setSuiteId] = useState('')
  const [creating, setCreating] = useState(false)
  const [planName, setPlanName] = useState('')
  const [uploadRunId, setUploadRunId] = useState('')
  const [uploading, setUploading] = useState(false)
  const [uploadMessage, setUploadMessage] = useState<string | null>(null)

  useEffect(() => {
    if (!projectId) {
      setRuns([])
      setPlans([])
      return
    }
    let cancelled = false
    setLoading(true)
    Promise.all([api.listRuns(projectId), api.listPlans(projectId)])
      .then(([runPage, planList]) => {
        if (cancelled) return
        setRuns(runPage.items)
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
      const runPage = await api.listRuns(projectId)
      setRuns(runPage.items)
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
      const runPage = await api.listRuns(projectId)
      setRuns(runPage.items)
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
    return <div className="empty">Select a project to manage runs.</div>
  }

  return (
    <div className="runs-page">
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
          />
          <select
            className="filter-input"
            value={planId}
            onChange={(e) => setPlanId(e.target.value)}
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
          >
            {creating ? 'Creating…' : 'Create run'}
          </button>
        </div>
      </section>

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
          />
        </div>
        {uploadMessage && <p className="muted">{uploadMessage}</p>}
      </section>

      <section className="panel" style={{ marginBottom: '1rem' }}>
        <h3 style={{ marginTop: 0 }}>Plans</h3>
        <div className="form-row">
          <input
            className="search-input"
            placeholder="New plan name"
            value={planName}
            onChange={(e) => setPlanName(e.target.value)}
          />
          <button type="button" className="btn secondary" onClick={() => void createPlan()}>
            Add plan
          </button>
        </div>
        {plans.length === 0 ? (
          <p className="muted">No plans yet.</p>
        ) : (
          <ul className="history-list">
            {plans.map((p) => (
              <li key={p.id}>
                <strong>{p.name}</strong>{' '}
                <span className="muted">· {new Date(p.updated_at).toLocaleString()}</span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="panel">
        <h3 style={{ marginTop: 0 }}>Recent runs</h3>
        {error && <p className="error">{error}</p>}
        {loading && <p className="muted">Loading…</p>}
        {!loading && runs.length === 0 && <div className="empty">No runs yet.</div>}
        {runs.length > 0 && (
          <table className="table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Status</th>
                <th>Started</th>
              </tr>
            </thead>
            <tbody>
              {runs.map((r) => (
                <tr key={r.id}>
                  <td>
                    <Link to={`/runs/${r.id}`}>{r.name}</Link>
                  </td>
                  <td>
                    <span className={`status-pill status-${r.status}`}>{r.status}</span>
                  </td>
                  <td className="muted">
                    {r.started_at ? new Date(r.started_at).toLocaleString() : '—'}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  )
}
