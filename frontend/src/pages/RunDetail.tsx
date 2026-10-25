import { useEffect, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import {
  api,
  type RunItemExecution,
  type TestRunDetail,
} from '../api'

type Props = {
  projectId: string | null
}

const STATUSES = ['untested', 'passed', 'failed', 'blocked', 'skipped', 'retest'] as const

export function RunDetail({ projectId }: Props) {
  const { runId } = useParams()
  const [detail, setDetail] = useState<TestRunDetail | null>(null)
  const [active, setActive] = useState<RunItemExecution | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [assignee, setAssignee] = useState('')
  const [comment, setComment] = useState('')
  const [stepStatuses, setStepStatuses] = useState<string[]>([])
  const [saving, setSaving] = useState(false)

  const reload = async () => {
    if (!projectId || !runId) return
    const data = await api.getRun(projectId, runId)
    setDetail(data)
  }

  useEffect(() => {
    if (!projectId || !runId) {
      setLoading(false)
      return
    }
    let cancelled = false
    setLoading(true)
    reload()
      .then(() => {
        if (!cancelled) setError(null)
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
  }, [projectId, runId])

  const openItem = async (itemId: string) => {
    if (!projectId || !runId) return
    const exec = await api.getRunItem(projectId, runId, itemId)
    setActive(exec)
    setAssignee(exec.item.item.assignee ?? '')
    setComment(exec.item.item.comment ?? '')
    setStepStatuses(exec.steps.map(() => 'untested'))
  }

  const submitStatus = async (status: string) => {
    if (!projectId || !runId || !active) return
    setSaving(true)
    try {
      await api.createResult(projectId, runId, {
        run_item_id: active.item.item.id,
        status,
        message: comment,
        assignee: assignee || null,
        steps: active.steps.map((step, idx) => ({
          step_id: step.id,
          position: step.position,
          action: step.action,
          expected_result: step.expected_result,
          status: stepStatuses[idx] === 'untested' ? status : stepStatuses[idx],
        })),
      })
      if (status === 'failed' || status === 'blocked') {
        // optional: leave room for attachment UX; keep lightweight
      }
      await reload()
      const next = detail?.items.find(
        (i) => i.item.status === 'untested' && i.item.id !== active.item.item.id,
      )
      if (next) {
        await openItem(next.item.id)
      } else {
        setActive(null)
      }
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSaving(false)
    }
  }

  const attachNote = async () => {
    if (!projectId || !active?.latest_result) return
    const text = window.btoa(unescape(encodeURIComponent(comment || 'note')))
    await api.createAttachment(projectId, {
      entity_type: 'result',
      entity_id: active.latest_result.result.id,
      filename: 'execution-note.txt',
      content_type: 'text/plain',
      content_base64: text,
    })
  }

  if (!projectId) return <p className="muted">Select a project first.</p>
  if (loading) return <p className="muted">Loading run…</p>
  if (error && !detail) return <p className="error">{error}</p>
  if (!detail) return <p className="muted">Run not found.</p>

  const { run, progress, items } = detail

  return (
    <div className="run-layout">
      <div className="panel">
        <p style={{ marginTop: 0 }}>
          <Link to="/runs">← Runs</Link>
        </p>
        <h2 style={{ marginTop: 0 }}>{run.name}</h2>
        <div className="meta-row">
          <span className={`status-pill status-${run.status}`}>{run.status}</span>
          <span>
            Progress: {progress.completed}/{progress.total} ({progress.percent.toFixed(0)}%)
          </span>
        </div>
        <div className="progress-track" aria-label="Run progress">
          <div className="progress-seg passed" style={{ flex: progress.passed || 0.0001 }} />
          <div className="progress-seg failed" style={{ flex: progress.failed || 0.0001 }} />
          <div className="progress-seg blocked" style={{ flex: progress.blocked || 0.0001 }} />
          <div className="progress-seg skipped" style={{ flex: progress.skipped || 0.0001 }} />
          <div className="progress-seg untested" style={{ flex: progress.untested || 0.0001 }} />
        </div>
        <div className="progress-legend muted">
          <span>passed {progress.passed}</span>
          <span>failed {progress.failed}</span>
          <span>blocked {progress.blocked}</span>
          <span>skipped {progress.skipped}</span>
          <span>untested {progress.untested}</span>
        </div>

        {error && <p className="error">{error}</p>}

        <table className="table">
          <thead>
            <tr>
              <th>Case</th>
              <th>Priority</th>
              <th>Assignee</th>
              <th>Status</th>
            </tr>
          </thead>
          <tbody>
            {items.map((row) => (
              <tr
                key={row.item.id}
                className={active?.item.item.id === row.item.id ? 'row-active' : undefined}
                onClick={() => void openItem(row.item.id)}
              >
                <td>
                  <span className="badge">{row.case_key}</span> {row.case_title}
                </td>
                <td>{row.case_priority}</td>
                <td>{row.item.assignee || '—'}</td>
                <td>
                  <span className={`status-pill status-${row.item.status}`}>
                    {row.item.status}
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="panel execution-panel">
        {!active ? (
          <div className="empty">Select a case to execute step by step.</div>
        ) : (
          <>
            <h3 style={{ marginTop: 0 }}>
              <span className="badge">{active.item.case_key}</span> {active.item.case_title}
            </h3>
            <div className="form-row">
              <input
                className="filter-input"
                placeholder="Assignee"
                value={assignee}
                onChange={(e) => setAssignee(e.target.value)}
              />
              <input
                className="search-input"
                placeholder="Comment"
                value={comment}
                onChange={(e) => setComment(e.target.value)}
              />
            </div>
            <div className="steps">
              {active.steps.map((step, idx) => (
                <div className="step" key={step.id}>
                  <div className="step-num">{idx + 1}</div>
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
                    <select
                      className="filter-input"
                      style={{ marginTop: '0.35rem' }}
                      value={stepStatuses[idx] ?? 'untested'}
                      onChange={(e) => {
                        const next = [...stepStatuses]
                        next[idx] = e.target.value
                        setStepStatuses(next)
                      }}
                    >
                      {STATUSES.filter((s) => s !== 'retest').map((s) => (
                        <option key={s} value={s}>
                          {s}
                        </option>
                      ))}
                    </select>
                  </div>
                </div>
              ))}
            </div>
            <div className="exec-actions">
              {(['passed', 'failed', 'blocked', 'skipped'] as const).map((s) => (
                <button
                  key={s}
                  type="button"
                  className={`btn status-btn status-${s}`}
                  disabled={saving}
                  onClick={() => void submitStatus(s)}
                >
                  {s}
                </button>
              ))}
              {active.latest_result && (
                <button
                  type="button"
                  className="btn secondary"
                  onClick={() => void attachNote()}
                >
                  Attach note
                </button>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  )
}
