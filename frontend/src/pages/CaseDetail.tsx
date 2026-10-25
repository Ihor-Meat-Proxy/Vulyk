import { useEffect, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { api, type TestCaseDetail } from '../api'

type Props = {
  projectId: string | null
}

export function CaseDetail({ projectId }: Props) {
  const { caseId } = useParams()
  const [detail, setDetail] = useState<TestCaseDetail | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    if (!projectId || !caseId) {
      setLoading(false)
      return
    }
    let cancelled = false
    setLoading(true)
    api
      .getCase(projectId, caseId)
      .then((data) => {
        if (!cancelled) {
          setDetail(data)
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
    </div>
  )
}
