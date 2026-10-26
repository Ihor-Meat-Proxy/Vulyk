import {
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useState,
  useTransition,
} from 'react'
import { Link, useNavigate } from 'react-router-dom'
import {
  api,
  type CustomField,
  type Suite,
  type TestCase,
} from '../api'
import { useAuth } from '../auth'
import {
  clearLocalConfig,
  defaultConfig,
  extendWithCustomFields,
  gridTemplateFor,
  parseCustomFieldId,
  readLocalConfig,
  reorderColumns,
  sanitizeConfig,
  visibleColumns,
  visibleCustomFieldIds,
  writeLocalConfig,
  type CaseListColumnsConfig,
  type ResolvedColumn,
} from '../caseListColumns'
import { CaseFolderTree } from '../components/CaseFolderTree'
import { CaseListColumnSettings } from '../components/CaseListColumnSettings'
import { CaseStatusIcon } from '../components/CaseStatusIcon'
import { VirtualList } from '../components/VirtualList'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { useI18n } from '../i18n'
import { casePath } from '../lib/caseUrls'
import { TestIds, listItemProps, testIdProps } from '../testing'

type Props = {
  projectId: string | null
  suiteId: string | null
  suites: Suite[]
  onSelectSuite: (id: string | null) => void
  onSuitesChanged: () => void
}

const PAGE_SIZE = 200

const COL_HEADER_TEST_IDS: Record<string, string> = {
  key: TestIds.caseListColKey,
  title: TestIds.caseListColTitle,
  priority: TestIds.caseListColPriority,
  status: TestIds.caseListColStatus,
  automated: TestIds.caseListColAutomated,
  last_run_status: TestIds.caseListColLastRunStatus,
  updated_at: TestIds.caseListColUpdatedAt,
  created_at: TestIds.caseListColCreatedAt,
  folder: TestIds.caseListColFolder,
  tags: TestIds.caseListColTags,
}

function formatCfValue(value: unknown): string {
  if (value == null) return '—'
  if (typeof value === 'boolean') return value ? 'yes' : 'no'
  if (Array.isArray(value)) return value.map(String).join(', ')
  if (typeof value === 'object') return JSON.stringify(value)
  return String(value)
}

function colHeaderTestId(colId: string): string {
  return COL_HEADER_TEST_IDS[colId] ?? TestIds.caseListColCustomField
}

export function CaseList({
  projectId,
  suiteId,
  suites,
  onSelectSuite,
  onSuitesChanged,
}: Props) {
  const { t, formatNumber, formatDate, locale } = useI18n()
  const { user } = useAuth()
  const navigate = useNavigate()
  const { can } = useProjectPermissions(projectId)
  const canCreate = can('create_edit_cases')
  const canBulkEdit = can('create_edit_cases')
  const canManageFolders = can('manage_suites')
  const canDeleteCases = can('delete_cases')
  const canLlm = can('llm_assist')
  const canExport = can('export_reports')
  const showCheckbox = canBulkEdit || canLlm || canExport
  const [exporting, setExporting] = useState(false)

  const exportSelected = async () => {
    if (!projectId || selected.size === 0) return
    setExporting(true)
    try {
      await api.exportCases(projectId, {
        case_ids: [...selected],
        results: 5,
        locale,
      })
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setExporting(false)
    }
  }

  const exportOne = async (caseId: string) => {
    if (!projectId) return
    setExporting(true)
    try {
      await api.downloadCasePdf(projectId, caseId, { results: 5, locale })
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setExporting(false)
    }
  }

  const [cases, setCases] = useState<TestCase[]>([])
  const [total, setTotal] = useState(0)
  const [projectCaseTotal, setProjectCaseTotal] = useState(0)
  const [offset, setOffset] = useState(0)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [tag, setTag] = useState('')
  const [status, setStatus] = useState('')
  const [directOnly, setDirectOnly] = useState(false)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [pending, startTransition] = useTransition()
  const deferredQuery = useDeferredValue(query)
  const [newTitle, setNewTitle] = useState('')
  const [creating, setCreating] = useState(false)
  const [treeReloadToken, setTreeReloadToken] = useState(0)
  const [treeDrawerOpen, setTreeDrawerOpen] = useState(false)
  const [customFields, setCustomFields] = useState<CustomField[]>([])
  const [columnConfig, setColumnConfig] =
    useState<CaseListColumnsConfig>(defaultConfig)
  const [sort, setSort] = useState<string>('default')
  const [order, setOrder] = useState<'asc' | 'desc'>('asc')
  const [dragColId, setDragColId] = useState<string | null>(null)

  const registry = useMemo(
    () =>
      extendWithCustomFields(
        customFields.map((f) => ({
          id: f.id,
          name: f.name,
          field_type: f.field_type,
        })),
      ),
    [customFields],
  )

  const visibleCols = useMemo(
    () => visibleColumns(columnConfig, registry),
    [columnConfig, registry],
  )

  const gridTemplate = useMemo(() => {
    const base = gridTemplateFor(visibleCols, showCheckbox)
    return canExport ? `${base} 3.5rem` : base
  }, [visibleCols, showCheckbox, canExport])

  const cfIds = useMemo(
    () => visibleCustomFieldIds(columnConfig),
    [columnConfig],
  )

  const listParams = useCallback(
    (pageOffset: number) => ({
      suiteId,
      directOnly: suiteId != null && directOnly ? true : undefined,
      q: deferredQuery || undefined,
      tag: tag || undefined,
      status: status || undefined,
      sort: sort !== 'default' ? sort : undefined,
      order: sort !== 'default' ? order : undefined,
      cfIds: cfIds.length ? cfIds : undefined,
      limit: PAGE_SIZE,
      offset: pageOffset,
    }),
    [suiteId, directOnly, deferredQuery, tag, status, sort, order, cfIds],
  )

  const persistColumns = useCallback(
    async (next: CaseListColumnsConfig) => {
      if (!projectId) return
      const withSort: CaseListColumnsConfig = {
        ...next,
        sort:
          next.sort !== undefined
            ? next.sort
            : sort !== 'default'
              ? sort
              : null,
        order:
          next.order !== undefined
            ? next.order
            : sort !== 'default'
              ? order
              : null,
      }
      const sanitized = sanitizeConfig(withSort, registry)
      setColumnConfig(sanitized)
      writeLocalConfig(projectId, sanitized)
      if (user) {
        try {
          const saved = await api.putCaseListColumns(projectId, sanitized)
          const again = sanitizeConfig(saved, registry)
          setColumnConfig(again)
          writeLocalConfig(projectId, again)
        } catch {
          /* localStorage already written */
        }
      }
    },
    [projectId, registry, user, sort, order],
  )

  const resetColumns = useCallback(async () => {
    if (!projectId) return
    const defaults = sanitizeConfig(defaultConfig(), registry)
    setColumnConfig(defaults)
    setSort('default')
    setOrder('asc')
    clearLocalConfig(projectId)
    if (user) {
      try {
        const saved = await api.resetCaseListColumns(projectId)
        setColumnConfig(sanitizeConfig(saved, registry))
      } catch {
        /* ignore */
      }
    }
  }, [projectId, registry, user])

  useEffect(() => {
    if (!treeDrawerOpen) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setTreeDrawerOpen(false)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [treeDrawerOpen])


  // Restore list scroll after returning from case detail (Backspace / back link).
  useEffect(() => {
    const key = 'vulyk.caseListScroll'
    try {
      const raw = sessionStorage.getItem(key)
      if (raw) {
        const y = Number(raw)
        if (!Number.isNaN(y)) {
          requestAnimationFrame(() => window.scrollTo(0, y))
        }
        sessionStorage.removeItem(key)
      }
    } catch {
      /* ignore */
    }
    return () => {
      try {
        sessionStorage.setItem(key, String(window.scrollY))
      } catch {
        /* ignore */
      }
    }
  }, [])

  const selectSuite = (id: string | null) => {
    onSelectSuite(id)
    setTreeDrawerOpen(false)
  }

  const userId = user?.id ?? null

  // Load custom fields + column prefs when project (or signed-in user) changes.
  useEffect(() => {
    if (!projectId) {
      setCustomFields([])
      setColumnConfig(defaultConfig())
      return
    }
    let cancelled = false
    const local = readLocalConfig(projectId)
    api
      .listCustomFields(projectId)
      .then((fields) => {
        if (cancelled) return
        setCustomFields(fields)
        const reg = extendWithCustomFields(
          fields.map((f) => ({
            id: f.id,
            name: f.name,
            field_type: f.field_type,
          })),
        )
        const apply = (cfg: CaseListColumnsConfig) => {
          const sanitized = sanitizeConfig(cfg, reg)
          setColumnConfig(sanitized)
          if (sanitized.sort) setSort(sanitized.sort)
          if (sanitized.order === 'asc' || sanitized.order === 'desc') {
            setOrder(sanitized.order)
          }
        }
        if (userId) {
          api
            .getCaseListColumns(projectId)
            .then((remote) => {
              if (cancelled) return
              apply(remote.columns?.length ? remote : local ?? defaultConfig())
            })
            .catch(() => {
              if (!cancelled) apply(local ?? defaultConfig())
            })
        } else {
          apply(local ?? defaultConfig())
        }
      })
      .catch(() => {
        if (!cancelled) {
          setCustomFields([])
          setColumnConfig(
            sanitizeConfig(
              local ?? defaultConfig(),
              extendWithCustomFields([]),
            ),
          )
        }
      })
    return () => {
      cancelled = true
    }
  }, [projectId, userId])

  useEffect(() => {
    setOffset(0)
    setSelected(new Set())
  }, [projectId, suiteId, deferredQuery, tag, status, directOnly, sort, order, cfIds.join(',')])

  useEffect(() => {
    if (suiteId == null) setDirectOnly(false)
  }, [suiteId])

  useEffect(() => {
    if (!projectId) {
      setCases([])
      setTotal(0)
      setProjectCaseTotal(0)
      return
    }
    let cancelled = false
    setLoading(true)
    api
      .listCases(projectId, listParams(offset))
      .then((page) => {
        if (cancelled) return
        startTransition(() => {
          setCases((prev) =>
            offset === 0 ? page.items : [...prev, ...page.items],
          )
          setTotal(page.total)
          setError(null)
        })
      })
      .catch((err: Error) => {
        if (!cancelled) setError(err.message)
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [projectId, listParams, offset])

  useEffect(() => {
    if (!projectId) return
    let cancelled = false
    api
      .listCases(projectId, { limit: 1, offset: 0 })
      .then((page) => {
        if (!cancelled) setProjectCaseTotal(page.total)
      })
      .catch(() => {
        /* tree still works without the badge */
      })
    return () => {
      cancelled = true
    }
  }, [projectId, suites, treeReloadToken])

  const toggle = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const bumpTree = () => setTreeReloadToken((n) => n + 1)

  const reloadList = async () => {
    if (!projectId) return
    const page = await api.listCases(projectId, listParams(0))
    setOffset(0)
    setCases(page.items)
    setTotal(page.total)
    const all = await api.listCases(projectId, { limit: 1, offset: 0 })
    setProjectCaseTotal(all.total)
    bumpTree()
  }

  const toggleSort = (colId: string) => {
    const def = registry.find((c) => c.id === colId)
    if (!def?.sortable) return
    const nextSort = colId
    const nextOrder =
      sort === colId ? (order === 'asc' ? 'desc' : 'asc') : 'asc'
    setSort(nextSort)
    setOrder(nextOrder)
    void persistColumns({
      ...columnConfig,
      sort: nextSort,
      order: nextOrder,
    })
  }

  const createCase = async () => {
    if (!projectId || !newTitle.trim()) return
    setCreating(true)
    try {
      await api.createCase(projectId, {
        title: newTitle.trim(),
        suite_id: suiteId,
        status: 'ready',
        priority: 'medium',
        steps: [
          {
            action: 'Execute the scenario',
            expected_result: 'Expected outcome observed',
          },
        ],
      })
      setNewTitle('')
      await reloadList()
      onSuitesChanged()
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setCreating(false)
    }
  }

  const bulkStatus = async (nextStatus: string) => {
    if (!projectId || selected.size === 0) return
    await api.bulkCases(projectId, {
      action: 'set_status',
      case_ids: [...selected],
      status: nextStatus,
    })
    setSelected(new Set())
    await reloadList()
  }

  const onCreateFolder = async (parentId: string | null) => {
    if (!projectId || !canManageFolders) return
    const name = window.prompt(t('folders.promptName'))
    if (!name?.trim()) return
    try {
      await api.createSuite(projectId, {
        name: name.trim(),
        parent_id: parentId,
        sort_order: suites.filter((s) => s.parent_id === parentId).length,
      })
      onSuitesChanged()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  const onRenameFolder = async (suite: Suite) => {
    if (!projectId || !canManageFolders) return
    const name = window.prompt(t('folders.promptRename'), suite.name)
    if (!name?.trim() || name.trim() === suite.name) return
    try {
      await api.updateSuite(projectId, suite.id, { name: name.trim() })
      onSuitesChanged()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  const onDeleteFolder = async (suite: Suite) => {
    if (!projectId || !canManageFolders) return
    const hasContent =
      (suite.case_count ?? 0) > 0 ||
      (suite.direct_case_count ?? 0) > 0 ||
      suites.some((s) => s.parent_id === suite.id)
    let onNonempty: 'move_to_parent' | 'delete_cases' = 'move_to_parent'
    if (hasContent) {
      const choice = window.prompt(
        t('folders.deleteConfirm'),
        'move_to_parent',
      )
      if (choice == null) return
      if (choice.trim() === 'delete_cases') {
        if (!canDeleteCases) {
          setError(t('folders.deleteCasesForbidden'))
          return
        }
        onNonempty = 'delete_cases'
      } else {
        onNonempty = 'move_to_parent'
      }
    } else if (!window.confirm(t('folders.deleteEmpty'))) {
      return
    }
    try {
      await api.deleteSuite(projectId, suite.id, onNonempty)
      if (suiteId === suite.id) selectSuite(null)
      onSuitesChanged()
      await reloadList()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  const onMoveCase = async (caseId: string, nextSuiteId: string | null) => {
    if (!projectId || !canBulkEdit) return
    try {
      await api.bulkCases(projectId, {
        action: 'move_suite',
        case_ids: [caseId],
        suite_id: nextSuiteId,
        clear_suite: nextSuiteId == null,
      })
      onSuitesChanged()
      await reloadList()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  const onMoveFolder = async (folderId: string, parentId: string | null) => {
    if (!projectId || !canManageFolders) return
    try {
      await api.updateSuite(projectId, folderId, {
        parent_id: parentId ?? undefined,
        clear_parent: parentId == null,
      })
      onSuitesChanged()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  const columnLabel = (col: ResolvedColumn) =>
    col.label ?? (col.labelKey ? t(col.labelKey) : col.id)

  const renderCell = (col: ResolvedColumn, c: TestCase) => {
    switch (col.id) {
      case 'key':
        return (
          <Link to={casePath(c.key)} {...testIdProps(TestIds.caseRowKey)}>
            <span className="badge">{c.key}</span>
          </Link>
        )
      case 'title':
        return (
          <Link to={casePath(c.key)} {...testIdProps(TestIds.caseRowTitle)}>
            {c.title}
          </Link>
        )
      case 'priority':
        return (
          <span
            className={`priority-label priority-${c.priority}`}
            {...testIdProps(TestIds.caseRowPriority)}
          >
            <span
              className={`priority-bar inline priority-${c.priority}`}
              aria-hidden
            />
            {c.priority}
          </span>
        )
      case 'status':
        return (
          <span
            className={`status-pill status-${c.status}`}
            {...testIdProps(TestIds.caseRowStatus)}
          >
            <CaseStatusIcon status={c.status} />
            {c.status}
          </span>
        )
      case 'automated':
        return (
          <span {...testIdProps(TestIds.caseRowAutomated)}>
            {c.automated
              ? t('cases.automatedYes')
              : t('cases.automatedNo')}
          </span>
        )
      case 'last_run_status':
        return (
          <span {...testIdProps(TestIds.caseRowLastRunStatus)}>
            {c.last_run_status
              ? c.last_run_status
              : t('cases.lastRunNone')}
          </span>
        )
      case 'updated_at':
        return (
          <span {...testIdProps(TestIds.caseRowUpdatedAt)}>
            {formatDate(c.updated_at)}
          </span>
        )
      case 'created_at':
        return (
          <span {...testIdProps(TestIds.caseRowCreatedAt)}>
            {formatDate(c.created_at)}
          </span>
        )
      case 'folder':
        return (
          <span {...testIdProps(TestIds.caseRowFolder)}>
            {c.folder_name || t('cases.unfiled')}
          </span>
        )
      case 'tags':
        return (
          <span
            className="case-tags-cell"
            {...testIdProps(TestIds.caseRowTags)}
          >
            {(c.tag_names ?? []).join(', ') || '—'}
          </span>
        )
      default: {
        const fieldId = parseCustomFieldId(col.id)
        if (!fieldId) {
          return <span {...testIdProps(TestIds.caseRowCustomField)}>—</span>
        }
        const cf = (c.custom_fields ?? []).find((f) => f.field_id === fieldId)
        return (
          <span
            {...listItemProps(TestIds.caseRowCustomField, {
              entityId: fieldId,
              entityKey: col.id,
              index: 0,
            })}
          >
            {formatCfValue(cf?.value)}
          </span>
        )
      }
    }
  }

  if (!projectId) {
    return (
      <div className="empty">
        Create a project via the API to get started, then refresh.
      </div>
    )
  }

  return (
    <div
      className={`case-browser case-browser-split${treeDrawerOpen ? ' tree-drawer-open' : ''}`}
    >
      {treeDrawerOpen && (
        <button
          type="button"
          className="case-tree-backdrop"
          aria-label={t('nav.close')}
          onClick={() => setTreeDrawerOpen(false)}
          {...testIdProps(TestIds.caseListFoldersBackdrop)}
        />
      )}
      <aside
        className="case-tree-panel"
        id="case-folder-drawer"
        aria-hidden={false}
        {...testIdProps(TestIds.caseListFoldersDrawer)}
      >
        <div className="case-tree-drawer-header">
          <span className="folder-tree-title">{t('folders.title')}</span>
          <button
            type="button"
            className="btn secondary case-tree-drawer-close"
            onClick={() => setTreeDrawerOpen(false)}
            {...testIdProps(TestIds.caseListFoldersDrawerClose)}
          >
            {t('nav.close')}
          </button>
        </div>
        <CaseFolderTree
          projectId={projectId}
          suites={suites}
          projectCaseTotal={projectCaseTotal}
          selectedSuiteId={suiteId}
          canManage={canManageFolders}
          canEditCases={canBulkEdit}
          onSelectSuite={selectSuite}
          onCreateFolder={(id) => void onCreateFolder(id)}
          onRenameFolder={(s) => void onRenameFolder(s)}
          onDeleteFolder={(s) => void onDeleteFolder(s)}
          onMoveCase={(id, sid) => void onMoveCase(id, sid)}
          onMoveFolder={(id, pid) => void onMoveFolder(id, pid)}
          reloadToken={treeReloadToken}
        />
      </aside>

      <div className="case-list-panel" {...testIdProps(TestIds.caseListPanel)}>
        <div className="toolbar folders-drawer-bar">
          <button
            type="button"
            className="btn secondary folders-drawer-toggle"
            aria-expanded={treeDrawerOpen}
            aria-controls="case-folder-drawer"
            onClick={() => setTreeDrawerOpen(true)}
            {...testIdProps(TestIds.caseListFoldersToggle)}
          >
            {t('folders.title')}
          </button>
        </div>
        {canCreate && (
          <div className="toolbar create-case-bar">
            <input
              className="search-input"
              placeholder={t('cases.newTitle')}
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              aria-label={t('cases.newTitle')}
              onKeyDown={(e) => {
                if (e.key === 'Enter') void createCase()
              }}
              {...testIdProps(TestIds.caseListNewTitleInput)}
            />
            <button
              type="button"
              className="btn"
              disabled={creating || !newTitle.trim()}
              onClick={() => void createCase()}
              {...testIdProps(TestIds.caseListNewCaseButton)}
            >
              {creating ? t('cases.creating') : t('cases.create')}
            </button>
          </div>
        )}
        <div className="toolbar">
          <input
            className="search-input"
            placeholder={t('cases.search')}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            aria-label={t('cases.search')}
            {...testIdProps(TestIds.caseListSearchInput)}
          />
          <input
            className="filter-input"
            placeholder={t('cases.tag')}
            value={tag}
            onChange={(e) => setTag(e.target.value)}
            {...testIdProps(TestIds.caseListTagInput)}
          />
          <select
            className="filter-input"
            value={status}
            onChange={(e) => setStatus(e.target.value)}
            aria-label={t('cases.statusFilter')}
            {...testIdProps(TestIds.caseListStatusFilter)}
          >
            <option value="">{t('cases.allStatuses')}</option>
            <option value="draft">draft</option>
            <option value="ready">ready</option>
            <option value="deprecated">deprecated</option>
          </select>
          {suiteId != null && (
            <label className="direct-only-toggle">
              <input
                type="checkbox"
                checked={directOnly}
                onChange={(e) => setDirectOnly(e.target.checked)}
                {...testIdProps(TestIds.caseListDirectOnlyCheckbox)}
              />
              {t('folders.directOnly')}
            </label>
          )}
          <CaseListColumnSettings
            config={columnConfig}
            registry={registry}
            onChange={(next) => void persistColumns(next)}
            onReset={() => void resetColumns()}
          />
          <span className="muted">
            {formatNumber(total)} {t('cases.count')}
            {pending || loading ? ` · ${t('common.loading')}` : ''}
          </span>
        </div>

        {(canBulkEdit || canLlm || canExport) && selected.size > 0 && (
          <div className="bulk-bar" {...testIdProps(TestIds.caseListBulkBar)}>
            <span>
              {selected.size} {t('cases.selected')}
            </span>
            {canBulkEdit && (
              <>
                <button
                  type="button"
                  className="btn secondary"
                  onClick={() => void bulkStatus('ready')}
                  {...testIdProps(TestIds.caseListMarkReadyButton)}
                >
                  {t('cases.markReady')}
                </button>
                <button
                  type="button"
                  className="btn secondary"
                  onClick={() => void bulkStatus('deprecated')}
                  {...testIdProps(TestIds.caseListDeprecateButton)}
                >
                  {t('cases.deprecate')}
                </button>
              </>
            )}
            {canLlm && (
              <button
                type="button"
                className="btn secondary"
                onClick={() =>
                  navigate(`/generate?cases=${[...selected].join(',')}`)
                }
                {...testIdProps(TestIds.caseListGenerateButton)}
              >
                {t('cases.generateAutomation')}
              </button>
            )}
            {canExport && (
              <button
                type="button"
                className="btn secondary"
                data-testid="case-list-export-pdf"
                disabled={exporting}
                onClick={() => void exportSelected()}
              >
                {exporting ? t('cases.exporting') : t('cases.exportPdfBulk')}
              </button>
            )}
            <button
              type="button"
              className="btn secondary"
              onClick={() => setSelected(new Set())}
              {...testIdProps(TestIds.caseListClearSelectionButton)}
            >
              {t('cases.clearSelection')}
            </button>
          </div>
        )}

        {error && (
          <p className="error" {...testIdProps(TestIds.caseListError)}>
            {error}
          </p>
        )}

        {loading && cases.length === 0 ? (
          <div className="empty" {...testIdProps(TestIds.caseListLoading)}>
            {t('common.loading')}
          </div>
        ) : cases.length === 0 ? (
          <div className="empty" {...testIdProps(TestIds.caseListEmpty)}>
            {t('cases.empty')}
          </div>
        ) : (
          <>
            <div
              className="table-head case-list-head"
              style={{ gridTemplateColumns: gridTemplate }}
              {...testIdProps(TestIds.caseListHead)}
            >
              {showCheckbox ? <span /> : null}
              {visibleCols.map((col, colIndex) => {
                const label = columnLabel(col)
                const sorted = sort === col.id
                const headerId = colHeaderTestId(col.id)
                const headerProps =
                  headerId === TestIds.caseListColCustomField
                    ? listItemProps(TestIds.caseListColCustomField, {
                        entityId: parseCustomFieldId(col.id) ?? col.id,
                        entityKey: col.id,
                        index: colIndex,
                      })
                    : testIdProps(headerId)
                return (
                  <div
                    key={col.id}
                    className={`case-col-head${sorted ? ' is-sorted' : ''}`}
                    draggable
                    onDragStart={() => setDragColId(col.id)}
                    onDragOver={(e) => e.preventDefault()}
                    onDrop={() => {
                      if (dragColId) {
                        void persistColumns(
                          reorderColumns(columnConfig, dragColId, col.id),
                        )
                      }
                      setDragColId(null)
                    }}
                    onDragEnd={() => setDragColId(null)}
                    {...headerProps}
                  >
                    {col.sortable ? (
                      <button
                        type="button"
                        className="case-col-sort"
                        onClick={() => toggleSort(col.id)}
                        aria-label={t('cases.sortBy', { column: label })}
                        {...listItemProps(TestIds.caseListColSort, {
                          entityId: parseCustomFieldId(col.id) ?? col.id,
                          entityKey: col.id,
                          index: colIndex,
                        })}
                      >
                        {label}
                        {sorted ? (order === 'asc' ? ' ↑' : ' ↓') : ''}
                      </button>
                    ) : (
                      <span>{label}</span>
                    )}
                  </div>
                )
              })}
              {canExport ? <span /> : null}
            </div>
            <div {...testIdProps(TestIds.caseListTable)}>
              <VirtualList
                items={cases}
                rowHeight={44}
                height={520}
                className="case-virtual"
                renderRow={(c, index) => (
                  <div
                    className="table-row case-list-row"
                    style={{ gridTemplateColumns: gridTemplate }}
                    {...listItemProps(TestIds.caseRow, {
                      entityId: c.id,
                      entityKey: c.key,
                      index,
                    })}
                  >
                    {showCheckbox ? (
                      <input
                        type="checkbox"
                        checked={selected.has(c.id)}
                        onChange={() => toggle(c.id)}
                        aria-label={`Select ${c.key}`}
                        {...testIdProps(TestIds.caseRowCheckbox)}
                      />
                    ) : null}
                    {visibleCols.map((col) => (
                      <div
                        key={col.id}
                        className={`case-col case-col-${col.id}`}
                      >
                        {renderCell(col, c)}
                      </div>
                    ))}
                    {canExport ? (
                      <button
                        type="button"
                        className="btn secondary btn-compact"
                        data-testid={`case-list-row-export-${c.key}`}
                        disabled={exporting}
                        aria-label={`${t('cases.exportPdf')} ${c.key}`}
                        onClick={() => void exportOne(c.id)}
                      >
                        PDF
                      </button>
                    ) : null}
                  </div>
                )}
              />
            </div>
            {cases.length < total && (
              <div className="pager">
                <button
                  type="button"
                  className="btn secondary"
                  disabled={loading}
                  onClick={() => setOffset(cases.length)}
                  {...testIdProps(TestIds.caseListLoadMoreButton)}
                >
                  {t('cases.loadMore')} ({formatNumber(cases.length)} /{' '}
                  {formatNumber(total)})
                </button>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  )
}
