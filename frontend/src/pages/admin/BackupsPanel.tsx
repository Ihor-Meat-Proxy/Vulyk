import { useCallback, useEffect, useState, type FormEvent } from 'react'
import {
  api,
  type BackupDestination,
  type BackupPolicy,
  type BackupRun,
  type Project,
  type RestorePreview,
} from '../../api'
import { useI18n } from '../../i18n'

type Props = {
  projects: Project[]
  onError: (msg: string | null) => void
}

const KINDS = ['local', 'github', 'gitlab', 'nextcloud', 'gdrive'] as const

export function BackupsPanel({ projects, onError }: Props) {
  const { t } = useI18n()
  const [destinations, setDestinations] = useState<BackupDestination[]>([])
  const [policies, setPolicies] = useState<BackupPolicy[]>([])
  const [runs, setRuns] = useState<BackupRun[]>([])
  const [busy, setBusy] = useState(false)
  const [testMsg, setTestMsg] = useState<string | null>(null)
  const [preview, setPreview] = useState<RestorePreview | null>(null)

  const [destName, setDestName] = useState('')
  const [destKind, setDestKind] = useState<string>('local')
  const [destPath, setDestPath] = useState('./data/content-backups')
  const [destOwner, setDestOwner] = useState('')
  const [destRepo, setDestRepo] = useState('')
  const [destBranch, setDestBranch] = useState('main')
  const [destRemotePath, setDestRemotePath] = useState('vulyk-backups')
  const [destBaseUrl, setDestBaseUrl] = useState('')
  const [destUsername, setDestUsername] = useState('')
  const [destFolderId, setDestFolderId] = useState('')
  const [destSecret, setDestSecret] = useState('')

  const [runDestId, setRunDestId] = useState('')
  const [runProjectId, setRunProjectId] = useState('')
  const [packMode, setPackMode] = useState('zip')

  const [policyDestId, setPolicyDestId] = useState('')
  const [policySchedule, setPolicySchedule] = useState('daily')
  const [policyProjectId, setPolicyProjectId] = useState('')
  const [retainCount, setRetainCount] = useState('10')

  const load = useCallback(async () => {
    onError(null)
    try {
      const [d, p, r] = await Promise.all([
        api.listBackupDestinations(),
        api.listBackupPolicies(),
        api.listBackupRuns({ limit: 50 }),
      ])
      setDestinations(d)
      setPolicies(p)
      setRuns(r)
      if (!runDestId && d[0]) setRunDestId(d[0].id)
      if (!policyDestId && d[0]) setPolicyDestId(d[0].id)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }, [onError, runDestId, policyDestId])

  useEffect(() => {
    void load()
  }, [load])

  function buildConfig(): Record<string, unknown> {
    switch (destKind) {
      case 'local':
        return { path: destPath }
      case 'github':
        return {
          owner: destOwner,
          repo: destRepo,
          branch: destBranch,
          path: destRemotePath,
        }
      case 'gitlab':
        return {
          project_id: destRepo,
          branch: destBranch,
          path: destRemotePath,
          api_base: destBaseUrl || undefined,
        }
      case 'nextcloud':
        return {
          base_url: destBaseUrl,
          username: destUsername,
          path: destRemotePath,
        }
      case 'gdrive':
        return { folder_id: destFolderId }
      default:
        return {}
    }
  }

  function buildSecrets(): Record<string, unknown> | undefined {
    if (!destSecret.trim()) return undefined
    if (destKind === 'nextcloud') return { app_password: destSecret }
    if (destKind === 'gdrive') return { access_token: destSecret }
    return { token: destSecret }
  }

  async function onCreateDest(e: FormEvent) {
    e.preventDefault()
    setBusy(true)
    onError(null)
    try {
      await api.createBackupDestination({
        name: destName,
        kind: destKind,
        config: buildConfig(),
        secrets: buildSecrets(),
        enabled: true,
      })
      setDestName('')
      setDestSecret('')
      await load()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  async function onTest(id: string) {
    setBusy(true)
    setTestMsg(null)
    try {
      const res = await api.testBackupDestination(id)
      setTestMsg(res.ok ? res.message : `Failed: ${res.message}`)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  async function onCreatePolicy(e: FormEvent) {
    e.preventDefault()
    setBusy(true)
    try {
      await api.createBackupPolicy({
        destination_id: policyDestId,
        project_id: policyProjectId || null,
        schedule: policySchedule,
        debounce_minutes: 30,
        retain_count: retainCount ? Number(retainCount) : null,
        pack_mode: 'zip',
        include_attachments: true,
        enabled: true,
      })
      await load()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  async function onRunNow() {
    if (!runDestId) return
    setBusy(true)
    onError(null)
    try {
      await api.runBackupNow({
        destination_id: runDestId,
        project_id: runProjectId || null,
        pack_mode: packMode,
        include_attachments: true,
      })
      await load()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  async function onDownload(id: string) {
    try {
      await api.downloadBackupRun(id)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }

  async function onRestorePreview(runId: string) {
    setBusy(true)
    setPreview(null)
    try {
      const res = await api.restoreBackup({
        run_id: runId,
        mode: 'new_project',
        conflict: 'skip',
        dry_run: true,
      })
      setPreview(res)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="panel" data-testid="admin-backups-panel">
      <h3>{t('admin.backups')}</h3>
      <p className="muted">{t('admin.backupsHint')}</p>
      {testMsg && (
        <p className="muted" data-testid="admin-backups-test-msg">
          {testMsg}
        </p>
      )}

      <section data-testid="admin-backups-destinations">
        <h4>{t('admin.backupsDestinations')}</h4>
        <ul className="admin-list">
          {destinations.map((d) => (
            <li key={d.id} data-testid={`admin-backups-dest-${d.kind}`}>
              <strong>{d.name}</strong> ({d.kind})
              {d.has_secrets ? ` · ${t('admin.backupsSecretsMasked')}` : ''}
              <button
                type="button"
                data-testid="admin-backups-dest-test"
                disabled={busy}
                onClick={() => void onTest(d.id)}
              >
                {t('admin.backupsTestConnection')}
              </button>
              <button
                type="button"
                data-testid="admin-backups-dest-delete"
                disabled={busy}
                onClick={() =>
                  void api.deleteBackupDestination(d.id).then(load)
                }
              >
                {t('admin.backupsDelete')}
              </button>
            </li>
          ))}
        </ul>
        <form
          onSubmit={onCreateDest}
          className="admin-form"
          data-testid="admin-backups-dest-form"
        >
          <label>
            {t('admin.backupsDestName')}
            <input
              data-testid="admin-backups-dest-name"
              value={destName}
              onChange={(e) => setDestName(e.target.value)}
              required
            />
          </label>
          <label>
            {t('admin.backupsDestKind')}
            <select
              data-testid="admin-backups-dest-kind"
              value={destKind}
              onChange={(e) => setDestKind(e.target.value)}
            >
              {KINDS.map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </label>
          {destKind === 'local' && (
            <label>
              {t('admin.backupsPath')}
              <input
                data-testid="admin-backups-dest-path"
                value={destPath}
                onChange={(e) => setDestPath(e.target.value)}
              />
            </label>
          )}
          {(destKind === 'github' || destKind === 'gitlab') && (
            <>
              {destKind === 'github' && (
                <label>
                  Owner
                  <input
                    value={destOwner}
                    onChange={(e) => setDestOwner(e.target.value)}
                    required
                  />
                </label>
              )}
              <label>
                {destKind === 'github' ? 'Repo' : 'Project id'}
                <input
                  value={destRepo}
                  onChange={(e) => setDestRepo(e.target.value)}
                  required
                />
              </label>
              <label>
                Branch
                <input
                  value={destBranch}
                  onChange={(e) => setDestBranch(e.target.value)}
                />
              </label>
              <label>
                Path
                <input
                  value={destRemotePath}
                  onChange={(e) => setDestRemotePath(e.target.value)}
                />
              </label>
            </>
          )}
          {destKind === 'nextcloud' && (
            <>
              <label>
                Base URL
                <input
                  value={destBaseUrl}
                  onChange={(e) => setDestBaseUrl(e.target.value)}
                  required
                />
              </label>
              <label>
                Username
                <input
                  value={destUsername}
                  onChange={(e) => setDestUsername(e.target.value)}
                  required
                />
              </label>
              <label>
                Path
                <input
                  value={destRemotePath}
                  onChange={(e) => setDestRemotePath(e.target.value)}
                />
              </label>
            </>
          )}
          {destKind === 'gdrive' && (
            <label>
              Folder id
              <input
                value={destFolderId}
                onChange={(e) => setDestFolderId(e.target.value)}
                required
              />
            </label>
          )}
          {destKind !== 'local' && (
            <label>
              {t('admin.backupsSecret')}
              <input
                type="password"
                data-testid="admin-backups-dest-secret"
                value={destSecret}
                onChange={(e) => setDestSecret(e.target.value)}
                autoComplete="off"
              />
            </label>
          )}
          <button
            type="submit"
            data-testid="admin-backups-dest-save"
            disabled={busy}
          >
            {t('admin.backupsAddDestination')}
          </button>
        </form>
      </section>

      <section data-testid="admin-backups-policies">
        <h4>{t('admin.backupsPolicies')}</h4>
        <ul className="admin-list">
          {policies.map((p) => (
            <li key={p.id}>
              {p.schedule} → {p.destination_id.slice(0, 8)}…
              {p.project_id ? ` · project ${p.project_id.slice(0, 8)}` : ' · all'}
            </li>
          ))}
        </ul>
        <form onSubmit={onCreatePolicy} className="admin-form">
          <label>
            {t('admin.backupsDestination')}
            <select
              value={policyDestId}
              onChange={(e) => setPolicyDestId(e.target.value)}
              required
            >
              {destinations.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t('admin.backupsSchedule')}
            <select
              value={policySchedule}
              onChange={(e) => setPolicySchedule(e.target.value)}
            >
              <option value="hourly">hourly</option>
              <option value="daily">daily</option>
              <option value="manual_only">manual_only</option>
            </select>
          </label>
          <label>
            {t('admin.backupsProject')}
            <select
              value={policyProjectId}
              onChange={(e) => setPolicyProjectId(e.target.value)}
            >
              <option value="">{t('admin.backupsAllProjects')}</option>
              {projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.key}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t('admin.backupsRetainCount')}
            <input
              value={retainCount}
              onChange={(e) => setRetainCount(e.target.value)}
            />
          </label>
          <button type="submit" disabled={busy || !policyDestId}>
            {t('admin.backupsAddPolicy')}
          </button>
        </form>
      </section>

      <section data-testid="admin-backups-manual">
        <h4>{t('admin.backupsRunNow')}</h4>
        <div className="admin-form">
          <label>
            {t('admin.backupsDestination')}
            <select
              data-testid="admin-backups-run-dest"
              value={runDestId}
              onChange={(e) => setRunDestId(e.target.value)}
            >
              {destinations.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t('admin.backupsProject')}
            <select
              data-testid="admin-backups-run-project"
              value={runProjectId}
              onChange={(e) => setRunProjectId(e.target.value)}
            >
              <option value="">{t('admin.backupsAllProjects')}</option>
              {projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.key}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t('admin.backupsPackMode')}
            <select
              value={packMode}
              onChange={(e) => setPackMode(e.target.value)}
            >
              <option value="zip">zip</option>
              <option value="unpacked">unpacked</option>
            </select>
          </label>
          <button
            type="button"
            data-testid="admin-backups-run-now"
            disabled={busy || !runDestId}
            onClick={() => void onRunNow()}
          >
            {t('admin.backupsRunNow')}
          </button>
        </div>
      </section>

      <section data-testid="admin-backups-history">
        <h4>{t('admin.backupsHistory')}</h4>
        <table className="admin-table">
          <thead>
            <tr>
              <th>{t('admin.backupsStarted')}</th>
              <th>{t('admin.backupsStatus')}</th>
              <th>{t('admin.backupsSize')}</th>
              <th>{t('admin.backupsDuration')}</th>
              <th>{t('admin.backupsActions')}</th>
            </tr>
          </thead>
          <tbody>
            {runs.map((r) => (
              <tr key={r.id} data-testid="admin-backups-run-row">
                <td>{new Date(r.started_at).toLocaleString()}</td>
                <td data-testid="admin-backups-run-status">{r.status}</td>
                <td>{r.size_bytes}</td>
                <td>{r.duration_ms ?? '—'}</td>
                <td>
                  {r.status === 'success' && (
                    <>
                      <button
                        type="button"
                        data-testid="admin-backups-download"
                        onClick={() => void onDownload(r.id)}
                      >
                        {t('admin.backupsDownload')}
                      </button>
                      <button
                        type="button"
                        data-testid="admin-backups-restore"
                        onClick={() => void onRestorePreview(r.id)}
                      >
                        {t('admin.backupsRestore')}
                      </button>
                    </>
                  )}
                  {r.error && (
                    <span className="error" title={r.error}>
                      {t('admin.backupsError')}
                    </span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      {preview && (
        <section data-testid="admin-backups-restore-preview">
          <h4>{t('admin.backupsRestorePreview')}</h4>
          <p>
            {preview.project_key} · {preview.mode} · schema{' '}
            {preview.schema_version}
          </p>
          <pre data-testid="admin-backups-restore-counts">
            {JSON.stringify(preview.counts, null, 2)}
          </pre>
          {preview.conflicts.length > 0 && (
            <ul>
              {preview.conflicts.map((c) => (
                <li key={`${c.entity}-${c.key}`}>
                  {c.entity} {c.key}: {c.detail}
                </li>
              ))}
            </ul>
          )}
        </section>
      )}
    </div>
  )
}
