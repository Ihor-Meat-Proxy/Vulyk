import { useCallback, useEffect, useState, type FormEvent } from 'react'
import { api, type Requirement } from '../api'
import { useProjectPermissions } from '../hooks/useProjectPermissions'

type Props = {
  projectId: string | null
}

export function RequirementsPage({ projectId }: Props) {
  const { can } = useProjectPermissions(projectId)
  const canManage = can('manage_requirements')
  const [items, setItems] = useState<Requirement[]>([])
  const [title, setTitle] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  const refresh = useCallback(async () => {
    if (!projectId) {
      setItems([])
      return
    }
    setLoading(true)
    try {
      const data = await api.listRequirements(projectId)
      setItems(data)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }, [projectId])

  useEffect(() => {
    void refresh()
  }, [refresh])

  if (!projectId) {
    return <p className="muted">Select a project first.</p>
  }

  const onCreate = async (e: FormEvent) => {
    e.preventDefault()
    if (!title.trim()) return
    try {
      await api.createRequirement(projectId, { title: title.trim() })
      setTitle('')
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <div className="panel">
      <h2>Requirements</h2>
      <p className="muted" style={{ marginTop: 0 }}>
        Track requirements locally or import them from OpenProject / Jira
        integrations. Link cases from the case detail view.
      </p>

      {canManage && (
      <form className="req-form" onSubmit={(e) => void onCreate(e)}>
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="New requirement title"
          aria-label="Requirement title"
        />
        <button type="submit" className="btn" disabled={!title.trim()}>
          Add
        </button>
      </form>
      )}

      {error && <p className="error">{error}</p>}
      {loading ? (
        <p className="muted">Loading…</p>
      ) : items.length === 0 ? (
        <p className="empty">No requirements yet.</p>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>Key</th>
              <th>Title</th>
              <th>Source</th>
            </tr>
          </thead>
          <tbody>
            {items.map((r) => (
              <tr key={r.id}>
                <td>
                  <span className="badge">{r.external_key ?? '—'}</span>
                </td>
                <td>
                  {r.url ? (
                    <a href={r.url} target="_blank" rel="noreferrer">
                      {r.title}
                    </a>
                  ) : (
                    r.title
                  )}
                </td>
                <td className="muted">{r.source}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  )
}
