import { useCallback, useEffect, useState, type FormEvent } from 'react'
import {
  api,
  type CustomFieldDefinitionDetail,
  type OptionList,
  type Project,
} from '../../api'
import { useI18n } from '../../i18n'
import { TestIds, testIdProps } from '../../testing'

const FIELD_TYPES = [
  'text',
  'long_text',
  'number',
  'checkbox',
  'dropdown',
  'multi_select',
  'date',
  'url',
] as const

type Props = {
  projects: Project[]
  onError: (msg: string) => void
}

type FormState = {
  key: string
  name: string
  field_type: string
  entity: string
  option_list_id: string
  default_value: string
  required: boolean
  help_text: string
  sort_order: number
  show_in_list: boolean
  show_in_filters: boolean
  active: boolean
  scope: string
  project_ids: string[]
}

const emptyForm = (): FormState => ({
  key: '',
  name: '',
  field_type: 'text',
  entity: 'test_case',
  option_list_id: '',
  default_value: '',
  required: false,
  help_text: '',
  sort_order: 0,
  show_in_list: false,
  show_in_filters: false,
  active: true,
  scope: 'all',
  project_ids: [],
})

function parseDefault(fieldType: string, raw: string): unknown {
  const trimmed = raw.trim()
  if (!trimmed) return null
  if (fieldType === 'checkbox') return trimmed === 'true' || trimmed === '1'
  if (fieldType === 'number') {
    const n = Number(trimmed)
    return Number.isFinite(n) ? n : null
  }
  if (fieldType === 'multi_select') {
    try {
      const parsed = JSON.parse(trimmed) as unknown
      return Array.isArray(parsed) ? parsed : trimmed.split(',').map((s) => s.trim())
    } catch {
      return trimmed.split(',').map((s) => s.trim()).filter(Boolean)
    }
  }
  return trimmed
}

export function CustomFieldsPanel({ projects, onError }: Props) {
  const { t } = useI18n()
  const [fields, setFields] = useState<CustomFieldDefinitionDetail[]>([])
  const [lists, setLists] = useState<OptionList[]>([])
  const [form, setForm] = useState<FormState>(emptyForm)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [warning, setWarning] = useState<string | null>(null)

  const load = useCallback(async () => {
    try {
      const [defs, optionLists] = await Promise.all([
        api.listAdminCustomFields(),
        api.listAdminLists(),
      ])
      setFields(defs)
      setLists(optionLists)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }, [onError])

  useEffect(() => {
    void load()
  }, [load])

  const needsList =
    form.field_type === 'dropdown' || form.field_type === 'multi_select'

  const startEdit = (f: CustomFieldDefinitionDetail) => {
    setEditingId(f.id)
    setWarning(null)
    setForm({
      key: f.key,
      name: f.name,
      field_type: f.field_type,
      entity: f.entity,
      option_list_id: f.option_list_id ?? '',
      default_value:
        f.default_value == null
          ? ''
          : typeof f.default_value === 'string'
            ? f.default_value
            : JSON.stringify(f.default_value),
      required: f.required,
      help_text: f.help_text,
      sort_order: f.sort_order,
      show_in_list: f.show_in_list,
      show_in_filters: f.show_in_filters,
      active: f.active,
      scope: f.scope,
      project_ids: f.project_ids ?? [],
    })
  }

  const resetForm = () => {
    setEditingId(null)
    setForm(emptyForm())
    setWarning(null)
  }

  const buildBody = (acknowledge = false) => ({
    key: form.key.trim(),
    name: form.name.trim(),
    field_type: form.field_type,
    entity: form.entity,
    option_list_id: needsList && form.option_list_id ? form.option_list_id : null,
    default_value: parseDefault(form.field_type, form.default_value),
    required: form.required,
    help_text: form.help_text,
    sort_order: form.sort_order,
    show_in_list: form.show_in_list,
    show_in_filters: form.show_in_filters,
    active: form.active,
    scope: form.scope,
    project_ids: form.scope === 'selected' ? form.project_ids : [],
    ...(acknowledge ? { acknowledge_hide_values: true } : {}),
  })

  const onSubmit = async (e: FormEvent, acknowledge = false) => {
    e.preventDefault()
    if (form.scope === 'selected' && form.project_ids.length === 0) {
      onError(t('admin.customFields.projectsRequired'))
      return
    }
    if (
      editingId &&
      !acknowledge &&
      (form.scope === 'selected' || form.scope === 'all')
    ) {
      const existing = fields.find((f) => f.id === editingId)
      const narrowing =
        existing &&
        ((existing.scope === 'all' && form.scope === 'selected') ||
          (form.scope === 'selected' &&
            (existing.project_ids?.length ?? 0) > form.project_ids.length))
      if (narrowing) {
        const ok = window.confirm(t('admin.customFields.scopeWarn'))
        if (!ok) return
        acknowledge = true
      }
    }
    setBusy(true)
    setWarning(null)
    try {
      if (editingId) {
        const res = await api.updateAdminCustomField(
          editingId,
          buildBody(acknowledge),
        )
        if (res.warning) {
          setWarning(res.warning.warning)
        }
      } else {
        await api.createAdminCustomField(buildBody())
      }
      resetForm()
      await load()
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      if (msg.toLowerCase().includes('acknowledge') && editingId) {
        const ok = window.confirm(t('admin.customFields.scopeWarn'))
        if (ok) {
          try {
            await api.updateAdminCustomField(editingId, buildBody(true))
            resetForm()
            await load()
            return
          } catch (err2) {
            onError(err2 instanceof Error ? err2.message : String(err2))
            return
          }
        }
      }
      onError(msg)
    } finally {
      setBusy(false)
    }
  }

  const onDelete = async (f: CustomFieldDefinitionDetail) => {
    if (!window.confirm(t('admin.customFields.deleteConfirm'))) return
    setBusy(true)
    try {
      await api.deleteAdminCustomField(f.id)
      if (editingId === f.id) resetForm()
      await load()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const toggleProject = (id: string) => {
    setForm((prev) => ({
      ...prev,
      project_ids: prev.project_ids.includes(id)
        ? prev.project_ids.filter((p) => p !== id)
        : [...prev.project_ids, id],
    }))
  }

  return (
    <div className="panel admin-grid">
      <div>
        <h3>{t('admin.customFields')}</h3>
        <p className="muted">{t('admin.customFields.help')}</p>
        <table className="table">
          <thead>
            <tr>
              <th>{t('admin.customFields.key')}</th>
              <th>{t('admin.customFields.name')}</th>
              <th>{t('admin.customFields.type')}</th>
              <th>{t('admin.customFields.entity')}</th>
              <th>{t('admin.customFields.scope')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {fields.map((f) => (
              <tr key={f.id}>
                <td>
                  <span className="badge">{f.key}</span>
                </td>
                <td>
                  {f.name}
                  {!f.active ? (
                    <span className="muted"> ({t('admin.lists.inactive')})</span>
                  ) : null}
                </td>
                <td>{f.field_type}</td>
                <td>{f.entity}</td>
                <td>{f.scope}</td>
                <td className="row-actions">
                  <button
                    type="button"
                    className="btn secondary"
                    onClick={() => startEdit(f)}
                  >
                    {t('common.edit')}
                  </button>
                  <button
                    type="button"
                    className="btn secondary"
                    disabled={busy}
                    onClick={() => void onDelete(f)}
                  >
                    {t('common.delete')}
                  </button>
                </td>
              </tr>
            ))}
            {fields.length === 0 && (
              <tr>
                <td colSpan={6} className="muted">
                  {t('admin.customFields.empty')}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <form className="stack-form" onSubmit={(e) => void onSubmit(e)}>
        <h3>
          {editingId
            ? t('admin.customFields.edit')
            : t('admin.customFields.create')}
        </h3>
        {warning && <p className="error">{warning}</p>}
        <label className="field">
          <span>{t('admin.customFields.key')}</span>
          <input
            value={form.key}
            onChange={(e) => setForm({ ...form, key: e.target.value })}
            required
            pattern="[a-z0-9_\\-]+"
          />
        </label>
        <label className="field">
          <span>{t('admin.customFields.name')}</span>
          <input
            value={form.name}
            onChange={(e) => setForm({ ...form, name: e.target.value })}
            required
          />
        </label>
        <label className="field">
          <span>{t('admin.customFields.type')}</span>
          <select
            value={form.field_type}
            onChange={(e) => setForm({ ...form, field_type: e.target.value })}
          >
            {FIELD_TYPES.map((ft) => (
              <option key={ft} value={ft}>
                {ft}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>{t('admin.customFields.entity')}</span>
          <select
            value={form.entity}
            onChange={(e) => setForm({ ...form, entity: e.target.value })}
          >
            <option value="test_case">test_case</option>
            <option value="requirement">requirement</option>
            <option value="both">both</option>
          </select>
        </label>
        {needsList && (
          <label className="field">
            <span>{t('admin.customFields.optionList')}</span>
            <select
              value={form.option_list_id}
              onChange={(e) =>
                setForm({ ...form, option_list_id: e.target.value })
              }
              required
            >
              <option value="">—</option>
              {lists.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.name} ({l.key})
                </option>
              ))}
            </select>
          </label>
        )}
        <label className="field">
          <span>{t('admin.customFields.defaultValue')}</span>
          <input
            value={form.default_value}
            onChange={(e) =>
              setForm({ ...form, default_value: e.target.value })
            }
          />
        </label>
        <label className="field">
          <span>{t('admin.customFields.helpText')}</span>
          <textarea
            className="area"
            rows={2}
            value={form.help_text}
            onChange={(e) => setForm({ ...form, help_text: e.target.value })}
          />
        </label>
        <label className="field">
          <span>{t('admin.customFields.sortOrder')}</span>
          <input
            type="number"
            value={form.sort_order}
            onChange={(e) =>
              setForm({ ...form, sort_order: Number(e.target.value) || 0 })
            }
          />
        </label>
        <label className="field">
          <span>{t('admin.customFields.scope')}</span>
          <select
            value={form.scope}
            onChange={(e) => setForm({ ...form, scope: e.target.value })}
          >
            <option value="all">{t('admin.customFields.scopeAll')}</option>
            <option value="selected">
              {t('admin.customFields.scopeSelected')}
            </option>
          </select>
        </label>
        {form.scope === 'selected' && (
          <fieldset className="field">
            <legend>{t('admin.customFields.projects')}</legend>
            <div className="check-list">
              {projects.map((p) => (
                <label key={p.id} className="check">
                  <input
                    type="checkbox"
                    checked={form.project_ids.includes(p.id)}
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
            checked={form.required}
            onChange={(e) => setForm({ ...form, required: e.target.checked })}
          />
          {t('admin.customFields.required')}
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={form.show_in_list}
            onChange={(e) =>
              setForm({ ...form, show_in_list: e.target.checked })
            }
          />
          {t('admin.customFields.showInList')}
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={form.show_in_filters}
            onChange={(e) =>
              setForm({ ...form, show_in_filters: e.target.checked })
            }
          />
          {t('admin.customFields.showInFilters')}
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={form.active}
            onChange={(e) => setForm({ ...form, active: e.target.checked })}
          />
          {t('admin.lists.active')}
        </label>
        <div className="row-actions">
          <button
            className="btn"
            type="submit"
            disabled={busy}
            {...testIdProps(TestIds.adminCustomFieldsAddButton)}
          >
            {editingId ? t('common.save') : t('admin.customFields.create')}
          </button>
          {editingId && (
            <button
              type="button"
              className="btn secondary"
              onClick={resetForm}
            >
              {t('common.cancel')}
            </button>
          )}
        </div>
      </form>
    </div>
  )
}
