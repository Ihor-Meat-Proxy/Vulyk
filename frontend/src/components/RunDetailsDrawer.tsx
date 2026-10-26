import { useEffect, useId, useState } from 'react'
import { Link } from 'react-router-dom'
import { api, type AutomatedSuite, type SuiteResultRow } from '../api'
import { useAuth } from '../auth'
import { useI18n } from '../i18n'
import { formatTimestamp } from '../timezone'
import { StatusBadge } from './StatusBadge'

type Props = {
  projectId: string
  suite: AutomatedSuite | null
  open: boolean
  onClose: () => void
}

function envVarsText(vars: Record<string, unknown>): string {
  const entries = Object.entries(vars ?? {})
  if (entries.length === 0) return ''
  return entries
    .map(([k, v]) => `${k}=${typeof v === 'string' ? v : JSON.stringify(v)}`)
    .join('\n')
}

export function RunDetailsDrawer({ projectId, suite, open, onClose }: Props) {
  const { t } = useI18n()
  const { user, status: authStatus } = useAuth()
  const titleId = useId()
  const [items, setItems] = useState<SuiteResultRow[]>([])
  const [total, setTotal] = useState(0)
  const [offset, setOffset] = useState(0)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [expanded, setExpanded] = useState<Record<string, boolean>>({})
  const limit = 50

  const fmt = (value: string | null | undefined) =>
    formatTimestamp(value, {
      timeZone: user?.timezone,
      instanceTimeZone: authStatus?.default_timezone,
      locale: user?.locale ?? authStatus?.default_locale,
    })

  useEffect(() => {
    if (!open || !suite) {
      setItems([])
      return
    }
    let cancelled = false
    setLoading(true)
    api
      .listSuiteResults(projectId, suite.id, { limit, offset: 0 })
      .then((page) => {
        if (cancelled) return
        setItems(page.items)
        setTotal(page.total)
        setOffset(0)
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
  }, [open, suite?.id, projectId])

  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [open, onClose])

  const loadMore = async (nextOffset: number) => {
    if (!suite) return
    setLoading(true)
    try {
      const page = await api.listSuiteResults(projectId, suite.id, {
        limit,
        offset: nextOffset,
      })
      setItems(page.items)
      setTotal(page.total)
      setOffset(nextOffset)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }

  if (!open || !suite) return null

  return (
    <div className="exec-drawer-root" role="presentation">
      <button
        type="button"
        className="exec-drawer-backdrop"
        aria-label={t('executions.closeDrawer')}
        onClick={onClose}
      />
      <aside
        className="exec-drawer panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <div className="exec-drawer-header">
          <div>
            <h2 id={titleId} style={{ margin: 0 }}>
              {suite.name}
            </h2>
            <div className="meta-row">
              <StatusBadge status={suite.status} />
              <Link to={`/executions/runs/${suite.id}`}>
                {t('executions.details')}
              </Link>
            </div>
          </div>
          <button type="button" className="btn secondary" onClick={onClose}>
            {t('executions.closeDrawer')}
          </button>
        </div>

        <h3>{t('executions.results')}</h3>
        {error ? <p className="error">{error}</p> : null}
        {loading && items.length === 0 ? (
          <p className="muted">{t('common.loading')}</p>
        ) : items.length === 0 ? (
          <p className="muted">{t('executions.noResults')}</p>
        ) : (
          <ul className="exec-result-list">
            {items.map((row) => {
              const isOpen = Boolean(expanded[row.result_id])
              const envText = envVarsText(row.environment_vars)
              return (
                <li key={row.result_id} className="exec-result-row">
                  <button
                    type="button"
                    className="exec-result-toggle"
                    aria-expanded={isOpen}
                    onClick={() =>
                      setExpanded((prev) => ({
                        ...prev,
                        [row.result_id]: !prev[row.result_id],
                      }))
                    }
                  >
                    <StatusBadge status={row.status} />
                    <span className="exec-result-title">
                      {row.case_key ? (
                        <span className="badge">{row.case_key}</span>
                      ) : null}{' '}
                      {row.case_title || row.external_id || row.result_id}
                    </span>
                    <span className="muted">{fmt(row.executed_at)}</span>
                  </button>
                  {isOpen ? (
                    <div className="exec-result-body">
                      {row.case_id ? (
                        <p>
                          <Link
                            to={`/executions/runs/${suite.id}/cases/${row.case_id}`}
                          >
                            {t('executions.executionView')}
                          </Link>
                          {' · '}
                          <Link to={`/cases/${row.case_id}`}>
                            {t('executions.caseInfo')}
                          </Link>
                        </p>
                      ) : null}
                      {row.environment ? (
                        <p>
                          <strong>{t('executions.env')}:</strong>{' '}
                          {row.environment}
                        </p>
                      ) : null}
                      {envText ? (
                        <pre className="exec-pre">{envText}</pre>
                      ) : null}
                      {row.dataset ? (
                        <p>
                          <strong>{t('executions.dataset')}:</strong>{' '}
                          {row.dataset}
                        </p>
                      ) : null}
                      {row.message ? (
                        <pre className="exec-pre">{row.message}</pre>
                      ) : null}
                      {row.attachment_count > 0 ? (
                        <p className="muted">
                          {t('executions.attachments')}: {row.attachment_count}
                        </p>
                      ) : null}
                      {row.duration_ms != null ? (
                        <p className="muted">{row.duration_ms}ms</p>
                      ) : null}
                    </div>
                  ) : null}
                </li>
              )
            })}
          </ul>
        )}

        {total > limit ? (
          <div className="pager">
            <button
              type="button"
              className="btn secondary"
              disabled={offset <= 0 || loading}
              onClick={() => void loadMore(Math.max(0, offset - limit))}
            >
              ←
            </button>
            <span className="muted">
              {offset + 1}–{Math.min(offset + limit, total)} / {total}
            </span>
            <button
              type="button"
              className="btn secondary"
              disabled={loading || offset + limit >= total}
              onClick={() => void loadMore(offset + limit)}
            >
              →
            </button>
          </div>
        ) : null}
      </aside>
    </div>
  )
}
