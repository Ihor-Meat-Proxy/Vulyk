import { useCallback, useEffect, useState, type FormEvent } from 'react'
import {
  api,
  type CustomField,
  type Requirement,
} from '../api'
import { CustomFieldInput } from '../components/CustomFieldInput'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { TestIds, listChildProps, listItemProps, testIdProps } from '../testing'
import { useI18n } from '../i18n'

type Props = {
  projectId: string | null
}

export function RequirementsPage({ projectId }: Props) {
  const { t } = useI18n()
  const { can } = useProjectPermissions(projectId)
  const canManage = can('manage_requirements')
  const [items, setItems] = useState<Requirement[]>([])
  const [fields, setFields] = useState<CustomField[]>([])
  const [title, setTitle] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [selected, setSelected] = useState<Requirement | null>(null)
  const [fieldValues, setFieldValues] = useState<Record<string, unknown>>({})
  const [savingCf, setSavingCf] = useState(false)

  const refresh = useCallback(async () => {
    if (!projectId) {
      setItems([])
      setFields([])
      return
    }
    setLoading(true)
    try {
      const [data, defs] = await Promise.all([
        api.listRequirements(projectId),
        api.listCustomFields(projectId, 'requirement'),
      ])
      setItems(data)
      setFields(defs)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }, [projectId])

  useEffect(() => {
    void refresh()
  }, [refresh])

  useEffect(() => {
    if (!projectId || !selectedId) {
      setSelected(null)
      setFieldValues({})
      return
    }
    let cancelled = false
    api
      .getRequirement(projectId, selectedId)
      .then((req) => {
        if (cancelled) return
        setSelected(req)
        const vals: Record<string, unknown> = {}
        for (const cf of req.custom_fields ?? []) {
          vals[cf.field_id] = cf.value ?? null
        }
        setFieldValues(vals)
      })
      .catch((err: Error) => {
        if (!cancelled) setError(err.message)
      })
    return () => {
      cancelled = true
    }
  }, [projectId, selectedId])

  if (!projectId) {
    return <p className="muted">Select a project first.</p>
  }

  const onCreate = async (e: FormEvent) => {
    e.preventDefault()
    if (!title.trim()) return
    try {
      await api.createRequirement(projectId, { title: title.trim() })
      setTitle('')
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  const saveCustomFields = async () => {
    if (!selectedId || !canManage) return
    setSavingCf(true)
    try {
      const body = fields.map((f) => ({
        field_id: f.id,
        value:
          fieldValues[f.id] !== undefined
            ? fieldValues[f.id]
            : (f.default_value ?? null),
      }))
      const updated = await api.setRequirementCustomFields(
        projectId,
        selectedId,
        body,
      )
      setSelected(updated)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSavingCf(false)
    }
  }

  return (
    <div className="panel" {...testIdProps(TestIds.requirementsPage)}>
      <h2>Requirements</h2>
      <p className="muted" style={{ marginTop: 0 }}>
        Track requirements locally or import them from OpenProject / Jira
        integrations. Link cases from the case detail view.
      </p>

      {canManage && (
        <form className="req-form" onSubmit={(e) => void onCreate(e)}>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="New requirement title"
            aria-label="Requirement title"
            {...testIdProps(TestIds.requirementsTitleInput)}
          />
          <button
            type="submit"
            className="btn"
            disabled={!title.trim()}
            {...testIdProps(TestIds.requirementsAddButton)}
          >
            Add
          </button>
        </form>
      )}

      {error && (
        <p className="error" {...testIdProps(TestIds.requirementsError)}>
          {error}
        </p>
      )}
      {loading ? (
        <p className="muted" {...testIdProps(TestIds.requirementsLoading)}>
          Loading…
        </p>
      ) : items.length === 0 ? (
        <p className="empty" {...testIdProps(TestIds.requirementsEmpty)}>
          No requirements yet.
        </p>
      ) : (
        <table
          className="table"
          {...testIdProps(TestIds.requirementsList)}
        >
          <thead>
            <tr>
              <th>Key</th>
              <th>Title</th>
              <th>Source</th>
            </tr>
          </thead>
          <tbody>
            {items.map((r, index) => (
              <tr
                key={r.id}
                className={selectedId === r.id ? 'selected' : undefined}
                onClick={() => setSelectedId(r.id)}
                style={{ cursor: 'pointer' }}
                {...listItemProps(TestIds.requirementRow, {
                  entityId: r.id,
                  entityKey: r.external_key,
                  index,
                })}
              >
                <td {...testIdProps(TestIds.requirementRowKey)}>
                  <span className="badge">{r.external_key ?? '—'}</span>
                </td>
                <td {...testIdProps(TestIds.requirementRowTitle)}>
                  {r.url ? (
                    <a
                      href={r.url}
                      target="_blank"
                      rel="noreferrer"
                      {...listChildProps(TestIds.requirementRow, 'link')}
                      onClick={(e) => e.stopPropagation()}
                    >
                      {r.title}
                    </a>
                  ) : (
                    r.title
                  )}
                </td>
                <td className="muted">{r.source}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {selected && fields.length > 0 && (
        <section style={{ marginTop: '1.25rem' }}>
          <h3>
            {t('admin.customFields')} — {selected.title}
          </h3>
          <div className="cf-grid">
            {fields.map((cf) => (
              <CustomFieldInput
                key={cf.id}
                field={cf}
                value={
                  fieldValues[cf.id] !== undefined
                    ? fieldValues[cf.id]
                    : (cf.default_value ?? null)
                }
                onChange={(next) =>
                  setFieldValues((prev) => ({ ...prev, [cf.id]: next }))
                }
                readOnly={!canManage}
                disabled={savingCf}
              />
            ))}
          </div>
          {canManage && (
            <button
              type="button"
              className="btn"
              style={{ marginTop: '0.75rem' }}
              disabled={savingCf}
              onClick={() => void saveCustomFields()}
            >
              {savingCf ? t('common.loading') : t('common.save')}
            </button>
          )}
        </section>
      )}
    </div>
  )
}
