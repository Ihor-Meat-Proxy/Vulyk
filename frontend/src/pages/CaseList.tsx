import { useDeferredValue, useEffect, useState, useTransition } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { api, type Suite, type TestCase } from '../api'
import { CaseFolderTree } from '../components/CaseFolderTree'
import { CaseStatusIcon } from '../components/CaseStatusIcon'
import { VirtualList } from '../components/VirtualList'
import { useI18n } from '../i18n'
import { useProjectPermissions } from '../hooks/useProjectPermissions'

type Props = {
  projectId: string | null
  suiteId: string | null
  suites: Suite[]
  onSelectSuite: (id: string | null) => void
  onSuitesChanged: () => void
}

const PAGE_SIZE = 200

export function CaseList({
  projectId,
  suiteId,
  suites,
  onSelectSuite,
  onSuitesChanged,
}: Props) {
  const { t, formatNumber } = useI18n()
  const navigate = useNavigate()
  const { can } = useProjectPermissions(projectId)
  const canCreate = can('create_edit_cases')
  const canBulkEdit = can('create_edit_cases')
  const canManageFolders = can('manage_suites')
  const canDeleteCases = can('delete_cases')
  const canLlm = can('llm_assist')

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

  useEffect(() => {
    if (!treeDrawerOpen) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setTreeDrawerOpen(false)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [treeDrawerOpen])

  const selectSuite = (id: string | null) => {
    onSelectSuite(id)
    setTreeDrawerOpen(false)
  }

  useEffect(() => {
    setOffset(0)
    setSelected(new Set())
  }, [projectId, suiteId, deferredQuery, tag, status, directOnly])

  useEffect(() => {
    // Direct-only only applies when a folder is selected.
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
      .listCases(projectId, {
        suiteId,
        directOnly: suiteId != null && directOnly ? true : undefined,
        q: deferredQuery || undefined,
        tag: tag || undefined,
        status: status || undefined,
        limit: PAGE_SIZE,
        offset,
      })
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
  }, [projectId, suiteId, deferredQuery, tag, status, offset, directOnly])

  // Unfiltered project total for the tree "All cases" badge (never from a capped tree fetch).
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
    const page = await api.listCases(projectId, {
      suiteId,
      directOnly: suiteId != null && directOnly ? true : undefined,
      q: deferredQuery || undefined,
      tag: tag || undefined,
      status: status || undefined,
      limit: PAGE_SIZE,
      offset: 0,
    })
    setOffset(0)
    setCases(page.items)
    setTotal(page.total)
    const all = await api.listCases(projectId, { limit: 1, offset: 0 })
    setProjectCaseTotal(all.total)
    bumpTree()
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
        />
      )}
      <aside
        className="case-tree-panel"
        id="case-folder-drawer"
        aria-hidden={false}
      >
        <div className="case-tree-drawer-header">
          <span className="folder-tree-title">{t('folders.title')}</span>
          <button
            type="button"
            className="btn secondary case-tree-drawer-close"
            onClick={() => setTreeDrawerOpen(false)}
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

      <div className="case-list-panel">
        <div className="toolbar folders-drawer-bar">
          <button
            type="button"
            className="btn secondary folders-drawer-toggle"
            aria-expanded={treeDrawerOpen}
            aria-controls="case-folder-drawer"
            onClick={() => setTreeDrawerOpen(true)}
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
            />
            <button
              type="button"
              className="btn"
              disabled={creating || !newTitle.trim()}
              onClick={() => void createCase()}
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
          />
          <input
            className="filter-input"
            placeholder={t('cases.tag')}
            value={tag}
            onChange={(e) => setTag(e.target.value)}
          />
          <select
            className="filter-input"
            value={status}
            onChange={(e) => setStatus(e.target.value)}
            aria-label={t('cases.statusFilter')}
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
              />
              {t('folders.directOnly')}
            </label>
          )}
          <span className="muted">
            {formatNumber(total)} {t('cases.count')}
            {pending || loading ? ` · ${t('common.loading')}` : ''}
          </span>
        </div>

        {(canBulkEdit || canLlm) && selected.size > 0 && (
          <div className="bulk-bar">
            <span>
              {selected.size} {t('cases.selected')}
            </span>
            {canBulkEdit && (
              <>
                <button
                  type="button"
                  className="btn secondary"
                  onClick={() => void bulkStatus('ready')}
                >
                  {t('cases.markReady')}
                </button>
                <button
                  type="button"
                  className="btn secondary"
                  onClick={() => void bulkStatus('deprecated')}
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
              >
                {t('cases.generateAutomation')}
              </button>
            )}
            <button
              type="button"
              className="btn secondary"
              onClick={() => setSelected(new Set())}
            >
              {t('cases.clearSelection')}
            </button>
          </div>
        )}

        {error && <p className="error">{error}</p>}

        {cases.length === 0 && !loading ? (
          <div className="empty">{t('cases.empty')}</div>
        ) : (
          <>
            <div className="table-head">
              <span />
              <span>{t('cases.colKey')}</span>
              <span>{t('cases.colTitle')}</span>
              <span>{t('cases.colPriority')}</span>
              <span>{t('cases.colStatus')}</span>
            </div>
            <VirtualList
              items={cases}
              rowHeight={44}
              height={520}
              className="case-virtual"
              renderRow={(c) => (
                <div className="table-row">
                  {canBulkEdit || canLlm ? (
                    <input
                      type="checkbox"
                      checked={selected.has(c.id)}
                      onChange={() => toggle(c.id)}
                      aria-label={`Select ${c.key}`}
                    />
                  ) : (
                    <span />
                  )}
                  <Link to={`/cases/${c.id}`}>
                    <span className="badge">{c.key}</span>
                  </Link>
                  <Link to={`/cases/${c.id}`}>{c.title}</Link>
                  <span className={`priority-label priority-${c.priority}`}>
                    <span
                      className={`priority-bar inline priority-${c.priority}`}
                      aria-hidden
                    />
                    {c.priority}
                  </span>
                  <span className={`status-pill status-${c.status}`}>
                    <CaseStatusIcon status={c.status} />
                    {c.status}
                  </span>
                </div>
              )}
            />
            {cases.length < total && (
              <div className="pager">
                <button
                  type="button"
                  className="btn secondary"
                  disabled={loading}
                  onClick={() => setOffset(cases.length)}
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
