import { describe, expect, it } from 'vitest'
import {
  BUILTIN_COLUMNS,
  customFieldColumnId,
  defaultConfig,
  extendWithCustomFields,
  moveColumn,
  reorderColumns,
  sanitizeConfig,
  setColumnVisible,
  visibleColumns,
  visibleCustomFieldIds,
} from './caseListColumns'

describe('caseListColumns registry', () => {
  it('defaults show key/title/priority/status and hide extras', () => {
    const cfg = defaultConfig()
    const visible = new Set(
      cfg.columns.filter((c) => c.visible).map((c) => c.id),
    )
    expect(visible).toEqual(new Set(['key', 'title', 'priority', 'status']))
    expect(cfg.columns.some((c) => c.id === 'automated' && !c.visible)).toBe(
      true,
    )
  })

  it('never allows hiding title', () => {
    const registry = extendWithCustomFields([])
    const next = setColumnVisible(
      defaultConfig(),
      'title',
      false,
      registry,
    )
    expect(next.columns.find((c) => c.id === 'title')?.visible).toBe(true)
  })

  it('moves columns left/right and via reorder', () => {
    let cfg = defaultConfig()
    const first = cfg.columns[0].id
    const second = cfg.columns[1].id
    cfg = moveColumn(cfg, second, 'left')
    expect(cfg.columns[0].id).toBe(second)
    expect(cfg.columns[1].id).toBe(first)
    cfg = reorderColumns(cfg, first, second)
    expect(cfg.columns[0].id).toBe(first)
  })

  it('plugs custom fields into the registry extension point', () => {
    const fieldId = '11111111-1111-1111-1111-111111111111'
    const registry = extendWithCustomFields([
      { id: fieldId, name: 'Severity', field_type: 'select' },
    ])
    const cfId = customFieldColumnId(fieldId)
    expect(registry.some((c) => c.id === cfId && c.label === 'Severity')).toBe(
      true,
    )
    const cfg = sanitizeConfig(defaultConfig(), registry)
    expect(cfg.columns.some((c) => c.id === cfId && !c.visible)).toBe(true)
    const shown = setColumnVisible(cfg, cfId, true, registry)
    expect(visibleCustomFieldIds(shown)).toEqual([fieldId])
    expect(visibleColumns(shown, registry).some((c) => c.id === cfId)).toBe(
      true,
    )
  })

  it('exposes every builtin in BUILTIN_COLUMNS', () => {
    expect(BUILTIN_COLUMNS.map((c) => c.id)).toContain('last_run_status')
    expect(BUILTIN_COLUMNS.find((c) => c.id === 'title')?.hideable).toBe(false)
  })
})
