import { useCallback, useEffect, useState } from 'react'
import {
  api,
  type CiConfig,
  type CiDispatch,
  type TestResult,
  type TestRun,
} from '../api'
import { useAuth } from '../auth'
import { SampleBadge } from '../components/SampleBadge'
import { StatusBadge } from '../components/StatusBadge'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { TestIds, listItemProps, testIdProps } from '../testing'
import { formatTimestamp } from '../timezone'

type Props = {
  projectId: string | null
}

export function Runs({ projectId }: Props) {
  const { user, status } = useAuth()
  const { can } = useProjectPermissions(projectId)
  const canExecute = can('execute_runs')
  const canTrigger = can('trigger_ci')
  const canManageCi = can('manage_ci_config')
  const fmt = (value: string | null | undefined) =>
    formatTimestamp(value, {
      timeZone: user?.timezone,
      instanceTimeZone: status?.default_timezone,
      locale: user?.locale ?? status?.default_locale,
    })

  const [runs, setRuns] = useState<TestRun[]>([])
  const [configs, setConfigs] = useState<CiConfig[]>([])
  const [dispatches, setDispatches] = useState<CiDispatch[]>([])
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null)
  const [results, setResults] = useState<TestResult[]>([])
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [runName, setRunName] = useState('Manual smoke')
  const [ciName, setCiName] = useState('GitHub Actions')
  const [provider, setProvider] = useState<'github' | 'gitlab'>('github')
  const [ciJson, setCiJson] = useState(
    '{\n  "token": "",\n  "owner": "org",\n  "repo": "repo",\n  "workflow_id": "ci.yml",\n  "ref": "main"\n}',
  )

  const refresh = useCallback(async () => {
    if (!projectId) {
      setRuns([])
      setConfigs([])
      setDispatches([])
      return
    }
    try {
      const [r, c, d] = await Promise.all([
        api.listRuns(projectId),
        api.listCiConfigs(projectId),
        api.listDispatches(projectId),
      ])
      setRuns(r.items)
      setConfigs(c)
      setDispatches(d)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [projectId])

  useEffect(() => {
    void refresh()
  }, [refresh])

  useEffect(() => {
    if (!selectedRunId) {
      setResults([])
      return
    }
    let cancelled = false
    api
      .listResults(selectedRunId, { limit: 500 })
      .then((data) => {
        if (!cancelled) setResults(data.items)
      })
      .catch(() => {
        if (!cancelled) setResults([])
      })
    return () => {
      cancelled = true
    }
  }, [selectedRunId])

  const onCreateRun = async () => {
    if (!projectId) return
    setBusy(true)
    try {
      const detail = await api.createRun(projectId, {
        name: runName.trim() || 'Untitled run',
        source: 'ui',
        status: 'running',
      })
      setSelectedRunId(detail.run.id)
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onSaveCi = async () => {
    if (!projectId) return
    setBusy(true)
    try {
      const config = JSON.parse(ciJson) as Record<string, unknown>
      await api.createCiConfig(projectId, {
        provider,
        name: ciName.trim() || provider,
        config,
      })
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onDispatch = async (configId: string) => {
    setBusy(true)
    try {
      await api.dispatchCi(configId, {
        run_name: `UI trigger ${new Date().toISOString()}`,
        inputs: {},
      })
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  if (!projectId) {
    return <p className="muted">Select a project to manage runs and CI.</p>
  }

  return (
    <div className="stack-lg" {...testIdProps(TestIds.runsPage)}>
      {error && (
        <p className="error" {...testIdProps(TestIds.runsError)}>
          {error}
        </p>
      )}

      <section className="panel">
        <h2>Runs</h2>
        <p className="muted" style={{ marginTop: 0 }}>
          Ingest automated results via CLI, GitHub Action, or GitLab template.
          Create a run here as an upload target, or trigger CI below.
        </p>
        {canExecute && (
        <div className="toolbar">
          <input
            value={runName}
            onChange={(e) => setRunName(e.target.value)}
            aria-label="New run name"
            placeholder="Run name"
            {...testIdProps(TestIds.runsCreateNameInput)}
          />
          <button
            type="button"
            className="btn"
            disabled={busy}
            onClick={() => void onCreateRun()}
            {...testIdProps(TestIds.runsCreateButton)}
          >
            New run
          </button>
        </div>
        )}
        {runs.length === 0 ? (
          <p className="empty" {...testIdProps(TestIds.runsEmpty)}>
            No runs yet.
          </p>
        ) : (
          <table className="table" {...testIdProps(TestIds.runsList)}>
            <thead>
              <tr>
                <th>Name</th>
                <th>Status</th>
                <th>Source</th>
                <th>Created</th>
              </tr>
            </thead>
            <tbody>
              {runs.map((run, index) => (
                <tr
                  key={run.id}
                  className={selectedRunId === run.id ? 'selected' : undefined}
                  onClick={() => setSelectedRunId(run.id)}
                  {...listItemProps(TestIds.runRow, {
                    // UUID only — run names are freeform (spaces) and must not be entity keys
                    entityId: run.id,
                    index,
                  })}
                >
                  <td {...testIdProps(TestIds.runRowName)}>
                    {run.name}
                    {run.is_sample ? (
                      <>
                        {' '}
                        <SampleBadge testId="runs-list-sample-badge" />
                      </>
                    ) : null}
                  </td>
                  <td {...testIdProps(TestIds.runRowStatus)}>
                    <StatusBadge status={run.status} testId="runs-list-status-badge" />
                  </td>
                  <td>{run.source}</td>
                  <td className="muted">{fmt(run.created_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {selectedRunId && (
          <div style={{ marginTop: '1.25rem' }}>
            <h3 style={{ fontFamily: 'var(--font-display)', margin: '0 0 0.5rem' }}>
              Results
            </h3>
            {results.length === 0 ? (
              <p className="muted">No results ingested for this run yet.</p>
            ) : (
              <table className="table">
                <thead>
                  <tr>
                    <th>Status</th>
                    <th>External id</th>
                    <th>Message</th>
                    <th>ms</th>
                  </tr>
                </thead>
                <tbody>
                  {results.map((r) => (
                    <tr key={r.id} style={{ cursor: 'default' }}>
                      <td>
                        <StatusBadge status={r.status} testId="runs-result-status-badge" />
                      </td>
                      <td>
                        <code>{r.external_id ?? '—'}</code>
                      </td>
                      <td>{r.message || '—'}</td>
                      <td>{r.duration_ms ?? '—'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        )}
      </section>

      {(canManageCi || canTrigger) && (
      <section className="panel">
        <h2>CI triggers</h2>
        <p className="muted" style={{ marginTop: 0 }}>
          Dispatch GitHub <code>workflow_dispatch</code> or a GitLab pipeline, then
          sync status through webhooks (
          <code>/api/v1/webhooks/github</code>, <code>/api/v1/webhooks/gitlab</code>
          ).
        </p>
        {canManageCi && (
        <>
        <div className="toolbar">
          <select
            value={provider}
            onChange={(e) => {
              const next = e.target.value as 'github' | 'gitlab'
              setProvider(next)
              setCiName(next === 'github' ? 'GitHub Actions' : 'GitLab pipeline')
              setCiJson(
                next === 'github'
                  ? '{\n  "token": "",\n  "owner": "org",\n  "repo": "repo",\n  "workflow_id": "ci.yml",\n  "ref": "main"\n}'
                  : '{\n  "base_url": "https://gitlab.com",\n  "project_id": "group/project",\n  "trigger_token": "",\n  "ref": "main"\n}',
              )
            }}
            aria-label="CI provider"
            {...testIdProps(TestIds.runsCiProviderSelect)}
          >
            <option value="github">GitHub</option>
            <option value="gitlab">GitLab</option>
          </select>
          <input
            value={ciName}
            onChange={(e) => setCiName(e.target.value)}
            aria-label="CI config name"
            placeholder="Config name"
            {...testIdProps(TestIds.runsCiNameInput)}
          />
          <button
            type="button"
            className="btn"
            disabled={busy}
            onClick={() => void onSaveCi()}
            {...testIdProps(TestIds.runsCiSaveButton)}
          >
            Save config
          </button>
        </div>
        <textarea
          className="code-input"
          value={ciJson}
          onChange={(e) => setCiJson(e.target.value)}
          rows={8}
          aria-label="CI config JSON"
          {...testIdProps(TestIds.runsCiJsonInput)}
        />
        </>
        )}

        {configs.length > 0 && (
          <table className="table" style={{ marginTop: '1rem' }}>
            <thead>
              <tr>
                <th>Name</th>
                <th>Provider</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {configs.map((cfg, index) => (
                <tr key={cfg.id} style={{ cursor: 'default' }}>
                  <td>{cfg.name}</td>
                  <td>{cfg.provider}</td>
                  <td>
                    {canTrigger && (
                    <button
                      type="button"
                      className="btn secondary"
                      disabled={busy || !cfg.enabled}
                      onClick={() => void onDispatch(cfg.id)}
                      {...listItemProps(TestIds.runsCiDispatchButton, {
                        entityId: cfg.id,
                        index,
                      })}
                    >
                      Dispatch
                    </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

        {dispatches.length > 0 && (
          <>
            <h3 style={{ fontFamily: 'var(--font-display)', margin: '1.25rem 0 0.5rem' }}>
              Recent dispatches
            </h3>
            <table className="table">
              <thead>
                <tr>
                  <th>Status</th>
                  <th>External</th>
                  <th>Error</th>
                  <th>When</th>
                </tr>
              </thead>
              <tbody>
                {dispatches.slice(0, 10).map((d, index) => (
                  <tr key={d.id} style={{ cursor: 'default' }}>
                    <td>
                      <StatusBadge status={d.status} testId="runs-dispatch-status-badge" />
                    </td>
                    <td>
                      {d.external_url ? (
                        <a
                          href={d.external_url}
                          target="_blank"
                          rel="noreferrer"
                          {...listItemProps(TestIds.runsCiExternalLink, {
                            entityId: d.id,
                            index,
                          })}
                        >
                          {d.external_id ?? 'open'}
                        </a>
                      ) : (
                        (d.external_id ?? '—')
                      )}
                    </td>
                    <td className="muted">{d.last_error ?? '—'}</td>
                    <td className="muted">{fmt(d.created_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
      </section>
      )}
    </div>
  )
}
