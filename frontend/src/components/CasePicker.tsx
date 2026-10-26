import { useEffect, useMemo, useState } from 'react'
import { api, type Suite, type TestCase } from '../api'
import { useI18n } from '../i18n'
import { TestIds, listChildProps, listItemProps, testIdProps } from '../testing'

export type PickedCase = { id: string; key: string; title: string }

type Props = {
  projectId: string
  selected: PickedCase[]
  onChange: (next: PickedCase[]) => void
}

type SuiteNode = Suite & { children: SuiteNode[] }

function buildTree(suites: Suite[]): SuiteNode[] {
  const map = new Map<string, SuiteNode>()
  suites.forEach((s) => map.set(s.id, { ...s, children: [] }))
  const roots: SuiteNode[] = []
  map.forEach((node) => {
    if (node.parent_id && map.has(node.parent_id)) {
      map.get(node.parent_id)!.children.push(node)
    } else {
      roots.push(node)
    }
  })
  return roots
}

export function CasePicker({ projectId, selected, onChange }: Props) {
  const { t } = useI18n()
  const [suites, setSuites] = useState<Suite[]>([])
  const [cases, setCases] = useState<TestCase[]>([])
  const [search, setSearch] = useState('')
  const [activeSuite, setActiveSuite] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    void (async () => {
      try {
        const [s, page] = await Promise.all([
          api.listSuites(projectId),
          api.listCases(projectId, { limit: 500 }),
        ])
        if (cancelled) return
        setSuites(s)
        setCases(page.items)
        setError(null)
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err))
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [projectId])

  const tree = useMemo(() => buildTree(suites), [suites])
  const selectedIds = useMemo(
    () => new Set(selected.map((c) => c.id)),
    [selected],
  )

  const visibleCases = useMemo(() => {
    const q = search.trim().toLowerCase()
    return cases.filter((c) => {
      if (activeSuite && c.suite_id !== activeSuite) return false
      if (!q) return true
      return (
        c.key.toLowerCase().includes(q) ||
        c.title.toLowerCase().includes(q) ||
        c.description.toLowerCase().includes(q)
      )
    })
  }, [cases, search, activeSuite])

  const toggle = (c: TestCase) => {
    if (selectedIds.has(c.id)) {
      onChange(selected.filter((x) => x.id !== c.id))
    } else {
      onChange([
        ...selected,
        { id: c.id, key: c.key, title: c.title },
      ])
    }
  }

  const selectFolder = (suiteId: string) => {
    const inFolder = cases.filter((c) => c.suite_id === suiteId)
    const next = new Map(selected.map((c) => [c.id, c]))
    for (const c of inFolder) {
      next.set(c.id, { id: c.id, key: c.key, title: c.title })
    }
    onChange([...next.values()])
  }

  const removeChip = (id: string) => {
    onChange(selected.filter((c) => c.id !== id))
  }

  const renderNode = (node: SuiteNode, depth: number, index: number) => (
    <li key={node.id} style={{ marginLeft: depth * 12 }}>
      <div className="picker-folder-row">
        <button
          type="button"
          className={
            activeSuite === node.id ? 'linkish active' : 'linkish'
          }
          onClick={() =>
            setActiveSuite((cur) => (cur === node.id ? null : node.id))
          }
          {...listItemProps(TestIds.casePickerFolder, {
            entityId: node.id,
            index,
            depth,
          })}
        >
          {node.name}
          {typeof node.case_count === 'number' ? ` (${node.case_count})` : ''}
        </button>
        <button
          type="button"
          className="btn secondary tiny"
          onClick={() => selectFolder(node.id)}
          title={t('generate.selectFolder')}
          {...listChildProps(TestIds.casePickerFolder, 'select')}
        >
          {t('generate.selectFolder')}
        </button>
      </div>
      {node.children.length > 0 && (
        <ul className="plain-list picker-tree">
          {node.children.map((ch, childIndex) =>
            renderNode(ch, depth + 1, childIndex),
          )}
        </ul>
      )}
    </li>
  )

  return (
    <div className="case-picker" {...testIdProps(TestIds.casePicker)}>
      <div className="case-picker-chips" aria-live="polite">
        {selected.length === 0 ? (
          <span className="muted">{t('generate.noCasesSelected')}</span>
        ) : (
          selected.map((c, index) => (
            <button
              key={c.id}
              type="button"
              className="chip"
              onClick={() => removeChip(c.id)}
              title={c.title}
              {...listItemProps(TestIds.casePickerChip, {
                entityId: c.id,
                entityKey: c.key,
                index,
              })}
            >
              <span className="badge">{c.key}</span>
              <span aria-hidden="true"> ×</span>
            </button>
          ))
        )}
      </div>

      <label className="field">
        <span>{t('generate.searchCases')}</span>
        <input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder={t('generate.searchCasesPlaceholder')}
          {...testIdProps(TestIds.casePickerSearchInput)}
        />
      </label>

      {error && <p className="error">{error}</p>}
      {loading && <p className="muted">{t('common.loading')}</p>}

      <div className="case-picker-grid">
        <div className="case-picker-folders">
          <h4>{t('generate.folders')}</h4>
          <ul className="plain-list picker-tree">
            <li>
              <button
                type="button"
                className={activeSuite == null ? 'linkish active' : 'linkish'}
                onClick={() => setActiveSuite(null)}
                {...testIdProps(TestIds.casePickerAllFolders)}
              >
                {t('generate.allFolders')}
              </button>
            </li>
            {tree.map((n, index) => renderNode(n, 0, index))}
          </ul>
        </div>
        <div className="case-picker-list">
          <h4>{t('generate.cases')}</h4>
          <ul className="plain-list">
            {visibleCases.map((c, index) => (
              <li
                key={c.id}
                {...listItemProps(TestIds.casePickerItem, {
                  entityId: c.id,
                  entityKey: c.key,
                  index,
                })}
              >
                <label className="check">
                  <input
                    type="checkbox"
                    checked={selectedIds.has(c.id)}
                    onChange={() => toggle(c)}
                    {...listChildProps(TestIds.casePickerItem, 'checkbox')}
                  />
                  <span className="badge">{c.key}</span> {c.title}
                </label>
              </li>
            ))}
            {visibleCases.length === 0 && (
              <li
                className="muted"
                {...testIdProps(TestIds.casePickerEmpty)}
              >
                {t('cases.empty')}
              </li>
            )}
          </ul>
        </div>
      </div>
    </div>
  )
}
