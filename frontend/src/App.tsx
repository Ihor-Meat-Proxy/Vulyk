import { useCallback, useEffect, useState } from 'react'
import { Navigate, Route, Routes, useNavigate } from 'react-router-dom'
import { api, type Project, type Suite, type TestCase } from './api'
import { useAuth } from './auth'
import { Layout } from './components/Layout'
import { Admin } from './pages/Admin'
import { CaseDetail } from './pages/CaseDetail'
import { CaseList } from './pages/CaseList'
import { Chat } from './pages/Chat'
import { Generate } from './pages/Generate'
import { ImportPage } from './pages/ImportPage'
import { Login } from './pages/Login'
import { Placeholder } from './pages/Placeholder'
import { RequirementsPage } from './pages/RequirementsPage'
import { getStoredToken } from './token'

const PROJECT_KEY = 'vulyk.selectedProjectId'

export default function App() {
  const navigate = useNavigate()
  const { status, loading: authLoading, user } = useAuth()
  const [projects, setProjects] = useState<Project[]>([])
  const [projectId, setProjectId] = useState<string | null>(
    () => localStorage.getItem(PROJECT_KEY),
  )
  const [suites, setSuites] = useState<Suite[]>([])
  const [cases, setCases] = useState<TestCase[]>([])
  const [selectedSuiteId, setSelectedSuiteId] = useState<string | null>(null)
  const [loadingCases, setLoadingCases] = useState(false)
  const [casesError, setCasesError] = useState<string | null>(null)
  const [bootError, setBootError] = useState<string | null>(null)

  const project = projects.find((p) => p.id === projectId) ?? projects[0] ?? null
  const activeProjectId = project?.id ?? null

  const needsLogin =
    Boolean(status?.bootstrap_available) ||
    (Boolean(status?.auth_required) && !status?.authenticated)

  const authHeaders = (): Record<string, string> => {
    const token = getStoredToken()
    return token ? { Authorization: `Bearer ${token}` } : {}
  }

  const refreshProjects = useCallback(async () => {
    let list = await api.listProjects()
    if (list.length === 0 && (!status?.auth_required || status.authenticated)) {
      const demo = await api.createProject({
        key: 'DEMO',
        name: 'Demo project',
        description: 'Seeded by the Vulyk UI for local exploration.',
      })
      const suite = await fetch(`/api/v1/projects/${demo.id}/suites`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        credentials: 'include',
        body: JSON.stringify({ name: 'Smoke', sort_order: 0 }),
      }).then((r) => r.json())
      await fetch(`/api/v1/projects/${demo.id}/cases`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        credentials: 'include',
        body: JSON.stringify({
          key: 'DEMO-1',
          title: 'Health endpoint responds',
          suite_id: suite.id,
          description: 'Verify API liveness.',
          priority: 'high',
          status: 'ready',
          steps: [
            {
              action: 'GET /health',
              expected_result: 'HTTP 200 with status ok',
            },
          ],
        }),
      })
      list = await api.listProjects()
    }
    setProjects(list)
    const stored = localStorage.getItem(PROJECT_KEY)
    const initial =
      list.find((p) => p.id === stored)?.id ?? list[0]?.id ?? null
    setProjectId(initial)
  }, [status?.auth_required, status?.authenticated])

  useEffect(() => {
    if (authLoading || needsLogin) return
    let cancelled = false
    ;(async () => {
      try {
        await refreshProjects()
        if (!cancelled) setBootError(null)
      } catch (err) {
        if (!cancelled) {
          setBootError(err instanceof Error ? err.message : String(err))
        }
      }
    })()
    return () => {
      cancelled = true
    }
  }, [authLoading, needsLogin, refreshProjects, user?.id])

  useEffect(() => {
    if (activeProjectId) {
      localStorage.setItem(PROJECT_KEY, activeProjectId)
    }
  }, [activeProjectId])

  useEffect(() => {
    if (!activeProjectId) {
      setSuites([])
      return
    }
    let cancelled = false
    api
      .listSuites(activeProjectId)
      .then((data) => {
        if (!cancelled) setSuites(data)
      })
      .catch(() => {
        if (!cancelled) setSuites([])
      })
    return () => {
      cancelled = true
    }
  }, [activeProjectId])

  const refreshCases = useCallback(async () => {
    if (!activeProjectId) {
      setCases([])
      return
    }
    setLoadingCases(true)
    try {
      const data = await api.listCases(activeProjectId, selectedSuiteId)
      setCases(data)
      setCasesError(null)
    } catch (err) {
      setCasesError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoadingCases(false)
    }
  }, [activeProjectId, selectedSuiteId])

  useEffect(() => {
    void refreshCases()
  }, [refreshCases])

  const onSelectProject = (id: string) => {
    setProjectId(id)
    setSelectedSuiteId(null)
    navigate('/')
  }

  if (authLoading) {
    return (
      <p className="muted" style={{ margin: '2rem' }}>
        Loading…
      </p>
    )
  }

  if (needsLogin) {
    return <Login />
  }

  return (
    <>
      {bootError && (
        <p className="error" style={{ margin: '0.75rem 1rem' }}>
          API unavailable: {bootError}
        </p>
      )}
      <Routes>
        <Route
          element={
            <Layout
              projects={projects}
              project={project}
              suites={suites}
              selectedSuiteId={selectedSuiteId}
              onSelectProject={onSelectProject}
              onSelectSuite={setSelectedSuiteId}
            />
          }
        >
          <Route
            index
            element={
              <CaseList
                cases={cases}
                loading={loadingCases}
                error={casesError}
                projectId={activeProjectId}
              />
            }
          />
          <Route
            path="cases/:caseId"
            element={<CaseDetail projectId={activeProjectId} />}
          />
          <Route
            path="requirements"
            element={<RequirementsPage projectId={activeProjectId} />}
          />
          <Route
            path="import"
            element={<ImportPage projectId={activeProjectId} />}
          />
          <Route
            path="generate"
            element={<Generate projectId={activeProjectId} />}
          />
          <Route path="chat" element={<Chat projectId={activeProjectId} />} />
          <Route
            path="runs"
            element={
              <Placeholder
                title="Runs"
                blurb="Test plans and runs land in workstream W3."
              />
            }
          />
          <Route
            path="reports"
            element={
              <Placeholder
                title="Reports"
                blurb="Dashboards and coverage reports land in workstream W8."
              />
            }
          />
          <Route
            path="admin"
            element={
              <Admin
                projects={projects}
                project={project}
                onProjectsChanged={() => {
                  void refreshProjects()
                }}
              />
            }
          />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </>
  )
}
