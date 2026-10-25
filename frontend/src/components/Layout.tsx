import { useEffect, useId, useState } from 'react'
import { NavLink, Outlet, useLocation } from 'react-router-dom'
import type { Project, Suite } from '../api'
import { useAuth } from '../auth'
import { BottomNav } from './BottomNav'
import { PwaToasts } from './PwaToasts'

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
  const { user, logout, status } = useAuth()
  const location = useLocation()
  const [drawerOpen, setDrawerOpen] = useState(false)
  const drawerTitleId = useId()

  useEffect(() => {
    setDrawerOpen(false)
  }, [location.pathname, selectedSuiteId])

  useEffect(() => {
    if (!drawerOpen) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setDrawerOpen(false)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [drawerOpen])

  const selectSuite = (id: string | null) => {
    onSelectSuite(id)
    setDrawerOpen(false)
  }

  return (
    <div className={`app-shell${drawerOpen ? ' drawer-open' : ''}`}>
      <PwaToasts />

      {drawerOpen && (
        <button
          type="button"
          className="drawer-backdrop"
          aria-label="Close suite drawer"
          onClick={() => setDrawerOpen(false)}
        />
      )}

      <aside
        className="sidebar"
        id="suite-drawer"
        aria-labelledby={drawerTitleId}
      >
        <div className="brand">
          <span className="brand-mark" id={drawerTitleId}>
            Vulyk
          </span>
          <span className="brand-tag">Test management</span>
        </div>

        <nav className="nav nav-desktop" aria-label="Primary">
          <NavLink to="/" end>
            Cases
          </NavLink>
          <NavLink to="/requirements">Requirements</NavLink>
          <NavLink to="/import">Import</NavLink>
          <NavLink to="/generate">Generate</NavLink>
          <NavLink to="/chat">Chat</NavLink>
          <NavLink to="/runs">Runs</NavLink>
          <NavLink to="/ci">CI</NavLink>
          <NavLink to="/reports">Reports</NavLink>
          <NavLink to="/admin">Admin</NavLink>
        </nav>

        <div className="suite-block">
          <div className="muted" style={{ marginBottom: '0.5rem', fontSize: '0.8rem' }}>
            Suites
          </div>
          <div className="suite-tree">
            <button
              type="button"
              className={selectedSuiteId === null ? 'active' : undefined}
              onClick={() => selectSuite(null)}
            >
              All cases
            </button>
            {suites.map((suite) => (
              <button
                key={suite.id}
                type="button"
                className={selectedSuiteId === suite.id ? 'active' : undefined}
                onClick={() => selectSuite(suite.id)}
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
          <div className="topbar-lead">
            <button
              type="button"
              className="btn secondary menu-toggle"
              aria-expanded={drawerOpen}
              aria-controls="suite-drawer"
              onClick={() => setDrawerOpen((v) => !v)}
            >
              Suites
            </button>
            <h1>{project ? project.name : 'Vulyk'}</h1>
          </div>
          <div className="topbar-actions">
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
            {user ? (
              <div className="user-chip">
                <span>{user.display_name}</span>
                <button
                  type="button"
                  className="btn secondary"
                  onClick={() => void logout()}
                >
                  Sign out
                </button>
              </div>
            ) : status && !status.auth_required ? (
              <span className="muted">Open mode</span>
            ) : null}
          </div>
        </header>
        <main className="content">
          <Outlet />
        </main>
        <BottomNav />
      </div>
    </div>
  )
}
