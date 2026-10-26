import { useCallback, useEffect, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import {
  ApiError,
  api,
  type CaseExecutionHistoryPage,
  type CaseResultsPage,
  type CaseVersionSummary,
  type CustomField,
  type Requirement,
  type Suite,
  type TestCaseDetail,
} from '../api'
import { useAuth } from '../auth'
import { CaseBreadcrumbs } from '../components/CaseBreadcrumbs'
import { CaseKeyBadge } from '../components/CaseKeyBadge'
import { StatusBadge } from '../components/StatusBadge'
import { useBackspaceNavigate } from '../hooks/useBackspaceNavigate'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { useI18n } from '../i18n'
import { casePath, isCaseUuid } from '../lib/caseUrls'
import { cacheCase, getCachedCase } from '../pwa/offlineDb'
import { formatTimestamp } from '../timezone'

type Props = {
  projectId: string | null
  projectName: string
  suites: Suite[]
  onEnsureProject: (projectId: string) => void
  onBackToList: (suiteId: string | null) => void
}

type DraftStep = {
  action: string
  expected_result: string
  data: string
}

type Tab = 'details' | 'history' | 'executions'

function Sparkline({ statuses }: { statuses: string[] }) {
  if (statuses.length === 0) return null
  const w = Math.max(60, statuses.length * 8)
  const h = 22
  return (
    <svg
      className="exec-sparkline"
      width={w}
      height={h}
      viewBox={`0 0 ${w} ${h}`}
      aria-hidden
    >
      {statuses.map((status, i) => {
        const x = i * (w / statuses.length) + 1
        const barW = Math.max(3, w / statuses.length - 2)
        const passed = status.toLowerCase() === 'passed'
        const failed = status.toLowerCase() === 'failed'
        const fill = passed
          ? 'var(--status-passed)'
          : failed
            ? 'var(--status-failed)'
            : 'var(--status-skipped)'
        const barH = passed ? h - 4 : failed ? h - 2 : h * 0.55
        return (
          <rect
            key={`${status}-${i}`}
            x={x}
            y={h - barH}
            width={barW}
            height={barH}
            fill={fill}
            rx={1}
          />
        )
      })}
    </svg>
  )
}

export function CaseDetail({
  projectId,
  projectName,
  suites,
  onEnsureProject,
  onBackToList,
}: Props) {
  const { user, status: authStatus } = useAuth()
  const { t } = useI18n()
  const navigate = useNavigate()
  const { can } = useProjectPermissions(projectId)
  const canEdit = can('create_edit_cases')
  const canManageReqs = can('manage_requirements')
  const { caseId: caseRef } = useParams()
  const [detail, setDetail] = useState<TestCaseDetail | null>(null)
  const [versions, setVersions] = useState<CaseVersionSummary[]>([])
  const [fields, setFields] = useState<CustomField[]>([])
  const [linked, setLinked] = useState<Requirement[]>([])
  const [allReqs, setAllReqs] = useState<Requirement[]>([])
  const [linkId, setLinkId] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [deleted, setDeleted] = useState(false)
  const [offlineNotice, setOfflineNotice] = useState(false)
  const [loading, setLoading] = useState(true)
  const caseId = detail?.case.id ?? (caseRef && isCaseUuid(caseRef) ? caseRef : null)
  const [editing, setEditing] = useState(false)
  const [saving, setSaving] = useState(false)
  const [title, setTitle] = useState('')
  const [description, setDescription] = useState('')
  const [precondition, setPrecondition] = useState('')
  const [priority, setPriority] = useState('medium')
  const [status, setStatus] = useState('draft')
  const [tagInput, setTagInput] = useState('')
  const [steps, setSteps] = useState<DraftStep[]>([])
  const [fieldValues, setFieldValues] = useState<Record<string, string>>({})
  const [tab, setTab] = useState<Tab>('details')
  const [resultsPage, setResultsPage] = useState<CaseResultsPage | null>(null)
  const [resultsLoading, setResultsLoading] = useState(false)
  const [filterStatus, setFilterStatus] = useState('')
  const [filterRunId, setFilterRunId] = useState('')
  const [resultsOffset, setResultsOffset] = useState(0)
  const resultsLimit = 25
  const [execPage, setExecPage] = useState<CaseExecutionHistoryPage | null>(null)
  const [execLoading, setExecLoading] = useState(false)
  const [execOffset, setExecOffset] = useState(0)
  const execLimit = 25

  const applyDetail = (data: TestCaseDetail) => {
    setDetail(data)
    setTitle(data.case.title)
    setDescription(data.case.description)
    setPrecondition(data.case.precondition)
    setPriority(data.case.priority)
    setStatus(data.case.status)
    setTagInput(data.tags.map((t) => t.name).join(', '))
    setSteps(
      data.steps.map((s) => ({
        action: s.action,
        expected_result: s.expected_result,
        data: s.data,
      })),
    )
    const vals: Record<string, string> = {}
    for (const cf of data.custom_fields) {
      vals[cf.field_id] =
        cf.value === null || cf.value === undefined ? '' : String(cf.value)
    }
    setFieldValues(vals)
  }

  const goBack = useCallback(() => {
    onBackToList(detail?.case.suite_id ?? null)
  }, [detail?.case.suite_id, onBackToList])

  useBackspaceNavigate(goBack, !loading && !deleted)

  const loadRelated = async (pid: string, id: string, data: TestCaseDetail) => {
    const [vers, defs, reqs, all] = await Promise.all([
      api.listVersions(pid, id),
      api.listCustomFields(pid),
      api.listCaseRequirements(pid, id),
      api.listRequirements(pid),
    ])
    applyDetail(data)
    setVersions(vers)
    setFields(defs)
    setLinked(reqs)
    setAllReqs(all)
    setOfflineNotice(false)
    setDeleted(false)
    void cacheCase(pid, id, data)
  }

  const reload = async () => {
    if (!caseRef) return
    try {
      let data: TestCaseDetail
      if (isCaseUuid(caseRef)) {
        if (!projectId) return
        data = await api.getCase(projectId, caseRef)
        if (data.case.key) {
          navigate(casePath(data.case.key), { replace: true })
        }
      } else {
        data = await api.getCaseByKey(caseRef)
        onEnsureProject(data.case.project_id)
        if (data.case.key && data.case.key.toUpperCase() !== caseRef.toUpperCase()) {
          navigate(casePath(data.case.key), { replace: true })
        }
      }
      const pid = data.case.project_id
      const id = data.case.id
      try {
        await loadRelated(pid, id, data)
      } catch {
        applyDetail(data)
      }
    } catch (err) {
      if (err instanceof ApiError && (err.status === 410 || err.code === 'case_deleted')) {
        setDeleted(true)
        setDetail(null)
        return
      }
      if (projectId && caseRef && isCaseUuid(caseRef)) {
        const cached = (await getCachedCase(
          projectId,
          caseRef,
        )) as TestCaseDetail | null
        if (cached) {
          applyDetail(cached)
          setOfflineNotice(true)
          return
        }
      }
      throw err
    }
  }

  const loadResults = async (offset = 0) => {
    const pid = detail?.case.project_id ?? projectId
    const id = detail?.case.id
    if (!pid || !id) return
    setResultsLoading(true)
    try {
      const page = await api.listCaseResults(pid, id, {
        status: filterStatus || undefined,
        run_id: filterRunId.trim() || undefined,
        limit: resultsLimit,
        offset,
      })
      setResultsPage(page)
      setResultsOffset(offset)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setResultsLoading(false)
    }
  }

  useEffect(() => {
    if (!caseRef) {
      setLoading(false)
      return
    }
    // UUID URLs need an active project; key URLs resolve globally.
    if (isCaseUuid(caseRef) && !projectId) {
      setLoading(false)
      return
    }
    let cancelled = false
    setLoading(true)
    setDeleted(false)
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
  }, [projectId, caseRef])

  const loadExecutions = async (offset = 0) => {
    const pid = detail?.case.project_id ?? projectId
    const id = detail?.case.id
    if (!pid || !id) return
    setExecLoading(true)
    try {
      const page = await api.listCaseExecutions(pid, id, {
        limit: execLimit,
        offset,
      })
      setExecPage(page)
      setExecOffset(offset)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setExecLoading(false)
    }
  }

  useEffect(() => {
    if (tab === 'history' && detail?.case.id) {
      void loadResults(0)
    }
    if (tab === 'executions' && detail?.case.id) {
      void loadExecutions(0)
    }
  }, [tab, detail?.case.id])

  const save = async () => {
    const pid = detail?.case.project_id ?? projectId
    const id = detail?.case.id
    if (!pid || !id) return
    setSaving(true)
    try {
      const tagNames = tagInput
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean)
      await api.updateCase(pid, id, {
        title,
        description,
        precondition,
        priority,
        status,
        steps,
        tag_names: tagNames,
        change_summary: 'edited in UI',
      })
      const cfBody = Object.entries(fieldValues).map(([field_id, value]) => ({
        field_id,
        value,
      }))
      if (cfBody.length) {
        await api.setCustomFields(pid, id, cfBody)
      }
      await reload()
      setEditing(false)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSaving(false)
    }
  }

  if (isCaseUuid(caseRef) && !projectId) {
    return <p className="muted">{t('cases.selectProject')}</p>
  }

  if (loading) {
    return <p className="muted">{t('cases.loading')}</p>
  }

  if (deleted) {
    return (
      <div className="panel case-detail case-deleted" data-testid="case-detail-deleted">
        <h2>{t('cases.deletedTitle')}</h2>
        <p className="muted">
          {t('cases.deletedBody', { key: caseRef ?? '' })}
        </p>
        <p>
          <Link to="/" data-testid="case-detail-deleted-back">
            {t('cases.backToCases')}
          </Link>
        </p>
      </div>
    )
  }

  if (error && !detail) {
    return <p className="error">{error}</p>
  }

  if (!detail) {
    return <p className="muted">{t('cases.notFound')}</p>
  }

  const { case: c, tags } = detail
  const summary = resultsPage?.summary
  const activeProjectId = c.project_id

  return (
    <div className="panel case-detail" data-testid="case-detail-page">
      <CaseBreadcrumbs
        projectName={projectName}
        suites={suites}
        suiteId={c.suite_id}
        caseKey={c.key}
        caseTitle={c.title}
        onSelectFolder={(suiteId) => onBackToList(suiteId)}
      />
      <p style={{ marginTop: 0 }}>
        <button
          type="button"
          className="linkish case-detail-back"
          data-testid="case-detail-back"
          onClick={goBack}
          title={t('cases.backspaceHint')}
        >
          {t('cases.backToCases')}
          <kbd className="shortcut-hint">{t('cases.backspaceKey')}</kbd>
        </button>
      </p>
      {offlineNotice && (
        <p className="notice">{t('cases.offlineCached')}</p>
      )}
      {error && <p className="error">{error}</p>}
      <div className="detail-header">
        <h2>
          <CaseKeyBadge caseKey={c.key} />{' '}
          {editing ? (
            <input
              className="title-input"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              data-testid="case-detail-title-input"
            />
          ) : (
            <span data-testid="case-detail-title">{c.title}</span>
          )}
        </h2>
        <div className="detail-actions">
          {canEdit && !editing ? (
            <button type="button" className="btn" onClick={() => setEditing(true)}>
              Edit
            </button>
          ) : null}
          {canEdit && editing ? (
            <>
              <button
                type="button"
                className="btn secondary"
                onClick={() => setEditing(false)}
                disabled={saving}
              >
                Cancel
              </button>
              <button type="button" className="btn" onClick={() => void save()} disabled={saving}>
                {saving ? 'Saving…' : 'Save'}
              </button>
            </>
          ) : null}
        </div>
      </div>

      <div className="case-tabs" role="tablist" aria-label="Case sections">
        <button
          type="button"
          role="tab"
          aria-selected={tab === 'details'}
          className={tab === 'details' ? 'active' : ''}
          onClick={() => setTab('details')}
        >
          Details
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === 'history'}
          className={tab === 'history' ? 'active' : ''}
          onClick={() => setTab('history')}
        >
          History / Results
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === 'executions'}
          className={tab === 'executions' ? 'active' : ''}
          onClick={() => setTab('executions')}
        >
          {t('executions.caseExecutions')}
        </button>
      </div>

      {tab === 'details' && (
        <>
          <div className="meta-row">
            <span>
              Priority:{' '}
              {editing ? (
                <select value={priority} onChange={(e) => setPriority(e.target.value)}>
                  <option value="low">low</option>
                  <option value="medium">medium</option>
                  <option value="high">high</option>
                  <option value="critical">critical</option>
                </select>
              ) : (
                c.priority
              )}
            </span>
            <span>
              Status:{' '}
              {editing ? (
                <select value={status} onChange={(e) => setStatus(e.target.value)}>
                  <option value="draft">draft</option>
                  <option value="ready">ready</option>
                  <option value="deprecated">deprecated</option>
                </select>
              ) : (
                c.status
              )}
            </span>
            <span>Version: {c.version}</span>
          </div>

          <section className="tag-row">
            <strong>Tags</strong>
            {editing ? (
              <input
                className="search-input"
                value={tagInput}
                onChange={(e) => setTagInput(e.target.value)}
                placeholder="comma-separated tags"
              />
            ) : tags.length === 0 ? (
              <span className="muted">none</span>
            ) : (
              tags.map((t) => (
                <span key={t.id} className="tag-chip" style={{ borderColor: t.color }}>
                  {t.name}
                </span>
              ))
            )}
          </section>

          <section style={{ marginBottom: '1rem' }}>
            <h3 style={{ marginBottom: '0.35rem' }}>Preconditions</h3>
            {editing ? (
              <textarea
                className="area"
                value={precondition}
                onChange={(e) => setPrecondition(e.target.value)}
                rows={3}
              />
            ) : (
              <p className="muted" style={{ margin: 0, whiteSpace: 'pre-wrap' }}>
                {c.precondition || '—'}
              </p>
            )}
          </section>

          <section style={{ marginBottom: '1rem' }}>
            <h3 style={{ marginBottom: '0.35rem' }}>Description</h3>
            {editing ? (
              <textarea
                className="area"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                rows={4}
              />
            ) : (
              <p style={{ margin: 0, whiteSpace: 'pre-wrap' }}>{c.description || '—'}</p>
            )}
          </section>

          {(fields.length > 0 || detail.custom_fields.length > 0) && (
            <section style={{ marginBottom: '1rem' }}>
              <h3>Custom fields</h3>
              <div className="cf-grid">
                {editing
                  ? fields.map((cf) => (
                      <label key={cf.id} className="cf-field">
                        <span className="muted">{cf.name}</span>
                        <input
                          value={fieldValues[cf.id] ?? ''}
                          onChange={(e) =>
                            setFieldValues((prev) => ({
                              ...prev,
                              [cf.id]: e.target.value,
                            }))
                          }
                        />
                      </label>
                    ))
                  : detail.custom_fields.map((cf) => (
                      <label key={cf.field_id} className="cf-field">
                        <span className="muted">{cf.name}</span>
                        <span>
                          {cf.value === null || cf.value === undefined
                            ? '—'
                            : String(cf.value)}
                        </span>
                      </label>
                    ))}
              </div>
            </section>
          )}

          <section>
            <div className="detail-header">
              <h3>Steps</h3>
              {editing && (
                <button
                  type="button"
                  className="btn secondary"
                  onClick={() =>
                    setSteps((prev) => [
                      ...prev,
                      { action: '', expected_result: '', data: '' },
                    ])
                  }
                >
                  Add step
                </button>
              )}
            </div>
            {steps.length === 0 ? (
              <p className="muted">No steps defined.</p>
            ) : (
              <div className="steps">
                {steps.map((step, idx) => (
                  <div className="step" key={idx}>
                    <div className="step-num">{idx + 1}</div>
                    <div>
                      <div className="muted" style={{ fontSize: '0.78rem' }}>
                        Action
                      </div>
                      {editing ? (
                        <textarea
                          className="area"
                          rows={2}
                          value={step.action}
                          onChange={(e) => {
                            const next = [...steps]
                            next[idx] = { ...step, action: e.target.value }
                            setSteps(next)
                          }}
                        />
                      ) : (
                        <div style={{ whiteSpace: 'pre-wrap' }}>{step.action}</div>
                      )}
                    </div>
                    <div>
                      <div className="muted" style={{ fontSize: '0.78rem' }}>
                        Expected
                      </div>
                      {editing ? (
                        <textarea
                          className="area"
                          rows={2}
                          value={step.expected_result}
                          onChange={(e) => {
                            const next = [...steps]
                            next[idx] = { ...step, expected_result: e.target.value }
                            setSteps(next)
                          }}
                        />
                      ) : (
                        <div style={{ whiteSpace: 'pre-wrap' }}>{step.expected_result}</div>
                      )}
                      {editing && (
                        <button
                          type="button"
                          className="btn secondary"
                          style={{ marginTop: '0.35rem' }}
                          onClick={() => setSteps((prev) => prev.filter((_, i) => i !== idx))}
                        >
                          Remove
                        </button>
                      )}
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
            {canManageReqs && (
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
                    if (!activeProjectId || !caseId || !linkId) return
                    void api
                      .linkRequirement(activeProjectId, {
                        case_id: caseId,
                        requirement_id: linkId,
                      })
                      .then(() =>
                        api.listCaseRequirements(activeProjectId, caseId),
                      )
                      .then(setLinked)
                      .then(() => setLinkId(''))
                      .catch((err: Error) => setError(err.message))
                  }}
                >
                  Link
                </button>
              </div>
            )}
          </section>

          <section style={{ marginTop: '1.5rem' }}>
            <h3>Edit history</h3>
            {versions.length === 0 ? (
              <p className="muted">No snapshots yet.</p>
            ) : (
              <ul className="history-list">
                {versions.map((v) => (
                  <li key={v.id}>
                    <span className="badge">v{v.version}</span>{' '}
                    {v.change_summary || 'snapshot'}{' '}
                    <span className="muted">
                      ·{' '}
                      {formatTimestamp(v.created_at, {
                        timeZone: user?.timezone,
                        instanceTimeZone: authStatus?.default_timezone,
                        locale: user?.locale ?? authStatus?.default_locale,
                      })}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </>
      )}

      {tab === 'executions' && (
        <section className="case-executions-tab" aria-label="Case executions">
          {execLoading && !execPage ? (
            <p className="muted">Loading executions…</p>
          ) : !execPage ? (
            <p className="muted">No executions yet.</p>
          ) : (
            <>
              <div className="meta-row results-summary">
                <span>
                  Pass rate: <strong>{execPage.pass_rate.toFixed(0)}%</strong>
                </span>
                <span>Total: {execPage.total}</span>
                <Sparkline statuses={execPage.sparkline} />
              </div>
              {execPage.items.length === 0 ? (
                <p className="muted">No executions yet.</p>
              ) : (
                <div className="table-wrap">
                  <table className="table">
                    <thead>
                      <tr>
                        <th>Path</th>
                        <th>Plan</th>
                        <th>Exec #</th>
                        <th>Mode</th>
                        <th>Status</th>
                        <th>By</th>
                        <th>On</th>
                        <th>Issues</th>
                      </tr>
                    </thead>
                    <tbody>
                      {execPage.items.map((row) => (
                        <tr key={row.result_id}>
                          <td>
                            <Link
                              to={`/executions/runs/${row.run_id}/cases/${caseId}`}
                            >
                              {row.path}
                            </Link>
                          </td>
                          <td>{row.plan_type}</td>
                          <td>{row.exec_number}</td>
                          <td>{row.mode}</td>
                          <td>
                            <StatusBadge status={row.status} />
                          </td>
                          <td>{row.executed_by || '—'}</td>
                          <td>
                            {formatTimestamp(row.executed_on, {
                              timeZone: user?.timezone,
                              instanceTimeZone: authStatus?.default_timezone,
                              locale:
                                user?.locale ?? authStatus?.default_locale,
                            })}
                          </td>
                          <td>{row.linked_issues}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
              <div className="pager">
                <button
                  type="button"
                  className="btn secondary"
                  disabled={execOffset <= 0 || execLoading}
                  onClick={() =>
                    void loadExecutions(Math.max(0, execOffset - execLimit))
                  }
                >
                  Previous
                </button>
                <span className="muted">
                  {execOffset + 1}–
                  {Math.min(execOffset + execLimit, execPage.total)} of{' '}
                  {execPage.total}
                </span>
                <button
                  type="button"
                  className="btn secondary"
                  disabled={
                    execLoading || execOffset + execLimit >= execPage.total
                  }
                  onClick={() => void loadExecutions(execOffset + execLimit)}
                >
                  Next
                </button>
              </div>
            </>
          )}
        </section>
      )}

      {tab === 'history' && (
        <section className="case-results-history" aria-label="Result history">
          {summary && (
            <div className="meta-row results-summary">
              <span>
                Latest:{' '}
                {summary.latest_status ? (
                  <StatusBadge status={summary.latest_status} />
                ) : (
                  '—'
                )}
              </span>
              <span>Total results: {summary.total_results}</span>
              <span>Pass rate: {summary.pass_rate.toFixed(0)}%</span>
              <span className="muted">
                P{summary.passed} / F{summary.failed} / B{summary.blocked} / S
                {summary.skipped}
              </span>
            </div>
          )}

          <div className="form-row" style={{ marginBottom: '0.75rem' }}>
            <select
              className="filter-input"
              aria-label="Filter by status"
              value={filterStatus}
              onChange={(e) => setFilterStatus(e.target.value)}
            >
              <option value="">All statuses</option>
              <option value="Passed">Passed</option>
              <option value="Failed">Failed</option>
              <option value="Blocked">Blocked</option>
              <option value="Skipped">Skipped</option>
              <option value="Retest">Retest</option>
              <option value="Untested">Untested</option>
            </select>
            <input
              className="search-input"
              placeholder="Filter by run id"
              aria-label="Filter by run id"
              value={filterRunId}
              onChange={(e) => setFilterRunId(e.target.value)}
            />
            <button
              type="button"
              className="btn secondary"
              disabled={resultsLoading}
              onClick={() => void loadResults(0)}
            >
              Apply filters
            </button>
          </div>

          {resultsLoading && !resultsPage ? (
            <p className="muted">Loading results…</p>
          ) : !resultsPage || resultsPage.items.length === 0 ? (
            <p className="muted">No results yet.</p>
          ) : (
            <>
              <ol className="timeline-list" aria-label="Case results across runs">
                {resultsPage.items.map((h) => (
                  <li key={h.result.id}>
                    <StatusBadge status={h.result.status} />{' '}
                    <strong>{h.run_name || 'Run'}</strong>{' '}
                    <span className="muted">
                      {new Date(h.result.executed_at).toLocaleString()}
                      {h.executed_by_name ? ` · ${h.executed_by_name}` : ''}
                      {h.result.case_version != null
                        ? ` · case v${h.result.case_version}`
                        : ''}
                      {h.attachment_count > 0
                        ? ` · ${h.attachment_count} attachment(s)`
                        : ''}
                      {h.defect_count > 0 ? ` · ${h.defect_count} defect(s)` : ''}
                      {h.result.message ? ` — ${h.result.message}` : ''}
                    </span>
                  </li>
                ))}
              </ol>
              <div className="pager">
                <button
                  type="button"
                  className="btn secondary"
                  disabled={resultsOffset <= 0 || resultsLoading}
                  onClick={() => void loadResults(Math.max(0, resultsOffset - resultsLimit))}
                >
                  Previous
                </button>
                <span className="muted">
                  {resultsOffset + 1}–
                  {Math.min(resultsOffset + resultsLimit, resultsPage.total)} of{' '}
                  {resultsPage.total}
                </span>
                <button
                  type="button"
                  className="btn secondary"
                  disabled={
                    resultsLoading ||
                    resultsOffset + resultsLimit >= resultsPage.total
                  }
                  onClick={() => void loadResults(resultsOffset + resultsLimit)}
                >
                  Next
                </button>
              </div>
            </>
          )}
        </section>
      )}
    </div>
  )
}
