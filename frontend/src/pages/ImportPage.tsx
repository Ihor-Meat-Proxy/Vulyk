import { useEffect, useMemo, useState } from 'react'
import {
  api,
  type FieldMapping,
  type ImportPreview,
  type ImportReport,
  type ImporterInfo,
} from '../api'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { useI18n } from '../i18n'
import { TestIds, listItemProps, testIdProps } from '../testing'

type Props = {
  projectId: string | null
}

const EMPTY_MAPPING: FieldMapping = {
  title: 'title',
  key: 'key',
  description: 'description',
  precondition: 'precondition',
  priority: 'priority',
  status: 'status',
  suite: 'suite',
  steps: 'steps',
}

export function ImportPage({ projectId }: Props) {
  const { t } = useI18n()
  const { can } = useProjectPermissions(projectId)
  const canImport = can('import_cases')
  const [importers, setImporters] = useState<ImporterInfo[]>([])
  const [importerId, setImporterId] = useState('csv')
  const [format, setFormat] = useState('csv')
  const [content, setContent] = useState('')
  const [mapping, setMapping] = useState<FieldMapping>(EMPTY_MAPPING)
  const [preview, setPreview] = useState<ImportPreview | null>(null)
  const [report, setReport] = useState<ImportReport | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    api
      .listImporters()
      .then((list) => {
        setImporters(list)
        if (list[0]) {
          setImporterId(list[0].id)
          setFormat(list[0].supported_formats[0] ?? 'csv')
        }
      })
      .catch((err: Error) => setError(err.message))
  }, [])

  const selected = useMemo(
    () => importers.find((i) => i.id === importerId) ?? null,
    [importers, importerId],
  )

  useEffect(() => {
    if (!selected) return
    if (!selected.supported_formats.includes(format)) {
      setFormat(selected.supported_formats[0] ?? 'csv')
    }
  }, [selected, format])

  if (!projectId) {
    return <p className="muted">{t('cases.selectProject')}</p>
  }

  const runPreview = async () => {
    setBusy(true)
    setError(null)
    setReport(null)
    try {
      const data = await api.previewImport(projectId, {
        importer_id: importerId,
        format,
        content,
        field_mapping: mapping,
      })
      setPreview(data)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const runImport = async () => {
    setBusy(true)
    setError(null)
    try {
      const data = (await api.runImport(projectId, {
        importer_id: importerId,
        format,
        content,
        field_mapping: mapping,
        dry_run: false,
      })) as ImportReport
      setReport(data)
      setPreview(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const setMap = (key: keyof FieldMapping, value: string) => {
    setMapping((m) => ({ ...m, [key]: value }))
  }

  if (!canImport) {
    return (
      <div className="panel">
        <p className="muted">{t('import.noPermission')}</p>
      </div>
    )
  }

  return (
    <div className="panel import-panel" {...testIdProps(TestIds.importPage)}>
      <h2>{t('import.title')}</h2>
      <p className="muted" style={{ marginTop: 0 }}>
        {t('import.subtitle')}
      </p>

      <div className="import-grid">
        <label>
          {t('import.importer')}
          <select
            value={importerId}
            onChange={(e) => setImporterId(e.target.value)}
            {...testIdProps(TestIds.importImporterSelect)}
          >
            {importers.map((i) => (
              <option key={i.id} value={i.id}>
                {i.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t('import.format')}
          <select
            value={format}
            onChange={(e) => setFormat(e.target.value)}
            {...testIdProps(TestIds.importFormatSelect)}
          >
            {(selected?.supported_formats ?? [format]).map((f) => (
              <option key={f} value={f}>
                {f}
              </option>
            ))}
          </select>
        </label>
      </div>

      <label className="stack-label">
        {t('import.source')}
        <textarea
          className="import-source"
          rows={10}
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder={t('import.sourcePlaceholder')}
          spellCheck={false}
          {...testIdProps(TestIds.importFileInput)}
        />
      </label>

      <h3>{t('import.fieldMapping')}</h3>
      <div className="mapping-grid">
        {(Object.keys(EMPTY_MAPPING) as (keyof FieldMapping)[]).map(
          (key, index) => (
            <label key={key}>
              {key}
              <input
                value={mapping[key]}
                onChange={(e) => setMap(key, e.target.value)}
                {...listItemProps(TestIds.importMappingInput, {
                  entityId: key,
                  entityKey: key,
                  index,
                })}
              />
            </label>
          ),
        )}
      </div>

      <div className="import-actions">
        <button
          type="button"
          className="btn secondary"
          disabled={busy || !content.trim()}
          onClick={() => void runPreview()}
          {...testIdProps(TestIds.importPreviewButton)}
        >
          {t('import.preview')}
        </button>
        <button
          type="button"
          className="btn"
          disabled={busy || !content.trim()}
          onClick={() => void runImport()}
          {...testIdProps(TestIds.importSubmitButton)}
        >
          {t('import.submit')}
        </button>
      </div>

      {error && (
        <p className="error" {...testIdProps(TestIds.importError)}>
          {error}
        </p>
      )}

      {preview && (
        <section className="preview-block">
          <h3>{t('import.previewHeading', { count: preview.total })}</h3>
          {preview.warnings.length > 0 && (
            <p className="muted">{preview.warnings.join(' · ')}</p>
          )}
          <table className="table">
            <thead>
              <tr>
                <th>{t('cases.colKey')}</th>
                <th>{t('cases.colTitle')}</th>
                <th>{t('import.colSuite')}</th>
                <th>{t('cases.steps')}</th>
              </tr>
            </thead>
            <tbody>
              {preview.cases.slice(0, 50).map((c, idx) => (
                <tr key={`${c.key ?? 'row'}-${idx}`}>
                  <td>
                    <span className="badge">{c.key ?? '—'}</span>
                  </td>
                  <td>{c.title}</td>
                  <td>{c.suite_name ?? '—'}</td>
                  <td>{c.steps.length}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {report && (
        <section
          className="preview-block"
          {...testIdProps(TestIds.importSuccess)}
        >
          <h3>{t('import.complete')}</h3>
          <p>
            {t('import.createdCases', { count: report.created_cases })}
            {report.created_suites
              ? ` ${t('import.andSuites', { count: report.created_suites })}`
              : ''}
            {report.skipped
              ? ` ${t('import.skipped', { count: report.skipped })}`
              : ''}
            .
          </p>
        </section>
      )}
    </div>
  )
}
