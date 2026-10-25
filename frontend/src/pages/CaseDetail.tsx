import { useEffect, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import {
  api,
  type CaseVersionSummary,
  type CustomField,
  type Requirement,
  type TestCaseDetail,
} from '../api'
import { cacheCase, getCachedCase } from '../pwa/offlineDb'

type Props = {
  projectId: string | null
}

type DraftStep = {
  action: string
  expected_result: string
  data: string
}

export function CaseDetail({ projectId }: Props) {
  const { caseId } = useParams()
  const [detail, setDetail] = useState<TestCaseDetail | null>(null)
  const [versions, setVersions] = useState<CaseVersionSummary[]>([])
  const [fields, setFields] = useState<CustomField[]>([])
  const [linked, setLinked] = useState<Requirement[]>([])
  const [allReqs, setAllReqs] = useState<Requirement[]>([])
  const [linkId, setLinkId] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [offlineNotice, setOfflineNotice] = useState(false)
  const [loading, setLoading] = useState(true)
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

  const reload = async () => {
    if (!projectId || !caseId) return
    try {
      const [data, vers, defs, reqs, all] = await Promise.all([
        api.getCase(projectId, caseId),
        api.listVersions(projectId, caseId),
        api.listCustomFields(projectId),
        api.listCaseRequirements(projectId, caseId),
        api.listRequirements(projectId),
      ])
      applyDetail(data)
      setVersions(vers)
      setFields(defs)
      setLinked(reqs)
      setAllReqs(all)
      setOfflineNotice(false)
      void cacheCase(projectId, caseId, data)
    } catch (err) {
      const cached = (await getCachedCase(
        projectId,
        caseId,
      )) as TestCaseDetail | null
      if (cached) {
        applyDetail(cached)
        setOfflineNotice(true)
        return
      }
      throw err
    }
  }

  useEffect(() => {
    if (!projectId || !caseId) {
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
  }, [projectId, caseId])

  const save = async () => {
    if (!projectId || !caseId) return
    setSaving(true)
    try {
      const tagNames = tagInput
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean)
      await api.updateCase(projectId, caseId, {
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
        await api.setCustomFields(projectId, caseId, cfBody)
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

  if (!projectId) {
    return <p className="muted">Select a project first.</p>
  }

  if (loading) {
    return <p className="muted">Loading case…</p>
  }

  if (error && !detail) {
    return <p className="error">{error}</p>
  }

  if (!detail) {
    return <p className="muted">Case not found.</p>
  }

  const { case: c, tags } = detail

  return (
    <div className="panel case-detail">
      <p style={{ marginTop: 0 }}>
        <Link to="/">← Back to cases</Link>
      </p>
      {offlineNotice && (
        <p className="notice">Showing cached case (offline / network error).</p>
      )}
      {error && <p className="error">{error}</p>}
      <div className="detail-header">
        <h2>
          <span className="badge">{c.key}</span>{' '}
          {editing ? (
            <input
              className="title-input"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
          ) : (
            c.title
          )}
        </h2>
        <div className="detail-actions">
          {!editing ? (
            <button type="button" className="btn" onClick={() => setEditing(true)}>
              Edit
            </button>
          ) : (
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
          )}
        </div>
      </div>

      {error && <p className="error">{error}</p>}

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

      <section style={{ marginTop: '1.5rem' }}>
        <h3>History</h3>
        {versions.length === 0 ? (
          <p className="muted">No snapshots yet.</p>
        ) : (
          <ul className="history-list">
            {versions.map((v) => (
              <li key={v.id}>
                <span className="badge">v{v.version}</span>{' '}
                {v.change_summary || 'snapshot'}{' '}
                <span className="muted">
                  · {new Date(v.created_at).toLocaleString()}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  )
}
