import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { api, type Suite, type TestCase } from '../api'
import { useI18n } from '../i18n'
import { TestIds, listChildProps, listItemProps, testIdProps } from '../testing'
import { CaseStatusIcon } from './CaseStatusIcon'

const TREE_PAGE = 100
const UNFILED_KEY = '__unfiled__'

type Props = {
  projectId: string
  suites: Suite[]
  /** Unfiltered project case total (All cases). */
  projectCaseTotal: number
  selectedSuiteId: string | null
  selectedCaseId?: string | null
  canManage: boolean
  canEditCases: boolean
  onSelectSuite: (id: string | null) => void
  onCreateFolder: (parentId: string | null) => void
  onRenameFolder: (suite: Suite) => void
  onDeleteFolder: (suite: Suite) => void
  onMoveCase: (caseId: string, suiteId: string | null) => void
  onMoveFolder: (suiteId: string, parentId: string | null) => void
  /** Bump to invalidate cached folder case pages after mutations. */
  reloadToken?: number
}

type FolderNode = Suite & { children: FolderNode[] }

type FolderCasesState = {
  items: TestCase[]
  total: number
  loading: boolean
}

function buildTree(suites: Suite[]): FolderNode[] {
  const map = new Map<string, FolderNode>()
  for (const s of suites) {
    map.set(s.id, { ...s, children: [] })
  }
  const roots: FolderNode[] = []
  for (const s of suites) {
    const node = map.get(s.id)!
    if (s.parent_id && map.has(s.parent_id)) {
      map.get(s.parent_id)!.children.push(node)
    } else {
      roots.push(node)
    }
  }
  const sortRec = (nodes: FolderNode[]) => {
    nodes.sort(
      (a, b) => a.sort_order - b.sort_order || a.name.localeCompare(b.name),
    )
    nodes.forEach((n) => sortRec(n.children))
  }
  sortRec(roots)
  return roots
}

function countTooltip(recursive: number, direct: number): string {
  return `${recursive} (${direct} direct)`
}

export function CaseFolderTree({
  projectId,
  suites,
  projectCaseTotal,
  selectedSuiteId,
  selectedCaseId,
  canManage,
  canEditCases,
  onSelectSuite,
  onCreateFolder,
  onRenameFolder,
  onDeleteFolder,
  onMoveCase,
  onMoveFolder,
  reloadToken = 0,
}: Props) {
  const { t } = useI18n()
  const tree = useMemo(() => buildTree(suites), [suites])
  // Start collapsed — expand loads direct cases for that folder only.
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set())
  const [folderCases, setFolderCases] = useState<
    Record<string, FolderCasesState>
  >({})
  const folderCasesRef = useRef(folderCases)
  folderCasesRef.current = folderCases
  const [dragCaseId, setDragCaseId] = useState<string | null>(null)
  const [dragFolderId, setDragFolderId] = useState<string | null>(null)
  const [mobileMoveCaseId, setMobileMoveCaseId] = useState<string | null>(null)

  useEffect(() => {
    setFolderCases({})
    setExpanded(new Set())
  }, [projectId, reloadToken])

  const loadFolderCases = useCallback(
    async (suiteId: string | null, append: boolean) => {
      const key = suiteId ?? UNFILED_KEY
      const offset = append
        ? (folderCasesRef.current[key]?.items.length ?? 0)
        : 0
      setFolderCases((prev) => ({
        ...prev,
        [key]: {
          items: append ? (prev[key]?.items ?? []) : [],
          total: append ? (prev[key]?.total ?? 0) : 0,
          loading: true,
        },
      }))
      try {
        const page = await api.listCases(projectId, {
          suiteId: suiteId ?? undefined,
          directOnly: suiteId != null,
          unfiledOnly: suiteId == null,
          limit: TREE_PAGE,
          offset,
        })
        setFolderCases((prev) => {
          const prevItems = append ? (prev[key]?.items ?? []) : []
          return {
            ...prev,
            [key]: {
              items: [...prevItems, ...page.items],
              total: page.total,
              loading: false,
            },
          }
        })
      } catch {
        setFolderCases((prev) => ({
          ...prev,
          [key]: {
            items: prev[key]?.items ?? [],
            total: prev[key]?.total ?? 0,
            loading: false,
          },
        }))
      }
    },
    [projectId],
  )

  const toggle = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
        if (!folderCases[id]) {
          void loadFolderCases(id, false)
        }
      }
      return next
    })
  }

  const toggleUnfiled = () => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(UNFILED_KEY)) {
        next.delete(UNFILED_KEY)
      } else {
        next.add(UNFILED_KEY)
        if (!folderCases[UNFILED_KEY]) {
          void loadFolderCases(null, false)
        }
      }
      return next
    })
  }

  const onDropTarget = (suiteId: string | null) => {
    if (dragCaseId && canEditCases) {
      onMoveCase(dragCaseId, suiteId)
      setDragCaseId(null)
    } else if (dragFolderId && canManage && suiteId !== dragFolderId) {
      onMoveFolder(dragFolderId, suiteId)
      setDragFolderId(null)
    }
  }

  const renderCases = (suiteId: string | null, depth: number) => {
    const key = suiteId ?? UNFILED_KEY
    const state = folderCases[key]
    if (!state) return null
    return (
      <>
        {state.items.map((c, index) => (
          <div
            key={c.id}
            className={`folder-tree-case${selectedCaseId === c.id ? ' active' : ''}`}
            style={{ paddingLeft: `${0.75 + depth * 0.9}rem` }}
            draggable={canEditCases}
            onDragStart={() => setDragCaseId(c.id)}
            onDragEnd={() => setDragCaseId(null)}
            {...listItemProps(TestIds.folderCaseItem, {
              entityId: c.id,
              entityKey: c.key,
              index,
              depth,
            })}
          >
            <span className={`priority-bar priority-${c.priority}`} aria-hidden />
            <span
              className={`case-status-token status-${c.status}`}
              title={c.status}
            >
              <CaseStatusIcon status={c.status} />
              <span className="visually-hidden">{c.status}</span>
            </span>
            <Link
              to={`/cases/${c.id}`}
              className="folder-tree-case-link"
              {...listChildProps(TestIds.folderCaseItem, 'link')}
            >
              <span className="folder-tree-case-key">{c.key}</span>
              <span className="folder-tree-case-title">{c.title}</span>
            </Link>
            {canEditCases && (
              <button
                type="button"
                className="folder-tree-move-btn"
                aria-label={t('folders.moveCase')}
                onClick={() =>
                  setMobileMoveCaseId((prev) => (prev === c.id ? null : c.id))
                }
                {...listChildProps(TestIds.folderCaseItem, 'move')}
              >
                {t('folders.moveTo')}
              </button>
            )}
            {mobileMoveCaseId === c.id && (
              <label className="folder-tree-move-select">
                <span className="visually-hidden">{t('folders.moveTo')}</span>
                <select
                  value={c.suite_id ?? ''}
                  onChange={(e) => {
                    const next = e.target.value || null
                    onMoveCase(c.id, next)
                    setMobileMoveCaseId(null)
                  }}
                  {...listChildProps(TestIds.folderCaseItem, 'move-select')}
                >
                  <option value="">{t('folders.root')}</option>
                  {suites.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.name}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </div>
        ))}
        {state.loading && (
          <p
            className="muted"
            style={{ paddingLeft: `${0.75 + depth * 0.9}rem`, fontSize: '0.8rem' }}
            {...testIdProps(TestIds.folderTreeLoading)}
          >
            {t('common.loading')}
          </p>
        )}
        {!state.loading && state.items.length < state.total && (
          <button
            type="button"
            className="folder-tree-load-more"
            style={{ marginLeft: `${0.75 + depth * 0.9}rem` }}
            onClick={() => void loadFolderCases(suiteId, true)}
            {...testIdProps(TestIds.folderNodeLoadMore)}
          >
            {t('folders.loadMore')} ({state.items.length}/{state.total})
          </button>
        )}
      </>
    )
  }

  const renderFolder = (node: FolderNode, depth: number, index: number) => {
    const open = expanded.has(node.id)
    const recursive = node.case_count ?? 0
    const direct = node.direct_case_count ?? 0
    return (
      <div key={node.id} className="folder-tree-node">
        <div
          className={`folder-tree-folder${selectedSuiteId === node.id ? ' active' : ''}`}
          style={{ paddingLeft: `${0.35 + depth * 0.9}rem` }}
          draggable={canManage}
          onDragStart={() => setDragFolderId(node.id)}
          onDragEnd={() => setDragFolderId(null)}
          onDragOver={(e) => e.preventDefault()}
          onDrop={(e) => {
            e.preventDefault()
            onDropTarget(node.id)
          }}
          {...listItemProps(TestIds.folderNode, {
            entityId: node.id,
            index,
            depth,
          })}
        >
          <button
            type="button"
            className="folder-tree-twistie"
            aria-expanded={open}
            aria-label={open ? t('folders.collapse') : t('folders.expand')}
            onClick={() => toggle(node.id)}
            {...testIdProps(TestIds.folderNodeTwistie)}
          >
            {open ? '−' : '+'}
          </button>
          <button
            type="button"
            className="folder-tree-folder-btn"
            onClick={() => onSelectSuite(node.id)}
            title={countTooltip(recursive, direct)}
            {...testIdProps(TestIds.folderNodeButton)}
          >
            <span className="folder-icon" aria-hidden />
            <span
              className="folder-tree-folder-name"
              {...testIdProps(TestIds.folderNodeName)}
            >
              {node.name}
            </span>
            <span
              className="folder-tree-count"
              title={countTooltip(recursive, direct)}
              {...testIdProps(TestIds.folderNodeCount)}
            >
              {recursive}
            </span>
          </button>
          {canManage && (
            <span className="folder-tree-actions">
              <button
                type="button"
                className="icon-btn"
                title={t('folders.addChild')}
                aria-label={t('folders.addChild')}
                onClick={() => onCreateFolder(node.id)}
                {...testIdProps(TestIds.folderNodeAddChild)}
              >
                +
              </button>
              <button
                type="button"
                className="icon-btn"
                title={t('folders.rename')}
                aria-label={t('folders.rename')}
                onClick={() => onRenameFolder(node)}
                {...testIdProps(TestIds.folderNodeRename)}
              >
                ✎
              </button>
              <button
                type="button"
                className="icon-btn danger"
                title={t('folders.delete')}
                aria-label={t('folders.delete')}
                onClick={() => onDeleteFolder(node)}
                {...testIdProps(TestIds.folderNodeDelete)}
              >
                ⌫
              </button>
            </span>
          )}
        </div>
        {open && (
          <div className="folder-tree-children">
            {node.children.map((child, childIndex) =>
              renderFolder(child, depth + 1, childIndex),
            )}
            {renderCases(node.id, depth + 1)}
          </div>
        )}
      </div>
    )
  }

  const unfiledOpen = expanded.has(UNFILED_KEY)

  return (
    <div
      className="folder-tree"
      role="region"
      aria-label={t('folders.panel')}
      {...testIdProps(TestIds.folderTree)}
    >
      <div className="folder-tree-toolbar">
        <span className="folder-tree-title">{t('folders.title')}</span>
        {canManage && (
          <button
            type="button"
            className="btn secondary"
            onClick={() => onCreateFolder(null)}
            {...testIdProps(TestIds.folderTreeNewButton)}
          >
            {t('folders.new')}
          </button>
        )}
      </div>
      <button
        type="button"
        className={`folder-tree-root${selectedSuiteId === null ? ' active' : ''}`}
        onClick={() => onSelectSuite(null)}
        onDragOver={(e) => e.preventDefault()}
        onDrop={(e) => {
          e.preventDefault()
          onDropTarget(null)
        }}
        {...testIdProps(TestIds.folderAllCases)}
      >
        <span>{t('nav.allCases')}</span>
        <span className="folder-tree-count" title={t('folders.projectTotal')}>
          {projectCaseTotal}
        </span>
      </button>
      {tree.map((node, index) => renderFolder(node, 0, index))}
      <div className="folder-tree-node" {...testIdProps(TestIds.folderUnfiled)}>
        <div
          className="folder-tree-folder"
          style={{ paddingLeft: '0.35rem' }}
          onDragOver={(e) => e.preventDefault()}
          onDrop={(e) => {
            e.preventDefault()
            onDropTarget(null)
          }}
        >
          <button
            type="button"
            className="folder-tree-twistie"
            aria-expanded={unfiledOpen}
            aria-label={unfiledOpen ? t('folders.collapse') : t('folders.expand')}
            onClick={toggleUnfiled}
            {...testIdProps(TestIds.folderUnfiledTwistie)}
          >
            {unfiledOpen ? '−' : '+'}
          </button>
          <button
            type="button"
            className="folder-tree-folder-btn"
            onClick={() => onSelectSuite(null)}
            {...testIdProps(TestIds.folderUnfiledButton)}
          >
            <span className="folder-tree-folder-name">{t('folders.unfiled')}</span>
          </button>
        </div>
        {unfiledOpen && (
          <div className="folder-tree-children">{renderCases(null, 1)}</div>
        )}
      </div>
      {suites.length === 0 && (
        <p
          className="muted folder-tree-empty"
          {...testIdProps(TestIds.folderTreeEmpty)}
        >
          {t('folders.empty')}
        </p>
      )}
    </div>
  )
}
