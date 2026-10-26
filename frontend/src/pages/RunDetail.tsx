import { useEffect, useRef, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import {
  api,
  RESULT_STATUSES,
  statusClass,
  type RunItemExecution,
  type TestRunDetail,
} from '../api'
import { StatusActionButton, StatusBadge } from '../components/StatusBadge'
import { cacheRun, enqueueStepResult, getCachedRun } from '../pwa/offlineDb'
import { bumpPendingCount } from '../pwa/offlineSync'
import { useI18n } from '../i18n'
import { TestIds, listChildProps, listItemProps, testIdProps } from '../testing'

type Props = {
  projectId: string | null
}

const STEP_STATUSES = RESULT_STATUSES.filter((s) => s !== 'Retest')

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const result = String(reader.result ?? '')
      const comma = result.indexOf(',')
      resolve(comma >= 0 ? result.slice(comma + 1) : result)
    }
    reader.onerror = () => reject(reader.error ?? new Error('read failed'))
    reader.readAsDataURL(file)
  })
}

export function RunDetail({ projectId }: Props) {
  const { runId } = useParams()
  const { t } = useI18n()
  const [detail, setDetail] = useState<TestRunDetail | null>(null)
  const [active, setActive] = useState<RunItemExecution | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [fromCache, setFromCache] = useState(false)
  const [loading, setLoading] = useState(true)
  const [assignee, setAssignee] = useState('')
  const [comment, setComment] = useState('')
  const [stepStatuses, setStepStatuses] = useState<string[]>([])
  const [saving, setSaving] = useState(false)
  const [mobileStep, setMobileStep] = useState(0)
  const cameraInputRef = useRef<HTMLInputElement>(null)

  const reload = async () => {
    if (!projectId || !runId) return
    try {
      const data = await api.getRun(projectId, runId)
      setDetail(data)
      setFromCache(false)
      void cacheRun(projectId, runId, data)
    } catch (err) {
      const cached = (await getCachedRun(projectId, runId)) as TestRunDetail | null
      if (cached) {
        setDetail(cached)
        setFromCache(true)
        setNotice('Showing cached run (offline).')
        return
      }
      throw err
    }
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
    try {
      const exec = await api.getRunItem(projectId, runId, itemId)
      setActive(exec)
      setAssignee(exec.item.item.assignee ?? '')
      setComment(exec.item.item.comment ?? '')
      setStepStatuses(exec.steps.map(() => 'Untested'))
      setMobileStep(0)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  const buildBody = (status: string) => {
    if (!active) return null
    return {
      run_item_id: active.item.item.id,
      status,
      message: comment,
      assignee: assignee || null,
      steps: active.steps.map((step, idx) => ({
        step_id: step.id,
        position: step.position,
        action: step.action,
        expected_result: step.expected_result,
        status: stepStatuses[idx]?.toLowerCase() === 'untested' ? status : stepStatuses[idx],
      })),
    }
  }

  const advanceAfterSave = async (currentItemId: string) => {
    if (!projectId || !runId) return
    const data = await api.getRun(projectId, runId)
    setDetail(data)
    setFromCache(false)
    void cacheRun(projectId, runId, data)
    const next = data.items.find(
      (i) =>
        i.item.status.toLowerCase() === 'untested' && i.item.id !== currentItemId,
    )
    if (next) {
      await openItem(next.item.id)
    } else {
      setActive(null)
    }
  }

  const submitStatus = async (status: string) => {
    if (!projectId || !runId || !active) return
    const body = buildBody(status)
    if (!body) return
    setSaving(true)
    setNotice(null)
    const currentId = active.item.item.id
    try {
      const offline = typeof navigator !== 'undefined' && !navigator.onLine
      if (offline) {
        await enqueueStepResult({ projectId, runId, body })
        await bumpPendingCount()
        setNotice('Saved offline — will sync when you are back online.')
        // Optimistically mark local active item
        setDetail((prev) => {
          if (!prev) return prev
          return {
            ...prev,
            items: prev.items.map((row) =>
              row.item.id === currentId
                ? { ...row, item: { ...row.item, status } }
                : row,
            ),
          }
        })
        setActive(null)
        setError(null)
        return
      }
      await api.createResult(projectId, runId, body)
      await advanceAfterSave(currentId)
      setError(null)
    } catch (err) {
      // Network failure mid-flight → queue for later
      try {
        await enqueueStepResult({ projectId, runId, body })
        await bumpPendingCount()
        setNotice('Network error — result queued for sync.')
        setActive(null)
        setError(null)
      } catch {
        setError(err instanceof Error ? err.message : String(err))
      }
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
    setNotice('Note attached.')
  }

  const attachPhoto = async (file: File) => {
    if (!projectId || !active?.latest_result) {
      setError('Record a result first, then attach a photo.')
      return
    }
    setSaving(true)
    try {
      const content_base64 = await fileToBase64(file)
      await api.createAttachment(projectId, {
        entity_type: 'result',
        entity_id: active.latest_result.result.id,
        filename: file.name || 'capture.jpg',
        content_type: file.type || 'image/jpeg',
        content_base64,
      })
      setNotice('Photo attached.')
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSaving(false)
    }
  }

  if (!projectId) return <p className="muted">Select a project first.</p>
  if (loading) {
    return (
      <p className="muted" {...testIdProps(TestIds.runsLoading)}>
        Loading run…
      </p>
    )
  }
  if (error && !detail) {
    return (
      <p className="error" {...testIdProps(TestIds.runDetailError)}>
        {error}
      </p>
    )
  }
  if (!detail) return <p className="muted">Run not found.</p>

  const { run, progress, items } = detail
  const currentStep = active?.steps[mobileStep]

  const STATUS_ACTION_IDS = {
    Passed: TestIds.executionPassButton,
    Failed: TestIds.executionFailButton,
    Blocked: TestIds.executionBlockButton,
    Skipped: TestIds.executionSkipButton,
  } as const

  return (
    <div
      className={`run-layout${active ? ' run-executing' : ''}`}
      {...testIdProps(TestIds.runDetailPanel)}
    >
      <div className="panel run-queue-panel">
        <p style={{ marginTop: 0 }}>
          <Link to="/runs" {...testIdProps(TestIds.runDetailBackLink)}>
            ← Runs
          </Link>
        </p>
        <h2 style={{ marginTop: 0 }}>{run.name}</h2>
        <div className="meta-row">
          <span className={`status-pill ${statusClass(run.status)}`}>{run.status}</span>
          <span>
            Progress: {progress.completed}/{progress.total} ({progress.percent.toFixed(0)}%)
          </span>
          {fromCache && <span className="badge">cached</span>}
        </div>
        <div className="progress-track" aria-label="Run progress">
          <div className="progress-seg passed" style={{ flex: progress.passed || 0.0001 }} />
          <div className="progress-seg failed" style={{ flex: progress.failed || 0.0001 }} />
          <div className="progress-seg blocked" style={{ flex: progress.blocked || 0.0001 }} />
          <div className="progress-seg skipped" style={{ flex: progress.skipped || 0.0001 }} />
          <div className="progress-seg untested" style={{ flex: progress.untested || 0.0001 }} />
        </div>
        <div className="progress-legend" aria-label="Progress legend">
          {(
            [
              ['passed', progress.passed],
              ['failed', progress.failed],
              ['blocked', progress.blocked],
              ['skipped', progress.skipped],
              ['untested', progress.untested],
            ] as const
          ).map(([key, count]) => (
            <span key={key} className="progress-legend-item">
              <span className={`progress-legend-swatch ${key}`} aria-hidden />
              {t(`status.${key}`)} {count}
            </span>
          ))}
        </div>

        {error && (
          <p className="error" {...testIdProps(TestIds.runDetailError)}>
            {error}
          </p>
        )}
        {notice && <p className="notice">{notice}</p>}

        <ul
          className="run-item-list"
          aria-label="Run cases"
          {...testIdProps(TestIds.runDetailCases)}
        >
          {items.map((row, index) => (
            <li
              key={row.item.id}
              {...listItemProps(TestIds.runDetailCaseRow, {
                entityId: row.item.id,
                entityKey: row.case_key,
                index,
              })}
            >
              <button
                type="button"
                className={`run-item-btn${active?.item.item.id === row.item.id ? ' active' : ''}`}
                onClick={() => void openItem(row.item.id)}
                {...listChildProps(TestIds.runDetailCaseRow, 'open')}
              >
                <span className="badge">{row.case_key}</span>
                <span className="run-item-title">{row.case_title}</span>
                <StatusBadge status={row.item.status} />
              </button>
            </li>
          ))}
        </ul>

        <div className="run-table-desktop">
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
              {items.map((row, index) => (
                <tr
                  key={row.item.id}
                  className={active?.item.item.id === row.item.id ? 'row-active' : undefined}
                  onClick={() => void openItem(row.item.id)}
                  {...listItemProps(TestIds.runDetailCaseRow, {
                    entityId: row.item.id,
                    entityKey: row.case_key,
                    index,
                  })}
                >
                  <td>
                    <span className="badge">{row.case_key}</span> {row.case_title}
                  </td>
                  <td>{row.case_priority}</td>
                  <td>{row.item.assignee || '—'}</td>
                  <td>
                    <StatusBadge status={row.item.status} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      <div className="panel execution-panel">
        {!active ? (
          <div className="empty">Select a case to execute step by step.</div>
        ) : (
          <>
            <div className="exec-header">
              <button
                type="button"
                className="btn secondary exec-back"
                onClick={() => setActive(null)}
                {...testIdProps(TestIds.executionBackToQueueButton)}
              >
                ← Queue
              </button>
              <h3 style={{ margin: 0 }}>
                <span className="badge">{active.item.case_key}</span>{' '}
                {active.item.case_title}
              </h3>
            </div>
            <div className="form-row">
              <input
                className="filter-input"
                placeholder="Assignee"
                value={assignee}
                onChange={(e) => setAssignee(e.target.value)}
                aria-label="Assignee"
                {...testIdProps(TestIds.executionAssigneeInput)}
              />
              <input
                className="search-input"
                placeholder="Comment"
                value={comment}
                onChange={(e) => setComment(e.target.value)}
                aria-label="Comment"
                {...testIdProps(TestIds.executionCommentInput)}
              />
            </div>

            {/* Desktop: all steps */}
            <div className="steps steps-desktop">
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
                      value={stepStatuses[idx] ?? 'Untested'}
                      aria-label={`Step ${idx + 1} status`}
                      onChange={(e) => {
                        const next = [...stepStatuses]
                        next[idx] = e.target.value
                        setStepStatuses(next)
                      }}
                      {...testIdProps(TestIds.executionStepStatusSelect)}
                    >
                      {STEP_STATUSES.map((s) => (
                        <option key={s} value={s}>
                          {s}
                        </option>
                      ))}
                    </select>
                  </div>
                </div>
              ))}
            </div>

            {/* Mobile: one step at a time */}
            {currentStep && (
              <div className="mobile-step-card">
                <div className="mobile-step-meta muted">
                  Step {mobileStep + 1} of {active.steps.length}
                </div>
                <div className="mobile-step-block">
                  <div className="muted" style={{ fontSize: '0.78rem' }}>
                    Action
                  </div>
                  <div style={{ whiteSpace: 'pre-wrap' }}>{currentStep.action}</div>
                </div>
                <div className="mobile-step-block">
                  <div className="muted" style={{ fontSize: '0.78rem' }}>
                    Expected
                  </div>
                  <div style={{ whiteSpace: 'pre-wrap' }}>
                    {currentStep.expected_result}
                  </div>
                </div>
                <select
                  className="filter-input"
                  value={stepStatuses[mobileStep] ?? 'Untested'}
                  aria-label={`Step ${mobileStep + 1} status`}
                  onChange={(e) => {
                    const next = [...stepStatuses]
                    next[mobileStep] = e.target.value
                    setStepStatuses(next)
                  }}
                  {...testIdProps(TestIds.executionStepStatusSelect)}
                >
                  {STEP_STATUSES.map((s) => (
                    <option key={s} value={s}>
                      {s}
                    </option>
                  ))}
                </select>
                <div className="mobile-step-nav">
                  <button
                    type="button"
                    className="btn secondary"
                    disabled={mobileStep === 0}
                    onClick={() => setMobileStep((s) => Math.max(0, s - 1))}
                    {...testIdProps(TestIds.executionPrevButton)}
                  >
                    Prev
                  </button>
                  <button
                    type="button"
                    className="btn secondary"
                    disabled={mobileStep >= active.steps.length - 1}
                    onClick={() =>
                      setMobileStep((s) =>
                        Math.min(active.steps.length - 1, s + 1),
                      )
                    }
                    {...testIdProps(TestIds.executionNextButton)}
                  >
                    Next
                  </button>
                </div>
              </div>
            )}

            <div className="exec-actions exec-actions-thumb" role="group" aria-label="Result status">
              {(['Passed', 'Failed', 'Blocked', 'Skipped'] as const).map((s) => (
                <StatusActionButton
                  key={s}
                  status={s}
                  className="thumb-btn"
                  disabled={saving}
                  onClick={() => void submitStatus(s)}
                  {...testIdProps(STATUS_ACTION_IDS[s])}
                />
              ))}
            </div>

            {active.history && active.history.length > 0 && (
              <section className="result-timeline" aria-label="Result history">
                <h4 style={{ marginBottom: '0.35rem' }}>Result history</h4>
                <ol className="timeline-list">
                  {active.history.map((h) => (
                    <li key={h.result.id}>
                      <StatusBadge status={h.result.status} />{' '}
                      <span className="muted">
                        {new Date(h.result.executed_at).toLocaleString()}
                        {h.executed_by_name ? ` · ${h.executed_by_name}` : ''}
                        {h.result.case_version != null
                          ? ` · v${h.result.case_version}`
                          : ''}
                        {h.result.message ? ` — ${h.result.message}` : ''}
                      </span>
                    </li>
                  ))}
                </ol>
              </section>
            )}

            <div className="attach-row">
              <input
                ref={cameraInputRef}
                type="file"
                accept="image/*"
                capture="environment"
                className="sr-only"
                aria-label="Attach photo from camera"
                onChange={(e) => {
                  const file = e.target.files?.[0]
                  if (file) void attachPhoto(file)
                  e.target.value = ''
                }}
                {...testIdProps(TestIds.executionCameraInput)}
              />
              <button
                type="button"
                className="btn secondary"
                disabled={saving || !active.latest_result}
                onClick={() => cameraInputRef.current?.click()}
                {...testIdProps(TestIds.executionCameraButton)}
              >
                Camera / photo
              </button>
              {active.latest_result && (
                <button
                  type="button"
                  className="btn secondary"
                  onClick={() => void attachNote()}
                  {...testIdProps(TestIds.executionAttachNoteButton)}
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
