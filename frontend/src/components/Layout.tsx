import { NavLink, Outlet } from 'react-router-dom'
import type { Project, Suite } from '../api'

type Props = {
  projects: Project[]
  project: Project | null
  suites: Suite[]
  selectedSuiteId: string | null
  onSelectProject: (id: string) => void
  onSelectSuite: (id: string | null) => void
}

export function Layout({
  projects,
  project,
  suites,
  selectedSuiteId,
  onSelectProject,
  onSelectSuite,
}: Props) {
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">Vulyk</span>
          <span className="brand-tag">Test management</span>
        </div>

        <nav className="nav" aria-label="Primary">
          <NavLink to="/" end>
            Cases
          </NavLink>
          <NavLink to="/requirements">Requirements</NavLink>
          <NavLink to="/import">Import</NavLink>
          <NavLink to="/generate">Generate</NavLink>
          <NavLink to="/chat">Chat</NavLink>
          <NavLink to="/runs">Runs</NavLink>
          <NavLink to="/reports">Reports</NavLink>
          <NavLink to="/admin">Admin</NavLink>
        </nav>

        <div>
          <div className="muted" style={{ marginBottom: '0.5rem', fontSize: '0.8rem' }}>
            Suites
          </div>
          <div className="suite-tree">
            <button
              type="button"
              className={selectedSuiteId === null ? 'active' : undefined}
              onClick={() => onSelectSuite(null)}
            >
              All cases
            </button>
            {suites.map((suite) => (
              <button
                key={suite.id}
                type="button"
                className={selectedSuiteId === suite.id ? 'active' : undefined}
                onClick={() => onSelectSuite(suite.id)}
              >
                {suite.name}
              </button>
            ))}
            {project && suites.length === 0 && (
              <p className="muted" style={{ margin: '0.35rem 0.55rem', fontSize: '0.85rem' }}>
                No suites yet
              </p>
            )}
          </div>
        </div>
      </aside>

      <div className="main">
        <header className="topbar">
          <h1>{project ? project.name : 'Vulyk'}</h1>
          <label>
            <span className="muted" style={{ marginRight: '0.5rem' }}>
              Project
            </span>
            <select
              className="project-select"
              value={project?.id ?? ''}
              onChange={(e) => onSelectProject(e.target.value)}
              disabled={projects.length === 0}
            >
              {projects.length === 0 && <option value="">No projects</option>}
              {projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.key} — {p.name}
                </option>
              ))}
            </select>
          </label>
        </header>
        <main className="content">
          <Outlet />
        </main>
      </div>
    </div>
  )
}
