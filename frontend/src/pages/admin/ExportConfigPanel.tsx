import { useState, type FormEvent } from 'react'
import { api, type ConfigImportPreview, type Project } from '../../api'
import { useI18n } from '../../i18n'
import { TestIds, testIdProps } from '../../testing'

type Props = {
  projects: Project[]
  onError: (msg: string) => void
}

export function ExportConfigPanel({ projects, onError }: Props) {
  const { t } = useI18n()
  const [scope, setScope] = useState<'all' | 'selected'>('all')
  const [projectIds, setProjectIds] = useState<string[]>([])
  const [includeMemberships, setIncludeMemberships] = useState(false)
  const [includeData, setIncludeData] = useState(false)
  const [includeFolders, setIncludeFolders] = useState(true)
  const [format, setFormat] = useState<'json' | 'yaml'>('json')
  const [busy, setBusy] = useState(false)

  const [strategy, setStrategy] = useState<'merge' | 'replace'>('merge')
  const [bundle, setBundle] = useState<unknown>(null)
  const [preview, setPreview] = useState<ConfigImportPreview | null>(null)
  const [fileName, setFileName] = useState('')

  const toggleProject = (id: string) => {
    setProjectIds((prev) =>
      prev.includes(id) ? prev.filter((p) => p !== id) : [...prev, id],
    )
  }

  const onExport = async (e: FormEvent) => {
    e.preventDefault()
    if (scope === 'selected' && projectIds.length === 0) {
      onError(t('admin.export.projectsRequired'))
      return
    }
    setBusy(true)
    try {
      await api.exportAdminConfig({
        project_scope: scope,
        project_ids: scope === 'selected' ? projectIds : [],
        include_memberships: includeMemberships,
        include_data: includeData,
        include_folder_structure: includeFolders,
        format,
      })
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onFile = async (file: File | null) => {
    setPreview(null)
    setBundle(null)
    setFileName(file?.name ?? '')
    if (!file) return
    try {
      const text = await file.text()
      let parsed: unknown
      if (file.name.endsWith('.yaml') || file.name.endsWith('.yml')) {
        // Minimal YAML: try JSON first; otherwise ask user to use JSON export.
        try {
          parsed = JSON.parse(text)
        } catch {
          onError(t('admin.export.yamlNeedJson'))
          return
        }
      } else {
        parsed = JSON.parse(text)
      }
      setBundle(parsed)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }

  const onPreview = async () => {
    if (!bundle) {
      onError(t('admin.export.needFile'))
      return
    }
    setBusy(true)
    try {
      const res = await api.previewAdminConfigImport({
        bundle,
        strategy,
      })
      setPreview(res)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onApply = async () => {
    if (!bundle) {
      onError(t('admin.export.needFile'))
      return
    }
    if (!window.confirm(t('admin.export.applyConfirm'))) return
    setBusy(true)
    try {
      const res = await api.importAdminConfig({
        bundle,
        strategy,
        dry_run: false,
      })
      setPreview(res)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const renderChanges = (title: string, items: ConfigImportPreview['creates']) =>
    items.length > 0 ? (
      <div>
        <h4>{title}</h4>
        <ul>
          {items.map((c, i) => (
            <li key={`${c.kind}-${c.key}-${i}`}>
              <strong>{c.kind}</strong> {c.key}: {c.action}
              {c.detail ? <span className="muted"> — {c.detail}</span> : null}
            </li>
          ))}
        </ul>
      </div>
    ) : null

  return (
    <div className="panel admin-grid">
      <form className="stack-form" onSubmit={(e) => void onExport(e)}>
        <h3>{t('admin.export.title')}</h3>
        <p className="muted">{t('admin.export.help')}</p>
        <label className="field">
          <span>{t('admin.export.projectScope')}</span>
          <select
            value={scope}
            onChange={(e) => setScope(e.target.value as 'all' | 'selected')}
          >
            <option value="all">{t('admin.export.scopeAll')}</option>
            <option value="selected">{t('admin.export.scopeSelected')}</option>
          </select>
        </label>
        {scope === 'selected' && (
          <fieldset className="field">
            <legend>{t('admin.customFields.projects')}</legend>
            <div className="check-list">
              {projects.map((p) => (
                <label key={p.id} className="check">
                  <input
                    type="checkbox"
                    checked={projectIds.includes(p.id)}
                    onChange={() => toggleProject(p.id)}
                  />
                  {p.name} <span className="muted">({p.key})</span>
                </label>
              ))}
            </div>
          </fieldset>
        )}
        <label className="check">
          <input
            type="checkbox"
            checked={includeMemberships}
            onChange={(e) => setIncludeMemberships(e.target.checked)}
          />
          {t('admin.export.memberships')}
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={includeData}
            onChange={(e) => setIncludeData(e.target.checked)}
          />
          {t('admin.export.data')}
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={includeFolders}
            onChange={(e) => setIncludeFolders(e.target.checked)}
          />
          {t('admin.export.folders')}
        </label>
        <label className="field">
          <span>{t('admin.export.format')}</span>
          <select
            value={format}
            onChange={(e) => setFormat(e.target.value as 'json' | 'yaml')}
          >
            <option value="json">JSON</option>
            <option value="yaml">YAML</option>
          </select>
        </label>
        <button
          className="btn"
          type="submit"
          disabled={busy}
          {...testIdProps(TestIds.adminConfigExportButton)}
        >
          {t('admin.export.download')}
        </button>
      </form>

      <div className="stack-form">
        <h3>{t('admin.export.importTitle')}</h3>
        <p className="muted">{t('admin.export.importHelp')}</p>
        <label className="field">
          <span>{t('admin.export.file')}</span>
          <input
            type="file"
            accept=".json,.yaml,.yml,application/json"
            {...testIdProps(TestIds.adminConfigImportInput)}
            onChange={(e) => void onFile(e.target.files?.[0] ?? null)}
          />
          {fileName ? <span className="muted">{fileName}</span> : null}
        </label>
        <label className="field">
          <span>{t('admin.export.strategy')}</span>
          <select
            value={strategy}
            onChange={(e) =>
              setStrategy(e.target.value as 'merge' | 'replace')
            }
          >
            <option value="merge">{t('admin.export.merge')}</option>
            <option value="replace">{t('admin.export.replace')}</option>
          </select>
        </label>
        <div className="row-actions">
          <button
            type="button"
            className="btn secondary"
            disabled={busy || !bundle}
            onClick={() => void onPreview()}
          >
            {t('admin.export.preview')}
          </button>
          <button
            type="button"
            className="btn"
            disabled={busy || !bundle}
            onClick={() => void onApply()}
            {...testIdProps(TestIds.adminConfigImportButton)}
          >
            {t('admin.export.apply')}
          </button>
        </div>
        {preview && (
          <div className="import-preview">
            <p>
              {preview.applied
                ? t('admin.export.applied')
                : t('admin.export.dryRun')}{' '}
              · {preview.strategy}
            </p>
            {renderChanges(t('admin.export.creates'), preview.creates)}
            {renderChanges(t('admin.export.updates'), preview.updates)}
            {renderChanges(t('admin.export.conflicts'), preview.conflicts)}
            {renderChanges(t('admin.export.skipped'), preview.skipped)}
          </div>
        )}
      </div>
    </div>
  )
}
