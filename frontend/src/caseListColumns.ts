/** Case list column registry + preference helpers (localStorage + API shape). */

export type BuiltinColumnId =
  | 'key'
  | 'title'
  | 'priority'
  | 'status'
  | 'automated'
  | 'last_run_status'
  | 'updated_at'
  | 'created_at'
  | 'folder'
  | 'tags'

export type ColumnId = BuiltinColumnId | `cf:${string}`

export type CaseListColumnPref = {
  id: string
  visible: boolean
}

export type CaseListColumnsConfig = {
  columns: CaseListColumnPref[]
  sort?: string | null
  order?: 'asc' | 'desc' | null
}

export type BuiltinColumnDef = {
  id: BuiltinColumnId
  labelKey: string
  sortable: boolean
  hideable: boolean
  defaultVisible: boolean
  /** CSS grid track size */
  width: string
}

/** Built-in columns. Custom fields plug in via `extendWithCustomFields`. */
export const BUILTIN_COLUMNS: BuiltinColumnDef[] = [
  {
    id: 'key',
    labelKey: 'cases.colKey',
    sortable: true,
    hideable: true,
    defaultVisible: true,
    width: '7rem',
  },
  {
    id: 'title',
    labelKey: 'cases.colTitle',
    sortable: true,
    hideable: false,
    defaultVisible: true,
    width: 'minmax(10rem, 1fr)',
  },
  {
    id: 'priority',
    labelKey: 'cases.colPriority',
    sortable: true,
    hideable: true,
    defaultVisible: true,
    width: '5.5rem',
  },
  {
    id: 'status',
    labelKey: 'cases.colStatus',
    sortable: true,
    hideable: true,
    defaultVisible: true,
    width: '6rem',
  },
  {
    id: 'automated',
    labelKey: 'cases.colAutomated',
    sortable: true,
    hideable: true,
    defaultVisible: false,
    width: '5.5rem',
  },
  {
    id: 'last_run_status',
    labelKey: 'cases.colLastRunStatus',
    sortable: true,
    hideable: true,
    defaultVisible: false,
    width: '7rem',
  },
  {
    id: 'updated_at',
    labelKey: 'cases.colUpdatedAt',
    sortable: true,
    hideable: true,
    defaultVisible: false,
    width: '7.5rem',
  },
  {
    id: 'created_at',
    labelKey: 'cases.colCreatedAt',
    sortable: true,
    hideable: true,
    defaultVisible: false,
    width: '7.5rem',
  },
  {
    id: 'folder',
    labelKey: 'cases.colFolder',
    sortable: true,
    hideable: true,
    defaultVisible: false,
    width: '8rem',
  },
  {
    id: 'tags',
    labelKey: 'cases.colTags',
    sortable: true,
    hideable: true,
    defaultVisible: false,
    width: '8rem',
  },
]

export type CustomFieldColumnSource = {
  id: string
  name: string
  field_type: string
}

export type ResolvedColumn = {
  id: string
  labelKey?: string
  label?: string
  sortable: boolean
  hideable: boolean
  defaultVisible: boolean
  width: string
  kind: 'builtin' | 'custom_field'
  fieldType?: string
}

export function defaultConfig(): CaseListColumnsConfig {
  return {
    columns: BUILTIN_COLUMNS.map((c) => ({
      id: c.id,
      visible: c.defaultVisible,
    })),
    sort: null,
    order: null,
  }
}

export function isCustomFieldColumnId(id: string): boolean {
  return /^cf:[0-9a-f-]{36}$/i.test(id)
}

export function customFieldColumnId(fieldId: string): string {
  return `cf:${fieldId}`
}

export function parseCustomFieldId(columnId: string): string | null {
  const m = /^cf:([0-9a-f-]{36})$/i.exec(columnId)
  return m ? m[1] : null
}

/** Merge project custom fields into the column registry (extension point). */
export function extendWithCustomFields(
  fields: CustomFieldColumnSource[],
): ResolvedColumn[] {
  const builtins: ResolvedColumn[] = BUILTIN_COLUMNS.map((c) => ({
    ...c,
    kind: 'builtin' as const,
  }))
  const custom: ResolvedColumn[] = fields.map((f) => ({
    id: customFieldColumnId(f.id),
    label: f.name,
    sortable: true,
    hideable: true,
    defaultVisible: false,
    width: '7rem',
    kind: 'custom_field' as const,
    fieldType: f.field_type,
  }))
  return [...builtins, ...custom]
}

export function sanitizeConfig(
  raw: CaseListColumnsConfig | null | undefined,
  registry: ResolvedColumn[],
): CaseListColumnsConfig {
  const base = defaultConfig()
  if (!raw?.columns?.length) {
    // Ensure custom fields appear (hidden) in the menu.
    const known = new Set(base.columns.map((c) => c.id))
    for (const col of registry) {
      if (!known.has(col.id)) {
        base.columns.push({ id: col.id, visible: col.defaultVisible })
      }
    }
    return base
  }

  const allowed = new Set(registry.map((c) => c.id))
  const seen = new Set<string>()
  const columns: CaseListColumnPref[] = []
  for (const c of raw.columns) {
    if (!allowed.has(c.id) || seen.has(c.id)) continue
    seen.add(c.id)
    const def = registry.find((r) => r.id === c.id)
    const visible = def && !def.hideable ? true : Boolean(c.visible)
    columns.push({ id: c.id, visible })
  }
  for (const col of registry) {
    if (!seen.has(col.id)) {
      columns.push({ id: col.id, visible: col.defaultVisible })
    }
  }
  // Title always visible.
  const title = columns.find((c) => c.id === 'title')
  if (title) title.visible = true
  else columns.unshift({ id: 'title', visible: true })

  let sort = raw.sort ?? null
  let order = raw.order ?? null
  if (sort && !allowed.has(sort) && sort !== 'default' && sort !== 'sort_order') {
    sort = null
    order = null
  }
  if (order && order !== 'asc' && order !== 'desc') order = 'asc'

  return { columns, sort, order }
}

export function visibleColumns(
  config: CaseListColumnsConfig,
  registry: ResolvedColumn[],
): ResolvedColumn[] {
  const byId = new Map(registry.map((c) => [c.id, c]))
  return config.columns
    .filter((c) => c.visible)
    .map((c) => byId.get(c.id))
    .filter((c): c is ResolvedColumn => Boolean(c))
}

export function moveColumn(
  config: CaseListColumnsConfig,
  id: string,
  direction: 'left' | 'right',
): CaseListColumnsConfig {
  const idx = config.columns.findIndex((c) => c.id === id)
  if (idx < 0) return config
  const swapWith = direction === 'left' ? idx - 1 : idx + 1
  if (swapWith < 0 || swapWith >= config.columns.length) return config
  const columns = [...config.columns]
  ;[columns[idx], columns[swapWith]] = [columns[swapWith], columns[idx]]
  return { ...config, columns }
}

export function reorderColumns(
  config: CaseListColumnsConfig,
  fromId: string,
  toId: string,
): CaseListColumnsConfig {
  if (fromId === toId) return config
  const from = config.columns.findIndex((c) => c.id === fromId)
  const to = config.columns.findIndex((c) => c.id === toId)
  if (from < 0 || to < 0) return config
  const columns = [...config.columns]
  const [item] = columns.splice(from, 1)
  columns.splice(to, 0, item)
  return { ...config, columns }
}

export function setColumnVisible(
  config: CaseListColumnsConfig,
  id: string,
  visible: boolean,
  registry: ResolvedColumn[],
): CaseListColumnsConfig {
  const def = registry.find((c) => c.id === id)
  if (def && !def.hideable) return config
  return {
    ...config,
    columns: config.columns.map((c) =>
      c.id === id ? { ...c, visible } : c,
    ),
  }
}

export function gridTemplateFor(
  visible: ResolvedColumn[],
  showCheckbox: boolean,
): string {
  const tracks = visible.map((c) => c.width)
  return [showCheckbox ? '2rem' : '', ...tracks].filter(Boolean).join(' ')
}

export function storageKey(projectId: string): string {
  return `vulyk.caseList.columns.${projectId}`
}

export function readLocalConfig(projectId: string): CaseListColumnsConfig | null {
  try {
    const raw = localStorage.getItem(storageKey(projectId))
    if (!raw) return null
    return JSON.parse(raw) as CaseListColumnsConfig
  } catch {
    return null
  }
}

export function writeLocalConfig(
  projectId: string,
  config: CaseListColumnsConfig,
): void {
  try {
    localStorage.setItem(storageKey(projectId), JSON.stringify(config))
  } catch {
    /* ignore quota */
  }
}

export function clearLocalConfig(projectId: string): void {
  try {
    localStorage.removeItem(storageKey(projectId))
  } catch {
    /* ignore */
  }
}

export function visibleCustomFieldIds(config: CaseListColumnsConfig): string[] {
  return config.columns
    .filter((c) => c.visible)
    .map((c) => parseCustomFieldId(c.id))
    .filter((id): id is string => Boolean(id))
}
