import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import {
  api,
  type CiConfig,
  type CiDispatch,
  type TestResult,
  type TestRun,
} from '../api'
import { useAuth } from '../auth'
import { DocsLink } from '../components/DocsLink'
import { SampleBadge } from '../components/SampleBadge'
import { StatusBadge } from '../components/StatusBadge'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { TestIds, listItemProps, testIdProps } from '../testing'
import { useI18n } from '../i18n'
import { formatTimestamp } from '../timezone'

type Props = {
  projectId: string | null
}

type CiProvider = 'github' | 'gitlab' | 'jenkins'

const GITHUB_JSON = `{
  "token": "",
  "owner": "org",
  "repo": "repo",
  "workflow_id": "ci.yml",
  "ref": "main"
}`

const GITLAB_JSON = `{
  "base_url": "https://gitlab.com",
  "project_id": "group/project",
  "trigger_token": "",
  "ref": "main"
}`

export function Runs({ projectId }: Props) {
  const { t } = useI18n()
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
  const [runName, setRunName] = useState(() => t('runs.manualSmoke'))
  const [ciName, setCiName] = useState(() => t('runs.githubActions'))
  const [provider, setProvider] = useState<CiProvider>('github')
  const [ciJson, setCiJson] = useState(GITHUB_JSON)
  const [jenkinsBaseUrl, setJenkinsBaseUrl] = useState(
    'https://jenkins.example.com',
  )
  const [jenkinsJobPath, setJenkinsJobPath] = useState('folder/e2e')
  const [jenkinsUsername, setJenkinsUsername] = useState('')
  const [jenkinsToken, setJenkinsToken] = useState('')
  const [jenkinsUseCrumb, setJenkinsUseCrumb] = useState(true)
  const [testMessage, setTestMessage] = useState<string | null>(null)

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

  const applyProviderDefaults = (next: CiProvider) => {
    setProvider(next)
    setTestMessage(null)
    if (next === 'github') {
      setCiName(t('runs.githubActions'))
      setCiJson(GITHUB_JSON)
    } else if (next === 'gitlab') {
      setCiName(t('runs.gitlabPipeline'))
      setCiJson(GITLAB_JSON)
    } else {
      setCiName(t('runs.jenkinsJob'))
    }
  }

  const onCreateRun = async () => {
    if (!projectId) return
    setBusy(true)
    try {
      const detail = await api.createRun(projectId, {
        name: runName.trim() || t('runs.untitledRun'),
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

  const buildJenkinsConfig = () => ({
    base_url: jenkinsBaseUrl.trim(),
    job_path: jenkinsJobPath.trim(),
    username: jenkinsUsername.trim(),
    token: jenkinsToken,
    use_crumb: jenkinsUseCrumb,
  })

  const onSaveCi = async () => {
    if (!projectId) return
    setBusy(true)
    setTestMessage(null)
    try {
      const config =
        provider === 'jenkins'
          ? buildJenkinsConfig()
          : (JSON.parse(ciJson) as Record<string, unknown>)
      await api.createCiConfig(projectId, {
        provider,
        name: ciName.trim() || provider,
        config,
      })
      if (provider === 'jenkins') {
        setJenkinsToken('')
      }
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onDispatch = async (configId: string) => {
    setBusy(true)
    setTestMessage(null)
    try {
      await api.dispatchCi(configId, {
        run_name: t('runs.uiTrigger', { time: new Date().toISOString() }),
        inputs: {},
      })
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onTestConnection = async (configId: string) => {
    setBusy(true)
    setTestMessage(null)
    try {
      const res = await api.testCiConfig(configId)
      setTestMessage(
        res.ok
          ? t('runs.testConnectionOk', { message: res.message })
          : t('runs.testConnectionFail', { message: res.message }),
      )
    } catch (err) {
      setTestMessage(
        t('runs.testConnectionFail', {
          message: err instanceof Error ? err.message : String(err),
        }),
      )
    } finally {
      setBusy(false)
    }
  }

  if (!projectId) {
    return <p className="muted">{t('runs.selectProjectCi')}</p>
  }

  return (
    <div className="stack-lg" {...testIdProps(TestIds.runsPage)}>
      {error && (
        <p className="error" {...testIdProps(TestIds.runsError)}>
          {error}
        </p>
      )}

      <section className="panel">
        <h2>{t('runs.title')}</h2>
        <p className="muted" style={{ marginTop: 0 }}>
          {t('runs.ciIngestHelp')}{' '}
          <Link
            to="/settings#ci-tokens"
            {...testIdProps(TestIds.runsCiTokensLink)}
          >
            {t('runs.ciTokensLink')}
          </Link>{' '}
          <DocsLink to="/docs/cicd" testIdSuffix="ci-ingest" />
        </p>
        {canExecute && (
          <div className="toolbar">
            <input
              value={runName}
              onChange={(e) => setRunName(e.target.value)}
              aria-label={t('a11y.newRunName')}
              placeholder={t('a11y.placeholder.runName')}
              {...testIdProps(TestIds.runsCreateNameInput)}
            />
            <button
              type="button"
              className="btn"
              disabled={busy}
              onClick={() => void onCreateRun()}
              {...testIdProps(TestIds.runsCreateButton)}
            >
              {t('runs.newRun')}
            </button>
          </div>
        )}
        {runs.length === 0 ? (
          <p className="empty" {...testIdProps(TestIds.runsEmpty)}>
            {t('runs.empty')}
          </p>
        ) : (
          <table className="table" {...testIdProps(TestIds.runsList)}>
            <thead>
              <tr>
                <th>{t('runs.colName')}</th>
                <th>{t('runs.colStatus')}</th>
                <th>{t('runs.colSource')}</th>
                <th>{t('runs.colCreated')}</th>
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
                    <StatusBadge
                      status={run.status}
                      testId="runs-list-status-badge"
                    />
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
            <h3
              style={{
                fontFamily: 'var(--font-display)',
                margin: '0 0 0.5rem',
              }}
            >
              {t('runs.results')}
            </h3>
            {results.length === 0 ? (
              <p className="muted">{t('runs.noResults')}</p>
            ) : (
              <table className="table">
                <thead>
                  <tr>
                    <th>{t('runs.colStatus')}</th>
                    <th>{t('runs.colExternalId')}</th>
                    <th>{t('runs.colMessage')}</th>
                    <th>{t('runs.colDurationMs')}</th>
                  </tr>
                </thead>
                <tbody>
                  {results.map((r) => (
                    <tr key={r.id} style={{ cursor: 'default' }}>
                      <td>
                        <StatusBadge
                          status={r.status}
                          testId="runs-result-status-badge"
                        />
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
          <h2>{t('runs.ciTriggers')}</h2>
          <p className="muted" style={{ marginTop: 0 }}>
            {t('runs.ciTriggersHelp')}{' '}
            <DocsLink to="/docs/cicd" testIdSuffix="ci-triggers" />
          </p>
          {canManageCi && (
            <>
              <div className="toolbar">
                <select
                  value={provider}
                  onChange={(e) =>
                    applyProviderDefaults(e.target.value as CiProvider)
                  }
                  aria-label={t('a11y.ciProvider')}
                  {...testIdProps(TestIds.runsCiProviderSelect)}
                >
                  <option value="github">GitHub</option>
                  <option value="gitlab">GitLab</option>
                  <option value="jenkins">Jenkins</option>
                </select>
                <input
                  value={ciName}
                  onChange={(e) => setCiName(e.target.value)}
                  aria-label={t('a11y.ciConfigName')}
                  placeholder={t('a11y.placeholder.configName')}
                  {...testIdProps(TestIds.runsCiNameInput)}
                />
                <button
                  type="button"
                  className="btn"
                  disabled={busy}
                  onClick={() => void onSaveCi()}
                  {...testIdProps(TestIds.runsCiSaveButton)}
                >
                  {t('runs.saveConfig')}
                </button>
              </div>
              {provider === 'jenkins' ? (
                <div className="stack" style={{ gap: '0.75rem' }}>
                  <label>
                    <span className="muted">{t('runs.ciBaseUrl')}</span>
                    <input
                      value={jenkinsBaseUrl}
                      onChange={(e) => setJenkinsBaseUrl(e.target.value)}
                      aria-label={t('a11y.ciBaseUrl')}
                      placeholder="https://jenkins.example.com"
                      {...testIdProps(TestIds.runsCiBaseUrlInput)}
                    />
                  </label>
                  <label>
                    <span className="muted">{t('runs.ciJobPath')}</span>
                    <input
                      value={jenkinsJobPath}
                      onChange={(e) => setJenkinsJobPath(e.target.value)}
                      aria-label={t('a11y.ciJobPath')}
                      placeholder="folder/e2e"
                      {...testIdProps(TestIds.runsCiJobPathInput)}
                    />
                  </label>
                  <label>
                    <span className="muted">{t('runs.ciUsername')}</span>
                    <input
                      value={jenkinsUsername}
                      onChange={(e) => setJenkinsUsername(e.target.value)}
                      aria-label={t('a11y.ciUsername')}
                      autoComplete="username"
                      {...testIdProps(TestIds.runsCiUsernameInput)}
                    />
                  </label>
                  <label>
                    <span className="muted">{t('runs.ciToken')}</span>
                    <input
                      type="password"
                      value={jenkinsToken}
                      onChange={(e) => setJenkinsToken(e.target.value)}
                      aria-label={t('a11y.ciToken')}
                      autoComplete="new-password"
                      {...testIdProps(TestIds.runsCiTokenInput)}
                    />
                  </label>
                  <label
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: '0.5rem',
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={jenkinsUseCrumb}
                      onChange={(e) => setJenkinsUseCrumb(e.target.checked)}
                      aria-label={t('a11y.ciUseCrumb')}
                      {...testIdProps(TestIds.runsCiUseCrumbCheckbox)}
                    />
                    <span>{t('runs.ciUseCrumb')}</span>
                  </label>
                </div>
              ) : (
                <textarea
                  className="code-input"
                  value={ciJson}
                  onChange={(e) => setCiJson(e.target.value)}
                  rows={8}
                  aria-label={t('a11y.ciConfigJson')}
                  {...testIdProps(TestIds.runsCiJsonInput)}
                />
              )}
            </>
          )}

          {testMessage && (
            <p
              className="muted"
              style={{ marginTop: '0.75rem' }}
              {...testIdProps(TestIds.runsCiTestConnectionResult)}
            >
              {testMessage}
            </p>
          )}

          {configs.length > 0 && (
            <table className="table" style={{ marginTop: '1rem' }}>
              <thead>
                <tr>
                  <th>{t('runs.colName')}</th>
                  <th>{t('runs.colProvider')}</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {configs.map((cfg, index) => (
                  <tr key={cfg.id} style={{ cursor: 'default' }}>
                    <td>{cfg.name}</td>
                    <td>{cfg.provider}</td>
                    <td>
                      <div
                        className="toolbar"
                        style={{ justifyContent: 'flex-end' }}
                      >
                        {canManageCi && cfg.provider === 'jenkins' && (
                          <button
                            type="button"
                            className="btn secondary"
                            disabled={busy || !cfg.enabled}
                            onClick={() => void onTestConnection(cfg.id)}
                            {...listItemProps(
                              TestIds.runsCiTestConnectionButton,
                              {
                                entityId: cfg.id,
                                index,
                              },
                            )}
                          >
                            {t('runs.testConnection')}
                          </button>
                        )}
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
                            {t('runs.dispatch')}
                          </button>
                        )}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}

          {dispatches.length > 0 && (
            <>
              <h3
                style={{
                  fontFamily: 'var(--font-display)',
                  margin: '1.25rem 0 0.5rem',
                }}
              >
                {t('runs.recentDispatches')}
              </h3>
              <table className="table">
                <thead>
                  <tr>
                    <th>{t('runs.colStatus')}</th>
                    <th>{t('runs.colExternal')}</th>
                    <th>{t('runs.colError')}</th>
                    <th>{t('runs.colWhen')}</th>
                  </tr>
                </thead>
                <tbody>
                  {dispatches.slice(0, 10).map((d, index) => (
                    <tr key={d.id} style={{ cursor: 'default' }}>
                      <td>
                        <StatusBadge
                          status={d.status}
                          testId="runs-dispatch-status-badge"
                        />
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
                            {d.external_id ?? t('runs.openExternal')}
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
