import { useDeferredValue, useEffect, useState, useTransition } from 'react'
import { Link } from 'react-router-dom'
import { api, type TestCase } from '../api'
import { VirtualList } from '../components/VirtualList'

type Props = {
  projectId: string | null
  suiteId: string | null
}

const PAGE_SIZE = 200

export function CaseList({ projectId, suiteId }: Props) {
  const [cases, setCases] = useState<TestCase[]>([])
  const [total, setTotal] = useState(0)
  const [offset, setOffset] = useState(0)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [tag, setTag] = useState('')
  const [status, setStatus] = useState('')
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [pending, startTransition] = useTransition()
  const deferredQuery = useDeferredValue(query)
  const [newKey, setNewKey] = useState('')
  const [newTitle, setNewTitle] = useState('')
  const [creating, setCreating] = useState(false)

  useEffect(() => {
    setOffset(0)
    setSelected(new Set())
  }, [projectId, suiteId, deferredQuery, tag, status])

  useEffect(() => {
    if (!projectId) {
      setCases([])
      setTotal(0)
      return
    }
    let cancelled = false
    setLoading(true)
    api
      .listCases(projectId, {
        suiteId,
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
  }, [projectId, suiteId, deferredQuery, tag, status, offset])

  const toggle = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const createCase = async () => {
    if (!projectId || !newKey.trim() || !newTitle.trim()) return
    setCreating(true)
    try {
      await api.createCase(projectId, {
        key: newKey.trim().toUpperCase(),
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
      setNewKey('')
      setNewTitle('')
      setOffset(0)
      const page = await api.listCases(projectId, {
        suiteId,
        q: deferredQuery || undefined,
        tag: tag || undefined,
        status: status || undefined,
        limit: PAGE_SIZE,
        offset: 0,
      })
      setCases(page.items)
      setTotal(page.total)
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
    setOffset(0)
    setSelected(new Set())
    const page = await api.listCases(projectId, {
      suiteId,
      q: deferredQuery || undefined,
      tag: tag || undefined,
      status: status || undefined,
      limit: PAGE_SIZE,
      offset: 0,
    })
    setCases(page.items)
    setTotal(page.total)
  }

  if (!projectId) {
    return (
      <div className="empty">
        Create a project via the API to get started, then refresh.
      </div>
    )
  }

  return (
    <div className="case-browser">
      <div className="toolbar create-case-bar">
        <input
          className="filter-input"
          placeholder="Case key"
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
          aria-label="New case key"
        />
        <input
          className="search-input"
          placeholder="New case title"
          value={newTitle}
          onChange={(e) => setNewTitle(e.target.value)}
          aria-label="New case title"
        />
        <button
          type="button"
          className="btn"
          disabled={creating || !newKey.trim() || !newTitle.trim()}
          onClick={() => void createCase()}
        >
          {creating ? 'Creating…' : 'Create case'}
        </button>
      </div>
      <div className="toolbar">
        <input
          className="search-input"
          placeholder="Search cases (FTS)…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          aria-label="Search cases"
        />
        <input
          className="filter-input"
          placeholder="Tag"
          value={tag}
          onChange={(e) => setTag(e.target.value)}
        />
        <select
          className="filter-input"
          value={status}
          onChange={(e) => setStatus(e.target.value)}
        >
          <option value="">All statuses</option>
          <option value="draft">draft</option>
          <option value="ready">ready</option>
          <option value="deprecated">deprecated</option>
        </select>
        <span className="muted">
          {total.toLocaleString()} cases
          {pending || loading ? ' · loading…' : ''}
        </span>
      </div>

      {selected.size > 0 && (
        <div className="bulk-bar">
          <span>{selected.size} selected</span>
          <button type="button" className="btn secondary" onClick={() => bulkStatus('ready')}>
            Mark ready
          </button>
          <button
            type="button"
            className="btn secondary"
            onClick={() => bulkStatus('deprecated')}
          >
            Deprecate
          </button>
          <button type="button" className="btn secondary" onClick={() => setSelected(new Set())}>
            Clear
          </button>
        </div>
      )}

      {error && <p className="error">{error}</p>}

      {cases.length === 0 && !loading ? (
        <div className="empty">No test cases in this view yet.</div>
      ) : (
        <>
          <div className="table-head">
            <span />
            <span>Key</span>
            <span>Title</span>
            <span>Priority</span>
            <span>Status</span>
          </div>
          <VirtualList
            items={cases}
            rowHeight={44}
            height={520}
            className="case-virtual"
            renderRow={(c) => (
              <div className="table-row">
                <input
                  type="checkbox"
                  checked={selected.has(c.id)}
                  onChange={() => toggle(c.id)}
                  aria-label={`Select ${c.key}`}
                />
                <Link to={`/cases/${c.id}`}>
                  <span className="badge">{c.key}</span>
                </Link>
                <Link to={`/cases/${c.id}`}>{c.title}</Link>
                <span>{c.priority}</span>
                <span className={`status-pill status-${c.status}`}>{c.status}</span>
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
                Load more ({cases.length.toLocaleString()} / {total.toLocaleString()})
              </button>
            </div>
          )}
        </>
      )}
    </div>
  )
}
