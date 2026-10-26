import {
  Fragment,
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useState,
} from 'react'
import {
  Link,
  useNavigate,
  useParams,
  useSearchParams,
} from 'react-router-dom'
import {
  api,
  type RunCaseResultExpand,
  type RunCaseResultRow,
  type RunReportDetail as RunReportDetailType,
  type RunResultGroup,
  type RunResultsParams,
} from '../api'
import { StatusBadge } from '../components/StatusBadge'
import { useBackspaceNavigate } from '../hooks/useBackspaceNavigate'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { useI18n } from '../i18n'
import { TestIds, listItemProps, testIdProps } from '../testing'
import { formatTimestamp } from '../timezone'

type Props = {
  projectId: string | null
}

const PAGE_SIZE = 50

function formatDuration(ms: number | null | undefined): string {
  if (ms == null || Number.isNaN(ms)) return '—'
  if (ms < 1000) return `${ms} ms`
  const s = ms / 1000
  if (s < 60) return `${s.toFixed(1)} s`
  const m = Math.floor(s / 60)
  const rem = Math.round(s % 60)
  return `${m}m ${rem}s`
}

function comparisonClass(c: string | null): string {
  if (!c) return ''
  return `comparison-pill comparison-${c}`
}

export function RunReportDetail({ projectId }: Props) {
  const { runId } = useParams()
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  const { t } = useI18n()
  const { can } = useProjectPermissions(projectId)
  const canExport = can('export_reports')

  const statusFilter = searchParams.get('status') ?? ''
  const folderFilter = searchParams.get('folder') ?? ''
  const priorityFilter = searchParams.get('priority') ?? ''
  const assigneeFilter = searchParams.get('assignee') ?? ''
  const qFilter = searchParams.get('q') ?? ''
  const sort = searchParams.get('sort') ?? 'key_asc'
  const groupBy =
    (searchParams.get('group_by') as 'folder' | 'status' | null) ?? null
  const offset = Number(searchParams.get('offset') ?? '0') || 0

  const deferredQ = useDeferredValue(qFilter)

  const [detail, setDetail] = useState<RunReportDetailType | null>(null)
  const [rows, setRows] = useState<RunCaseResultRow[]>([])
  const [total, setTotal] = useState(0)
  const [groups, setGroups] = useState<RunResultGroup[]>([])
  const [expanded, setExpanded] = useState<
    Record<string, RunCaseResultExpand | 'loading' | 'error'>
  >({})
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const goBack = useCallback(() => {
    navigate('/reports')
  }, [navigate])
  useBackspaceNavigate(goBack)

  const patchParams = useCallback(
    (patch: Record<string, string | null>, resetOffset = true) => {
      setSearchParams((prev) => {
        const next = new URLSearchParams(prev)
        for (const [k, v] of Object.entries(patch)) {
          if (v == null || v === '') next.delete(k)
          else next.set(k, v)
        }
        if (resetOffset) next.delete('offset')
        return next
      })
    },
    [setSearchParams],
  )

  const listParams: RunResultsParams = useMemo(
    () => ({
      status: statusFilter || undefined,
      folder: folderFilter || undefined,
      priority: priorityFilter || undefined,
      assignee: assigneeFilter || undefined,
      q: deferredQ || undefined,
      sort,
      limit: PAGE_SIZE,
      offset,
    }),
    [
      statusFilter,
      folderFilter,
      priorityFilter,
      assigneeFilter,
      deferredQ,
      sort,
      offset,
    ],
  )

  const refresh = useCallback(async () => {
    if (!projectId || !runId) return
    setBusy(true)
    try {
      const d = await api.getRunReportDetail(projectId, runId)
      setDetail(d)
      if (groupBy) {
        const g = await api.getRunResultGroups(projectId, runId, {
          group_by: groupBy,
          status: listParams.status,
          folder: listParams.folder,
          priority: listParams.priority,
          assignee: listParams.assignee,
          q: listParams.q,
        })
        setGroups(g.groups)
        setRows([])
        setTotal(0)
      } else {
        const page = await api.getRunCaseResults(projectId, runId, listParams)
        setRows(page.items)
        setTotal(page.total)
        setGroups([])
      }
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }, [projectId, runId, groupBy, listParams])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const toggleExpand = async (row: RunCaseResultRow) => {
    if (!projectId || !runId) return
    if (expanded[row.case_id]) {
      setExpanded((prev) => {
        const next = { ...prev }
        delete next[row.case_id]
        return next
      })
      return
    }
    setExpanded((prev) => ({ ...prev, [row.case_id]: 'loading' }))
    try {
      const data = await api.getRunCaseExpand(projectId, runId, row.case_id)
      setExpanded((prev) => ({ ...prev, [row.case_id]: data }))
    } catch {
      setExpanded((prev) => ({ ...prev, [row.case_id]: 'error' }))
    }
  }

  const onExport = async (format: 'csv' | 'pdf') => {
    if (!projectId || !runId) return
    try {
      await api.downloadReportExport(projectId, {
        format,
        report: 'run_results',
        runId,
        status: statusFilter || undefined,
        folder: folderFilter || undefined,
        priority: priorityFilter || undefined,
        assignee: assigneeFilter || undefined,
        q: qFilter || undefined,
        sort,
      })
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  if (!projectId) {
    return <p className="muted">{t('reports.selectProject')}</p>
  }

  return (
    <div
      className="reports-page run-report-page"
      {...testIdProps(TestIds.reportsRunPage)}
    >
      <nav
        className="breadcrumbs"
        aria-label={t('reports.breadcrumbs')}
        {...testIdProps(TestIds.reportsRunBreadcrumbs)}
      >
        <Link
          to="/reports"
          {...testIdProps(TestIds.reportsRunBreadcrumbReports)}
        >
          {t('reports.title')}
        </Link>
        <span aria-hidden>/</span>
        <span>{detail?.name ?? runId}</span>
      </nav>

      <div className="panel-header">
        <div>
          <h2 {...testIdProps(TestIds.reportsRunTitle)}>
            {detail?.name ?? t('reports.runDetail')}
          </h2>
          <p className="muted">{t('reports.runDetailSubtitle')}</p>
        </div>
        <div className="topbar-actions">
          <button
            type="button"
            className="btn secondary"
            onClick={goBack}
            {...testIdProps(TestIds.reportsRunBack)}
          >
            {t('reports.back')}
          </button>
          <button
            type="button"
            className="btn secondary"
            onClick={() => void refresh()}
            disabled={busy}
            {...testIdProps(TestIds.reportsRunRefresh)}
          >
            {t('reports.refresh')}
          </button>
          {canExport && (
            <button
              type="button"
              className="btn secondary"
              onClick={() => void onExport('csv')}
              {...testIdProps(TestIds.reportsRunExportCsv)}
            >
              {t('reports.exportCsv')}
            </button>
          )}
          {canExport && (
            <button
              type="button"
              className="btn"
              onClick={() => void onExport('pdf')}
              {...testIdProps(TestIds.reportsRunExportPdf)}
            >
              {t('reports.exportPdf')}
            </button>
          )}
        </div>
      </div>

      {error && (
        <p className="error" {...testIdProps(TestIds.reportsRunError)}>
          {error}
        </p>
      )}

      {detail && (
        <section
          className="run-report-header panel"
          {...testIdProps(TestIds.reportsRunHeader)}
        >
          <dl className="run-meta-grid">
            <div>
              <dt>{t('reports.source')}</dt>
              <dd {...testIdProps(TestIds.reportsRunSource)}>
                {detail.source_kind === 'manual'
                  ? t('reports.sourceManual')
                  : detail.source_kind === 'ci'
                    ? t('reports.sourceCi')
                    : t('reports.sourceAutomated')}
                <span className="muted"> ({detail.source})</span>
              </dd>
            </div>
            <div>
              <dt>{t('reports.status')}</dt>
              <dd>
                <StatusBadge status={detail.status} />
              </dd>
            </div>
            <div>
              <dt>{t('reports.environment')}</dt>
              <dd {...testIdProps(TestIds.reportsRunEnvironment)}>
                {detail.environment_label ?? '—'}
              </dd>
            </div>
            <div>
              <dt>{t('reports.branch')}</dt>
              <dd {...testIdProps(TestIds.reportsRunBranch)}>
                {detail.branch ?? '—'}
              </dd>
            </div>
            <div>
              <dt>{t('reports.commit')}</dt>
              <dd {...testIdProps(TestIds.reportsRunCommit)}>
                {detail.commit ? (
                  <code>{detail.commit.slice(0, 12)}</code>
                ) : (
                  '—'
                )}
              </dd>
            </div>
            <div>
              <dt>{t('reports.pipeline')}</dt>
              <dd>
                {detail.pipeline_url ? (
                  <a
                    href={detail.pipeline_url}
                    target="_blank"
                    rel="noreferrer"
                    {...testIdProps(TestIds.reportsRunPipeline)}
                  >
                    {t('reports.pipelineLink')}
                  </a>
                ) : (
                  '—'
                )}
              </dd>
            </div>
            <div>
              <dt>{t('reports.started')}</dt>
              <dd>
                {detail.started_at ? formatTimestamp(detail.started_at) : '—'}
              </dd>
            </div>
            <div>
              <dt>{t('reports.finished')}</dt>
              <dd>
                {detail.finished_at ? formatTimestamp(detail.finished_at) : '—'}
              </dd>
            </div>
            <div>
              <dt>{t('reports.duration')}</dt>
              <dd {...testIdProps(TestIds.reportsRunDuration)}>
                {formatDuration(detail.duration_ms)}
              </dd>
            </div>
            <div>
              <dt>{t('reports.ranBy')}</dt>
              <dd {...testIdProps(TestIds.reportsRunRanBy)}>
                {detail.created_by_name ?? '—'}
              </dd>
            </div>
          </dl>

          <section
            className="report-kpis"
            aria-label={t('reports.kpis')}
            {...testIdProps(TestIds.reportsRunKpis)}
          >
            <div className="kpi">
              <span className="kpi-label">{t('reports.kpiTotal')}</span>
              <span className="kpi-value">{detail.counts.total}</span>
            </div>
            <div className="kpi">
              <span className="kpi-label">{t('status.passed')}</span>
              <span className="kpi-value">{detail.counts.passed}</span>
            </div>
            <div className="kpi">
              <span className="kpi-label">{t('status.failed')}</span>
              <span className="kpi-value">{detail.counts.failed}</span>
            </div>
            <div className="kpi">
              <span className="kpi-label">{t('reports.passRate')}</span>
              <span className="kpi-value">{detail.pass_rate.toFixed(1)}%</span>
            </div>
          </section>
        </section>
      )}

      <section
        className="panel run-report-filters"
        {...testIdProps(TestIds.reportsRunFilters)}
      >
        <div className="filter-row">
          <label>
            <span className="sr-only">{t('reports.filterStatus')}</span>
            <select
              value={statusFilter}
              onChange={(e) => patchParams({ status: e.target.value || null })}
              {...testIdProps(TestIds.reportsRunFilterStatus)}
            >
              <option value="">{t('reports.filterStatusAll')}</option>
              <option value="passed">{t('status.passed')}</option>
              <option value="failed">{t('status.failed')}</option>
              <option value="blocked">{t('status.blocked')}</option>
              <option value="skipped">{t('status.skipped')}</option>
              <option value="untested">{t('status.untested')}</option>
              <option value="retest">{t('status.retest')}</option>
            </select>
          </label>
          <label>
            <span className="sr-only">{t('reports.filterFolder')}</span>
            <input
              type="search"
              placeholder={t('reports.filterFolder')}
              value={folderFilter}
              onChange={(e) => patchParams({ folder: e.target.value || null })}
              {...testIdProps(TestIds.reportsRunFilterFolder)}
            />
          </label>
          <label>
            <span className="sr-only">{t('reports.filterPriority')}</span>
            <select
              value={priorityFilter}
              onChange={(e) =>
                patchParams({ priority: e.target.value || null })
              }
              {...testIdProps(TestIds.reportsRunFilterPriority)}
            >
              <option value="">{t('reports.filterPriorityAll')}</option>
              <option value="critical">critical</option>
              <option value="high">high</option>
              <option value="medium">medium</option>
              <option value="low">low</option>
            </select>
          </label>
          <label>
            <span className="sr-only">{t('reports.filterAssignee')}</span>
            <input
              type="search"
              placeholder={t('reports.filterAssignee')}
              value={assigneeFilter}
              onChange={(e) =>
                patchParams({ assignee: e.target.value || null })
              }
              {...testIdProps(TestIds.reportsRunFilterAssignee)}
            />
          </label>
          <label>
            <span className="sr-only">{t('reports.filterSearch')}</span>
            <input
              type="search"
              placeholder={t('reports.filterSearch')}
              value={qFilter}
              onChange={(e) => patchParams({ q: e.target.value || null })}
              {...testIdProps(TestIds.reportsRunFilterSearch)}
            />
          </label>
          <label>
            <span className="sr-only">{t('reports.sort')}</span>
            <select
              value={sort}
              onChange={(e) => patchParams({ sort: e.target.value })}
              {...testIdProps(TestIds.reportsRunSort)}
            >
              <option value="key_asc">{t('reports.sortKeyAsc')}</option>
              <option value="key_desc">{t('reports.sortKeyDesc')}</option>
              <option value="title_asc">{t('reports.sortTitleAsc')}</option>
              <option value="status_asc">{t('reports.sortStatusAsc')}</option>
              <option value="duration_desc">
                {t('reports.sortDurationDesc')}
              </option>
              <option value="folder_asc">{t('reports.sortFolderAsc')}</option>
            </select>
          </label>
        </div>
        <div
          className="filter-row group-toggle"
          role="group"
          aria-label={t('reports.grouping')}
        >
          <button
            type="button"
            className={`btn secondary${!groupBy ? ' active' : ''}`}
            onClick={() => patchParams({ group_by: null })}
            {...testIdProps(TestIds.reportsRunGroupNone)}
          >
            {t('reports.groupNone')}
          </button>
          <button
            type="button"
            className={`btn secondary${groupBy === 'folder' ? ' active' : ''}`}
            onClick={() => patchParams({ group_by: 'folder' })}
            {...testIdProps(TestIds.reportsRunGroupFolder)}
          >
            {t('reports.groupFolder')}
          </button>
          <button
            type="button"
            className={`btn secondary${groupBy === 'status' ? ' active' : ''}`}
            onClick={() => patchParams({ group_by: 'status' })}
            {...testIdProps(TestIds.reportsRunGroupStatus)}
          >
            {t('reports.groupStatus')}
          </button>
        </div>
      </section>

      {groupBy ? (
        <section className="panel" {...testIdProps(TestIds.reportsRunGroups)}>
          <h3>{t('reports.groups')}</h3>
          {!groups.length && <p className="muted">{t('reports.noResults')}</p>}
          <ul className="report-list">
            {groups.map((g, idx) => (
              <li key={g.key}>
                <button
                  type="button"
                  className="group-row"
                  {...listItemProps(TestIds.reportsRunGroupRow, {
                    entityId: `${idx}`,
                    index: idx,
                  })}
                  onClick={() => {
                    if (groupBy === 'status') {
                      patchParams({ group_by: null, status: g.key })
                    } else {
                      patchParams({ group_by: null, folder: g.key })
                    }
                  }}
                >
                  <strong>{g.label}</strong>
                  <span className="muted">
                    {g.counts.total} · {g.pass_rate.toFixed(1)}% ·{' '}
                    {g.counts.passed}P / {g.counts.failed}F
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      ) : (
        <section className="panel" {...testIdProps(TestIds.reportsRunResults)}>
          <div className="panel-header compact">
            <h3>
              {t('reports.caseResults')}{' '}
              <span className="muted">({total})</span>
            </h3>
          </div>
          {!rows.length && <p className="muted">{t('reports.noResults')}</p>}
          <div className="table-wrap">
            <table
              className="data-table run-results-table"
              {...testIdProps(TestIds.reportsRunResultsTable)}
            >
              <thead>
                <tr>
                  <th scope="col">{t('reports.colKey')}</th>
                  <th scope="col">{t('reports.colTitle')}</th>
                  <th scope="col">{t('reports.colFolder')}</th>
                  <th scope="col">{t('reports.colStatus')}</th>
                  <th scope="col">{t('reports.colExecutedBy')}</th>
                  <th scope="col">{t('reports.colDuration')}</th>
                  <th scope="col">{t('reports.colComment')}</th>
                  <th scope="col">{t('reports.colArtifacts')}</th>
                  <th scope="col">{t('reports.colDefects')}</th>
                  <th scope="col">{t('reports.colPrevious')}</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((row, rowIdx) => {
                  const exp = expanded[row.case_id]
                  const open = Boolean(exp)
                  return (
                    <Fragment key={row.case_id}>
                      <tr
                        {...listItemProps(TestIds.reportsRunRow, {
                          entityId: row.case_id,
                          entityKey: row.case_key,
                          index: rowIdx,
                        })}
                        className={open ? 'expanded' : undefined}
                      >
                        <td>
                          <button
                            type="button"
                            className="linkish expand-btn"
                            aria-expanded={open}
                            onClick={() => void toggleExpand(row)}
                            {...listItemProps(TestIds.reportsRunRowExpand, {
                              entityId: row.case_id,
                              entityKey: row.case_key,
                              index: rowIdx,
                            })}
                          >
                            {open ? '▾' : '▸'}
                          </button>{' '}
                          <Link
                            to={`/cases/${encodeURIComponent(row.case_key)}`}
                            {...listItemProps(TestIds.reportsRunRowKey, {
                              entityId: row.case_id,
                              entityKey: row.case_key,
                              index: rowIdx,
                            })}
                          >
                            {row.case_key}
                          </Link>
                        </td>
                        <td>{row.case_title}</td>
                        <td className="muted">{row.folder_path}</td>
                        <td>
                          <StatusBadge status={row.status} />
                        </td>
                        <td>{row.executed_by_name ?? row.assignee ?? '—'}</td>
                        <td>{formatDuration(row.duration_ms)}</td>
                        <td className="comment-cell">{row.comment || '—'}</td>
                        <td>
                          {row.attachment_count > 0
                            ? row.attachments
                                .map((a) => a.filename)
                                .join(', ') || String(row.attachment_count)
                            : '—'}
                        </td>
                        <td>
                          {row.defects.length
                            ? row.defects.map((d) => (
                                <a
                                  key={d.id}
                                  href={d.external_url || '#'}
                                  target="_blank"
                                  rel="noreferrer"
                                >
                                  {d.external_key ?? d.title}
                                </a>
                              ))
                            : row.defect_count || '—'}
                        </td>
                        <td>
                          {row.previous_status ? (
                            <StatusBadge status={row.previous_status} />
                          ) : (
                            '—'
                          )}
                          {row.comparison && (
                            <span
                              className={comparisonClass(row.comparison)}
                              {...listItemProps(TestIds.reportsRunComparison, {
                                entityId: row.case_id,
                                entityKey: row.case_key,
                                index: rowIdx,
                              })}
                            >
                              {t(`reports.comparison.${row.comparison}`)}
                            </span>
                          )}
                        </td>
                      </tr>
                      {open && (
                        <tr
                          className="expand-row"
                          {...listItemProps(TestIds.reportsRunRowDetail, {
                            entityId: row.case_id,
                            entityKey: row.case_key,
                            index: rowIdx,
                          })}
                        >
                          <td colSpan={10}>
                            {exp === 'loading' && (
                              <p className="muted">{t('common.loading')}</p>
                            )}
                            {exp === 'error' && (
                              <p className="error">
                                {t('reports.expandError')}
                              </p>
                            )}
                            {exp && exp !== 'loading' && exp !== 'error' && (
                              <div className="expand-panels">
                                <div>
                                  <h4>{t('reports.steps')}</h4>
                                  <ol className="step-list">
                                    {exp.steps.map((s) => (
                                      <li key={s.position}>
                                        <StatusBadge status={s.status} />{' '}
                                        <strong>
                                          {t('reports.stepAction')}:
                                        </strong>{' '}
                                        {s.action}{' '}
                                        <strong>
                                          {t('reports.stepExpected')}:
                                        </strong>{' '}
                                        {s.expected_result}{' '}
                                        {s.actual_result && (
                                          <>
                                            <strong>
                                              {t('reports.stepActual')}:
                                            </strong>{' '}
                                            {s.actual_result}
                                          </>
                                        )}
                                      </li>
                                    ))}
                                  </ol>
                                </div>
                                <div>
                                  <h4>{t('reports.commentHistory')}</h4>
                                  {!exp.comment_history.length && (
                                    <p className="muted">
                                      {t('reports.noComments')}
                                    </p>
                                  )}
                                  <ul className="comment-history">
                                    {exp.comment_history.map((c, idx) => (
                                      <li key={`${c.result_id}-${idx}`}>
                                        <span className="muted">
                                          {formatTimestamp(c.executed_at)}
                                          {c.executed_by_name
                                            ? ` · ${c.executed_by_name}`
                                            : ''}
                                        </span>
                                        {c.status && (
                                          <StatusBadge status={c.status} />
                                        )}
                                        <div>{c.message || '—'}</div>
                                      </li>
                                    ))}
                                  </ul>
                                </div>
                              </div>
                            )}
                          </td>
                        </tr>
                      )}
                    </Fragment>
                  )
                })}
              </tbody>
            </table>
          </div>
          <div
            className="pagination-row"
            {...testIdProps(TestIds.reportsRunPagination)}
          >
            <button
              type="button"
              className="btn secondary"
              disabled={offset <= 0 || busy}
              onClick={() =>
                patchParams(
                  { offset: String(Math.max(0, offset - PAGE_SIZE)) },
                  false,
                )
              }
              {...testIdProps(TestIds.reportsRunPagePrev)}
            >
              {t('reports.prevPage')}
            </button>
            <span className="muted">
              {total === 0
                ? '0'
                : `${offset + 1}–${Math.min(offset + PAGE_SIZE, total)} / ${total}`}
            </span>
            <button
              type="button"
              className="btn secondary"
              disabled={offset + PAGE_SIZE >= total || busy}
              onClick={() =>
                patchParams({ offset: String(offset + PAGE_SIZE) }, false)
              }
              {...testIdProps(TestIds.reportsRunPageNext)}
            >
              {t('reports.nextPage')}
            </button>
          </div>
        </section>
      )}
    </div>
  )
}
