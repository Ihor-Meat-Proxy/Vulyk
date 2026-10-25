import { Link } from 'react-router-dom'
import type { TestCase } from '../api'

type Props = {
  cases: TestCase[]
  loading: boolean
  error: string | null
  projectId: string | null
}

export function CaseList({ cases, loading, error, projectId }: Props) {
  if (!projectId) {
    return (
      <div className="empty">
        Create a project via the API to get started, then refresh.
      </div>
    )
  }

  if (loading) {
    return <p className="muted">Loading cases…</p>
  }

  if (error) {
    return <p className="error">{error}</p>
  }

  if (cases.length === 0) {
    return <div className="empty">No test cases in this view yet.</div>
  }

  return (
    <table className="table">
      <thead>
        <tr>
          <th>Key</th>
          <th>Title</th>
          <th>Priority</th>
          <th>Status</th>
        </tr>
      </thead>
      <tbody>
        {cases.map((c) => (
          <tr key={c.id}>
            <td>
              <Link to={`/cases/${c.id}`}>
                <span className="badge">{c.key}</span>
              </Link>
            </td>
            <td>
              <Link to={`/cases/${c.id}`}>{c.title}</Link>
            </td>
            <td>{c.priority}</td>
            <td>{c.status}</td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}
