import { useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import type { Suite, TestCase } from '../api'
import { useI18n } from '../i18n'
import { CaseStatusIcon } from './CaseStatusIcon'

type Props = {
  suites: Suite[]
  cases: TestCase[]
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
}

type FolderNode = Suite & { children: FolderNode[] }

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
    nodes.sort((a, b) => a.sort_order - b.sort_order || a.name.localeCompare(b.name))
    nodes.forEach((n) => sortRec(n.children))
  }
  sortRec(roots)
  return roots
}

export function CaseFolderTree({
  suites,
  cases,
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
}: Props) {
  const { t } = useI18n()
  const tree = useMemo(() => buildTree(suites), [suites])
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set(suites.map((s) => s.id)))
  const [dragCaseId, setDragCaseId] = useState<string | null>(null)
  const [dragFolderId, setDragFolderId] = useState<string | null>(null)
  const [mobileMoveCaseId, setMobileMoveCaseId] = useState<string | null>(null)

  const casesBySuite = useMemo(() => {
    const map = new Map<string | null, TestCase[]>()
    for (const c of cases) {
      const key = c.suite_id
      const list = map.get(key) ?? []
      list.push(c)
      map.set(key, list)
    }
    for (const list of map.values()) {
      list.sort(
        (a, b) =>
          (a.sort_order ?? 0) - (b.sort_order ?? 0) || a.key.localeCompare(b.key),
      )
    }
    return map
  }, [cases])

  const toggle = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
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
    const list = casesBySuite.get(suiteId) ?? []
    return list.map((c) => (
      <div
        key={c.id}
        className={`folder-tree-case${selectedCaseId === c.id ? ' active' : ''}`}
        style={{ paddingLeft: `${0.75 + depth * 0.9}rem` }}
        draggable={canEditCases}
        onDragStart={() => setDragCaseId(c.id)}
        onDragEnd={() => setDragCaseId(null)}
      >
        <span className={`priority-bar priority-${c.priority}`} aria-hidden />
        <span className={`case-status-token status-${c.status}`} title={c.status}>
          <CaseStatusIcon status={c.status} />
          <span className="visually-hidden">{c.status}</span>
        </span>
        <Link to={`/cases/${c.id}`} className="folder-tree-case-link">
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
    ))
  }

  const renderFolder = (node: FolderNode, depth: number) => {
    const open = expanded.has(node.id)
    const count = node.case_count ?? (casesBySuite.get(node.id)?.length ?? 0)
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
        >
          <button
            type="button"
            className="folder-tree-twistie"
            aria-expanded={open}
            aria-label={open ? t('folders.collapse') : t('folders.expand')}
            onClick={() => toggle(node.id)}
          >
            {open ? '−' : '+'}
          </button>
          <button
            type="button"
            className="folder-tree-folder-btn"
            onClick={() => onSelectSuite(node.id)}
          >
            <span className="folder-icon" aria-hidden />
            <span className="folder-tree-folder-name">{node.name}</span>
            <span className="folder-tree-count">{count}</span>
          </button>
          {canManage && (
            <span className="folder-tree-actions">
              <button
                type="button"
                className="icon-btn"
                title={t('folders.addChild')}
                aria-label={t('folders.addChild')}
                onClick={() => onCreateFolder(node.id)}
              >
                +
              </button>
              <button
                type="button"
                className="icon-btn"
                title={t('folders.rename')}
                aria-label={t('folders.rename')}
                onClick={() => onRenameFolder(node)}
              >
                ✎
              </button>
              <button
                type="button"
                className="icon-btn danger"
                title={t('folders.delete')}
                aria-label={t('folders.delete')}
                onClick={() => onDeleteFolder(node)}
              >
                ⌫
              </button>
            </span>
          )}
        </div>
        {open && (
          <div className="folder-tree-children">
            {node.children.map((child) => renderFolder(child, depth + 1))}
            {renderCases(node.id, depth + 1)}
          </div>
        )}
      </div>
    )
  }

  return (
    <div className="folder-tree" role="region" aria-label={t('folders.panel')}>
      <div className="folder-tree-toolbar">
        <span className="folder-tree-title">{t('folders.title')}</span>
        {canManage && (
          <button
            type="button"
            className="btn secondary"
            onClick={() => onCreateFolder(null)}
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
      >
        {t('nav.allCases')}
      </button>
      {tree.map((node) => renderFolder(node, 0))}
      {renderCases(null, 0)}
      {suites.length === 0 && (
        <p className="muted folder-tree-empty">{t('folders.empty')}</p>
      )}
    </div>
  )
}
