import { useCallback, useEffect, useState } from 'react'
import {
  api,
  type CiConfig,
  type CiDispatch,
  type TestResult,
  type TestRun,
} from '../api'

type Props = {
  projectId: string | null
}

export function Runs({ projectId }: Props) {
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
      .listResults(selectedRunId)
      .then((data) => {
        if (!cancelled) setResults(data)
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
    <div className="stack-lg">
      {error && <p className="error">{error}</p>}

      <section className="panel">
        <h2>Runs</h2>
        <p className="muted" style={{ marginTop: 0 }}>
          Ingest automated results via CLI, GitHub Action, or GitLab template.
          Create a run here as an upload target, or trigger CI below.
        </p>
        <div className="toolbar">
          <input
            value={runName}
            onChange={(e) => setRunName(e.target.value)}
            aria-label="New run name"
            placeholder="Run name"
          />
          <button type="button" className="btn" disabled={busy} onClick={() => void onCreateRun()}>
            New run
          </button>
        </div>
        {runs.length === 0 ? (
          <p className="empty">No runs yet.</p>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Status</th>
                <th>Source</th>
                <th>Created</th>
              </tr>
            </thead>
            <tbody>
              {runs.map((run) => (
                <tr
                  key={run.id}
                  className={selectedRunId === run.id ? 'selected' : undefined}
                  onClick={() => setSelectedRunId(run.id)}
                >
                  <td>{run.name}</td>
                  <td>
                    <span className="badge">{run.status}</span>
                  </td>
                  <td>{run.source}</td>
                  <td className="muted">{new Date(run.created_at).toLocaleString()}</td>
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
                        <span className="badge">{r.status}</span>
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

      <section className="panel">
        <h2>CI triggers</h2>
        <p className="muted" style={{ marginTop: 0 }}>
          Dispatch GitHub <code>workflow_dispatch</code> or a GitLab pipeline, then
          sync status through webhooks (
          <code>/api/v1/webhooks/github</code>, <code>/api/v1/webhooks/gitlab</code>
          ).
        </p>
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
          >
            <option value="github">GitHub</option>
            <option value="gitlab">GitLab</option>
          </select>
          <input
            value={ciName}
            onChange={(e) => setCiName(e.target.value)}
            aria-label="CI config name"
            placeholder="Config name"
          />
          <button type="button" className="btn" disabled={busy} onClick={() => void onSaveCi()}>
            Save config
          </button>
        </div>
        <textarea
          className="code-input"
          value={ciJson}
          onChange={(e) => setCiJson(e.target.value)}
          rows={8}
          aria-label="CI config JSON"
        />

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
              {configs.map((cfg) => (
                <tr key={cfg.id} style={{ cursor: 'default' }}>
                  <td>{cfg.name}</td>
                  <td>{cfg.provider}</td>
                  <td>
                    <button
                      type="button"
                      className="btn secondary"
                      disabled={busy || !cfg.enabled}
                      onClick={() => void onDispatch(cfg.id)}
                    >
                      Dispatch
                    </button>
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
                {dispatches.slice(0, 10).map((d) => (
                  <tr key={d.id} style={{ cursor: 'default' }}>
                    <td>
                      <span className="badge">{d.status}</span>
                    </td>
                    <td>
                      {d.external_url ? (
                        <a href={d.external_url} target="_blank" rel="noreferrer">
                          {d.external_id ?? 'open'}
                        </a>
                      ) : (
                        (d.external_id ?? '—')
                      )}
                    </td>
                    <td className="muted">{d.last_error ?? '—'}</td>
                    <td className="muted">{new Date(d.created_at).toLocaleString()}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
      </section>
    </div>
  )
}
