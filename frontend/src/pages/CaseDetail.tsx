import { useEffect, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { api, type Requirement, type TestCaseDetail } from '../api'

type Props = {
  projectId: string | null
}

export function CaseDetail({ projectId }: Props) {
  const { caseId } = useParams()
  const [detail, setDetail] = useState<TestCaseDetail | null>(null)
  const [linked, setLinked] = useState<Requirement[]>([])
  const [allReqs, setAllReqs] = useState<Requirement[]>([])
  const [linkId, setLinkId] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    if (!projectId || !caseId) {
      setLoading(false)
      return
    }
    let cancelled = false
    setLoading(true)
    Promise.all([
      api.getCase(projectId, caseId),
      api.listCaseRequirements(projectId, caseId),
      api.listRequirements(projectId),
    ])
      .then(([data, reqs, all]) => {
        if (!cancelled) {
          setDetail(data)
          setLinked(reqs)
          setAllReqs(all)
          setError(null)
        }
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
  }, [projectId, caseId])

  if (!projectId) {
    return <p className="muted">Select a project first.</p>
  }

  if (loading) {
    return <p className="muted">Loading case…</p>
  }

  if (error) {
    return <p className="error">{error}</p>
  }

  if (!detail) {
    return <p className="muted">Case not found.</p>
  }

  const { case: c, steps } = detail

  return (
    <div className="panel">
      <p style={{ marginTop: 0 }}>
        <Link to="/">← Back to cases</Link>
      </p>
      <h2>
        <span className="badge">{c.key}</span> {c.title}
      </h2>
      <div className="meta-row">
        <span>Priority: {c.priority}</span>
        <span>Status: {c.status}</span>
        <span>Version: {c.version}</span>
      </div>
      {c.precondition && (
        <section style={{ marginBottom: '1rem' }}>
          <h3 style={{ marginBottom: '0.35rem' }}>Preconditions</h3>
          <p className="muted" style={{ margin: 0, whiteSpace: 'pre-wrap' }}>
            {c.precondition}
          </p>
        </section>
      )}
      {c.description && (
        <section style={{ marginBottom: '1rem' }}>
          <h3 style={{ marginBottom: '0.35rem' }}>Description</h3>
          <p style={{ margin: 0, whiteSpace: 'pre-wrap' }}>{c.description}</p>
        </section>
      )}
      <section>
        <h3>Steps</h3>
        {steps.length === 0 ? (
          <p className="muted">No steps defined.</p>
        ) : (
          <div className="steps">
            {steps.map((step) => (
              <div className="step" key={step.id}>
                <div className="step-num">{step.position + 1}</div>
                <div>
                  <div className="muted" style={{ fontSize: '0.78rem' }}>
                    Action
                  </div>
                  <div style={{ whiteSpace: 'pre-wrap' }}>{step.action}</div>
                </div>
                <div>
                  <div className="muted" style={{ fontSize: '0.78rem' }}>
                    Expected
                  </div>
                  <div style={{ whiteSpace: 'pre-wrap' }}>{step.expected_result}</div>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      <section style={{ marginTop: '1.5rem' }}>
        <h3>Linked requirements</h3>
        {linked.length === 0 ? (
          <p className="muted">No requirements linked.</p>
        ) : (
          <ul className="req-list">
            {linked.map((r) => (
              <li key={r.id}>
                <span className="badge">{r.external_key ?? r.source}</span>{' '}
                {r.title}
              </li>
            ))}
          </ul>
        )}
        <div className="req-form" style={{ marginTop: '0.75rem' }}>
          <select
            value={linkId}
            onChange={(e) => setLinkId(e.target.value)}
            aria-label="Requirement to link"
          >
            <option value="">Link a requirement…</option>
            {allReqs
              .filter((r) => !linked.some((l) => l.id === r.id))
              .map((r) => (
                <option key={r.id} value={r.id}>
                  {r.title}
                </option>
              ))}
          </select>
          <button
            type="button"
            className="btn secondary"
            disabled={!linkId || !caseId}
            onClick={() => {
              if (!projectId || !caseId || !linkId) return
              void api
                .linkRequirement(projectId, {
                  case_id: caseId,
                  requirement_id: linkId,
                })
                .then(() => api.listCaseRequirements(projectId, caseId))
                .then(setLinked)
                .then(() => setLinkId(''))
                .catch((err: Error) => setError(err.message))
            }}
          >
            Link
          </button>
        </div>
      </section>
    </div>
  )
}
