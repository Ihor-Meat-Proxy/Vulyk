import { useEffect, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import {
  api,
  type AutomatedSuite,
  type AttachmentMeta,
  type Requirement,
  type RunItemExecution,
  type SuiteResultRow,
  type TestRunDetail,
} from '../api'
import { StatusBadge } from '../components/StatusBadge'
import { useAuth } from '../auth'
import { useI18n } from '../i18n'
import { formatTimestamp } from '../timezone'

type Props = {
  projectId: string | null
}

/** Suite-focused run view and optional case execution (#N) detail. */
export function ExecutionPage({ projectId }: Props) {
  const { runId, caseId } = useParams()
  const { t } = useI18n()
  const { user, status: authStatus } = useAuth()
  const [runDetail, setRunDetail] = useState<TestRunDetail | null>(null)
  const [results, setResults] = useState<SuiteResultRow[]>([])
  const [active, setActive] = useState<RunItemExecution | null>(null)
  const [requirements, setRequirements] = useState<Requirement[]>([])
  const [attachments, setAttachments] = useState<AttachmentMeta[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const fmt = (value: string | null | undefined) =>
    formatTimestamp(value, {
      timeZone: user?.timezone,
      instanceTimeZone: authStatus?.default_timezone,
      locale: user?.locale ?? authStatus?.default_locale,
    })

  useEffect(() => {
    if (!projectId || !runId) {
      setLoading(false)
      return
    }
    let cancelled = false
    setLoading(true)
    Promise.all([
      api.getRun(projectId, runId),
      api.listSuiteResults(projectId, runId, { limit: 100, offset: 0 }),
    ])
      .then(async ([detail, page]) => {
        if (cancelled) return
        setRunDetail(detail)
        setResults(page.items)
        setError(null)

        if (caseId) {
          const row = detail.items.find((i) => i.item.case_id === caseId)
          if (row) {
            const exec = await api.getRunItem(projectId, runId, row.item.id)
            if (cancelled) return
            setActive(exec)
            const [reqs, atts] = await Promise.all([
              api.listCaseRequirements(projectId, caseId).catch(() => []),
              exec.latest_result
                ? api
                    .listAttachments(projectId, {
                      entity_type: 'result',
                      entity_id: exec.latest_result.result.id,
                    })
                    .catch(() => [])
                : Promise.resolve([]),
            ])
            if (cancelled) return
            setRequirements(reqs)
            setAttachments(atts)
          } else {
            // Automated suites may lack run_items — synthesize from suite result.
            const resultRow = page.items.find((r) => r.case_id === caseId)
            if (resultRow?.case_id) {
              const caseDetail = await api.getCase(projectId, resultRow.case_id)
              if (cancelled) return
              setActive({
                item: {
                  item: {
                    id: resultRow.result_id,
                    run_id: runId,
                    case_id: resultRow.case_id,
                    assignee: null,
                    status: resultRow.status,
                    sort_order: 0,
                    comment: resultRow.message,
                    executed_at: resultRow.executed_at,
                    created_at: resultRow.executed_at,
                    updated_at: resultRow.executed_at,
                  },
                  case_key: resultRow.case_key || caseDetail.case.key,
                  case_title: resultRow.case_title || caseDetail.case.title,
                  case_priority: caseDetail.case.priority,
                },
                steps: caseDetail.steps,
                latest_result: {
                  result: {
                    id: resultRow.result_id,
                    run_id: runId,
                    case_id: resultRow.case_id,
                    run_item_id: null,
                    status: resultRow.status,
                    message: resultRow.message,
                    source: detail.run.source,
                    external_id: resultRow.external_id,
                    duration_ms: resultRow.duration_ms,
                    assignee: null,
                    elapsed_ms: resultRow.duration_ms,
                    executed_by: null,
                    case_version: caseDetail.case.version,
                    executed_at: resultRow.executed_at,
                  },
                  steps: [],
                },
                history: [],
              })
              const [reqs, atts] = await Promise.all([
                api.listCaseRequirements(projectId, caseId).catch(() => []),
                api
                  .listAttachments(projectId, {
                    entity_type: 'result',
                    entity_id: resultRow.result_id,
                  })
                  .catch(() => []),
              ])
              if (cancelled) return
              setRequirements(reqs)
              setAttachments(atts)
            }
          }
        } else {
          setActive(null)
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
  }, [projectId, runId, caseId])

  if (!projectId) return <p className="muted">Select a project first.</p>
  if (loading) return <p className="muted">{t('common.loading')}</p>
  if (error && !runDetail) return <p className="error">{error}</p>
  if (!runDetail || !runId) return <p className="muted">Run not found.</p>

  const run = runDetail.run
  const backHref = run.iteration_id
    ? `/executions/iterations/${run.iteration_id}`
    : '/executions'

  const suiteShell: AutomatedSuite = {
    id: run.id,
    project_id: run.project_id,
    iteration_id: run.iteration_id ?? null,
    name: run.name,
    status: run.status,
    source: run.source,
    created_by_name: run.created_by_name ?? null,
    created_at: run.created_at,
    started_at: run.started_at,
    finished_at: run.finished_at,
    duration_ms: run.duration_ms ?? null,
    pass_count: run.pass_count ?? 0,
    fail_count: run.fail_count ?? 0,
    skip_count: run.skip_count ?? 0,
    blocked_count: run.blocked_count ?? 0,
    environment: run.environment ?? {},
    artifacts: run.artifacts ?? {},
    external_ref: run.external_ref,
  }

  // Case execution detail
  if (caseId && active) {
    const latest = active.latest_result
    const execNumber =
      results.findIndex((r) => r.case_id === caseId) + 1 || 1
    return (
      <div className="panel execution-case-page">
        <p style={{ marginTop: 0 }}>
          <Link to={`/executions/runs/${runId}`}>← {suiteShell.name}</Link>
          {' · '}
          <Link to={backHref}>{t('executions.back')}</Link>
        </p>
        {error ? <p className="error">{error}</p> : null}
        <div className="detail-header">
          <h2 style={{ margin: 0 }}>
            {t('executions.executionView')} #{execNumber}{' '}
            <StatusBadge status={active.item.item.status} />
          </h2>
        </div>
        <div className="meta-row">
          <span className="badge">{active.item.case_key}</span>
          <Link to={`/cases/${active.item.item.case_id}`}>
            {active.item.case_title}
          </Link>
          <span className="muted">{active.item.case_priority}</span>
          {active.item.item.executed_at ? (
            <span className="muted">{fmt(active.item.item.executed_at)}</span>
          ) : null}
        </div>

        <section style={{ marginTop: '1rem' }}>
          <h3>{t('executions.caseInfo')}</h3>
          <p className="muted" style={{ marginTop: 0 }}>
            {latest?.result.message || active.item.item.comment || '—'}
          </p>
        </section>

        <section>
          <h3>{t('executions.steps')}</h3>
          {active.steps.length === 0 && (latest?.steps.length ?? 0) === 0 ? (
            <p className="muted">—</p>
          ) : (
            <div className="steps">
              {(latest?.steps.length ? latest.steps : active.steps).map(
                (step, idx) => {
                  const status =
                    'status' in step ? String(step.status) : 'Untested'
                  const action =
                    'action' in step ? String(step.action) : ''
                  const expected =
                    'expected_result' in step
                      ? String(step.expected_result)
                      : ''
                  const actual =
                    'actual_result' in step
                      ? String(step.actual_result || '')
                      : ''
                  return (
                    <div className="step" key={'id' in step ? String(step.id) : idx}>
                      <div className="step-num">{idx + 1}</div>
                      <div>
                        <div className="muted" style={{ fontSize: '0.78rem' }}>
                          Action
                        </div>
                        <div style={{ whiteSpace: 'pre-wrap' }}>{action}</div>
                      </div>
                      <div>
                        <div className="muted" style={{ fontSize: '0.78rem' }}>
                          Expected
                        </div>
                        <div style={{ whiteSpace: 'pre-wrap' }}>{expected}</div>
                        <div style={{ marginTop: '0.35rem' }}>
                          <StatusBadge status={status} />
                        </div>
                        {actual ? (
                          <pre className="exec-pre" style={{ marginTop: '0.35rem' }}>
                            {actual}
                          </pre>
                        ) : null}
                      </div>
                    </div>
                  )
                },
              )}
            </div>
          )}
        </section>

        <section style={{ marginTop: '1rem' }}>
          <h3>{t('executions.requirements')}</h3>
          {requirements.length === 0 ? (
            <p className="muted">—</p>
          ) : (
            <ul>
              {requirements.map((r) => (
                <li key={r.id}>{r.title}</li>
              ))}
            </ul>
          )}
        </section>

        <section style={{ marginTop: '1rem' }}>
          <h3>{t('executions.comments')}</h3>
          <p style={{ whiteSpace: 'pre-wrap' }}>
            {latest?.result.message || active.item.item.comment || '—'}
          </p>
        </section>

        <section style={{ marginTop: '1rem' }}>
          <h3>{t('executions.attachments')}</h3>
          {attachments.length === 0 ? (
            <p className="muted">—</p>
          ) : (
            <ul>
              {attachments.map((a) => (
                <li key={a.id}>
                  {a.filename}{' '}
                  <span className="muted">
                    ({a.content_type}, {a.size_bytes}b)
                  </span>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    )
  }

  // Suite focus
  return (
    <div className="panel execution-suite-page">
      <p style={{ marginTop: 0 }}>
        <Link to={backHref}>← {t('executions.back')}</Link>
      </p>
      {error ? <p className="error">{error}</p> : null}
      <div className="detail-header">
        <h2 style={{ margin: 0 }}>{suiteShell.name}</h2>
        <StatusBadge status={suiteShell.status} />
      </div>
      <div className="meta-row">
        <span className="muted">{suiteShell.source}</span>
        {suiteShell.created_by_name ? (
          <span>{suiteShell.created_by_name}</span>
        ) : null}
        <span className="muted">{fmt(suiteShell.created_at)}</span>
        <span className="muted">
          P{suiteShell.pass_count} / F{suiteShell.fail_count} / S
          {suiteShell.skip_count} / B{suiteShell.blocked_count}
        </span>
      </div>

      <h3>{t('executions.results')}</h3>
      {results.length === 0 ? (
        <p className="muted">{t('executions.noResults')}</p>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>{t('executions.caseInfo')}</th>
              <th>{t('executions.colStatus')}</th>
              <th>{t('executions.env')}</th>
              <th>{t('executions.executedOn')}</th>
              <th>{t('executions.colActions')}</th>
            </tr>
          </thead>
          <tbody>
            {results.map((row) => (
              <tr key={row.result_id}>
                <td>
                  {row.case_key ? (
                    <span className="badge">{row.case_key}</span>
                  ) : null}{' '}
                  {row.case_title || row.external_id || '—'}
                </td>
                <td>
                  <StatusBadge status={row.status} />
                </td>
                <td>{row.environment || '—'}</td>
                <td>{fmt(row.executed_at)}</td>
                <td>
                  {row.case_id ? (
                    <Link
                      to={`/executions/runs/${runId}/cases/${row.case_id}`}
                    >
                      {t('executions.executionView')}
                    </Link>
                  ) : (
                    '—'
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  )
}
