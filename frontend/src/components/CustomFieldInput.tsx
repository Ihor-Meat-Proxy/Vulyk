import type { CustomField, CustomFieldOption } from '../api'
import { TestIds, testIdProps } from '../testing'

function asOptions(raw: CustomField['options']): CustomFieldOption[] {
  if (!Array.isArray(raw)) return []
  return raw.map((o) => {
    if (typeof o === 'string') return { label: o, value: o }
    const obj = o as CustomFieldOption
    return {
      id: obj.id,
      key: obj.key,
      label: obj.label ?? String(obj.value ?? ''),
      value: obj.value ?? obj.label ?? '',
      colour: obj.colour,
    }
  })
}

function displayValue(value: unknown): string {
  if (value === null || value === undefined) return '—'
  if (typeof value === 'boolean') return value ? 'yes' : 'no'
  if (Array.isArray(value)) return value.map(String).join(', ')
  if (typeof value === 'object') return JSON.stringify(value)
  return String(value)
}

type Props = {
  field: CustomField
  value: unknown
  onChange: (next: unknown) => void
  disabled?: boolean
  readOnly?: boolean
  testId?: string
}

export function CustomFieldInput({
  field,
  value,
  onChange,
  disabled,
  readOnly,
  testId = TestIds.caseDetailCustomFieldInput,
}: Props) {
  const options = asOptions(field.options)
  const help = field.help_text?.trim()

  if (readOnly) {
    return (
      <label className="cf-field">
        <span className="muted">
          {field.name}
          {field.required ? ' *' : ''}
        </span>
        <span>{displayValue(value)}</span>
        {help ? <span className="muted cf-help">{help}</span> : null}
      </label>
    )
  }

  const ft = field.field_type

  return (
    <label className="cf-field">
      <span className="muted">
        {field.name}
        {field.required ? ' *' : ''}
      </span>
      {ft === 'checkbox' ? (
        <input
          type="checkbox"
          checked={Boolean(value)}
          disabled={disabled}
          onChange={(e) => onChange(e.target.checked)}
          {...testIdProps(testId)}
        />
      ) : ft === 'long_text' ? (
        <textarea
          className="area"
          rows={3}
          value={value == null ? '' : String(value)}
          disabled={disabled}
          onChange={(e) => onChange(e.target.value)}
          {...testIdProps(testId)}
        />
      ) : ft === 'number' ? (
        <input
          type="number"
          value={value == null || value === '' ? '' : String(value)}
          disabled={disabled}
          onChange={(e) => {
            const raw = e.target.value
            if (raw === '') onChange(null)
            else onChange(Number(raw))
          }}
          {...testIdProps(testId)}
        />
      ) : ft === 'date' ? (
        <input
          type="date"
          value={value == null ? '' : String(value)}
          disabled={disabled}
          onChange={(e) => onChange(e.target.value || null)}
          {...testIdProps(testId)}
        />
      ) : ft === 'url' ? (
        <input
          type="url"
          placeholder="https://"
          value={value == null ? '' : String(value)}
          disabled={disabled}
          onChange={(e) => onChange(e.target.value)}
          {...testIdProps(testId)}
        />
      ) : ft === 'dropdown' ? (
        <select
          value={value == null ? '' : String(value)}
          disabled={disabled}
          onChange={(e) => onChange(e.target.value || null)}
          {...testIdProps(testId)}
        >
          <option value="">—</option>
          {options.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      ) : ft === 'multi_select' ? (
        <select
          multiple
          value={Array.isArray(value) ? value.map(String) : []}
          disabled={disabled}
          onChange={(e) => {
            const selected = Array.from(e.target.selectedOptions).map(
              (o) => o.value,
            )
            onChange(selected)
          }}
          size={Math.min(6, Math.max(3, options.length || 3))}
          {...testIdProps(testId)}
        >
          {options.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      ) : (
        <input
          type="text"
          value={value == null ? '' : String(value)}
          disabled={disabled}
          onChange={(e) => onChange(e.target.value)}
          {...testIdProps(testId)}
        />
      )}
      {help ? <span className="muted cf-help">{help}</span> : null}
    </label>
  )
}

export function formatCustomFieldValue(value: unknown): string {
  return displayValue(value)
}
