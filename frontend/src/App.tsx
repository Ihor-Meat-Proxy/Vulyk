import { useCallback, useEffect, useState } from 'react'
import {
  Navigate,
  Route,
  Routes,
  useLocation,
  useNavigate,
} from 'react-router-dom'
import { api, type Project, type Suite } from './api'
import { useAuth } from './auth'
import { Layout } from './components/Layout'
import { useI18n } from './i18n'
import { Admin } from './pages/Admin'
import { Appearance } from './pages/Appearance'
import { CaseDetail } from './pages/CaseDetail'
import { CaseList } from './pages/CaseList'
import { ChangePassword } from './pages/ChangePassword'
import { Chat } from './pages/Chat'
import { Generate } from './pages/Generate'
import { ImportPage } from './pages/ImportPage'
import { Login } from './pages/Login'
import { ExecutionPage } from './pages/ExecutionPage'
import { ExecutionsPage } from './pages/ExecutionsPage'
import { Runs } from './pages/Runs'
import { Reports } from './pages/Reports'
import { RequirementsPage } from './pages/RequirementsPage'
import { RunDetail } from './pages/RunDetail'
import { RunsPage } from './pages/RunsPage'
import { Settings } from './pages/Settings'
import { SetupWizard } from './pages/SetupWizard'
import { getStoredToken } from './token'

const PROJECT_KEY = 'vulyk.selectedProjectId'

export default function App() {
  const navigate = useNavigate()
  const location = useLocation()
  const { status, loading: authLoading, user } = useAuth()
  const { t } = useI18n()
  const [projects, setProjects] = useState<Project[]>([])
  const [projectId, setProjectId] = useState<string | null>(
    () => localStorage.getItem(PROJECT_KEY),
  )
  const [suites, setSuites] = useState<Suite[]>([])
  const [selectedSuiteId, setSelectedSuiteId] = useState<string | null>(null)
  const [bootError, setBootError] = useState<string | null>(null)

  const project = projects.find((p) => p.id === projectId) ?? projects[0] ?? null
  const activeProjectId = project?.id ?? null

  const needsSetup =
    Boolean(status?.setup_required) && !status?.authenticated

  const wantsLoginPage = location.pathname === '/login'

  const needsLogin =
    !needsSetup &&
    (Boolean(status?.bootstrap_available) ||
      (Boolean(status?.auth_required) && !status?.authenticated) ||
      (wantsLoginPage && !status?.authenticated))

  const needsPasswordChange =
    Boolean(status?.authenticated && user?.must_change_password)

  const authHeaders = (): Record<string, string> => {
    const token = getStoredToken()
    return token ? { Authorization: `Bearer ${token}` } : {}
  }

  const refreshProjects = useCallback(async () => {
    let list = await api.listProjects()
    if (list.length === 0 && (!status?.auth_required || status.authenticated)) {
      const demo = await api.createProject({
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
          title: 'Health endpoint responds',
          suite_id: suite.id,
          description: 'Verify API liveness.',
          priority: 'high',
          status: 'ready',
          tag_names: ['smoke'],
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
    if (authLoading || needsSetup || needsLogin || needsPasswordChange) return
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
  }, [
    authLoading,
    needsSetup,
    needsLogin,
    needsPasswordChange,
    refreshProjects,
    user?.id,
  ])

  useEffect(() => {
    if (activeProjectId) {
      localStorage.setItem(PROJECT_KEY, activeProjectId)
    }
  }, [activeProjectId])

  const refreshSuites = useCallback(async () => {
    if (!activeProjectId) {
      setSuites([])
      return
    }
    try {
      setSuites(await api.listSuites(activeProjectId))
    } catch {
      setSuites([])
    }
  }, [activeProjectId])

  useEffect(() => {
    void refreshSuites()
  }, [refreshSuites])

  const onSelectProject = (id: string) => {
    setProjectId(id)
    setSelectedSuiteId(null)
    navigate('/')
  }

  if (authLoading) {
    return (
      <p className="muted" style={{ margin: '2rem' }}>
        {t('common.loading')}
      </p>
    )
  }

  if (needsSetup) {
    return <SetupWizard />
  }

  if (needsLogin) {
    return <Login />
  }

  if (needsPasswordChange) {
    return <ChangePassword />
  }

  if (wantsLoginPage && status?.authenticated) {
    return <Navigate to="/" replace />
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
              onSelectProject={onSelectProject}
            />
          }
        >
          <Route
            index
            element={
              <CaseList
                projectId={activeProjectId}
                suiteId={selectedSuiteId}
                suites={suites}
                onSelectSuite={setSelectedSuiteId}
                onSuitesChanged={() => {
                  void refreshSuites()
                }}
              />
            }
          />
          <Route
            path="cases/:caseId"
            element={
              <CaseDetail
                projectId={activeProjectId}
                projectName={project?.name ?? ''}
                suites={suites}
                onEnsureProject={(id) => {
                  if (id !== projectId) setProjectId(id)
                }}
                onBackToList={(suiteId) => {
                  setSelectedSuiteId(suiteId)
                  navigate('/')
                }}
              />
            }
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
            element={<RunsPage projectId={activeProjectId} suites={suites} />}
          />
          <Route
            path="runs/:runId"
            element={<RunDetail projectId={activeProjectId} />}
          />
          <Route
            path="executions"
            element={<ExecutionsPage projectId={activeProjectId} />}
          />
          <Route
            path="executions/iterations/:iterationId"
            element={<ExecutionsPage projectId={activeProjectId} />}
          />
          <Route
            path="executions/runs/:runId"
            element={<ExecutionPage projectId={activeProjectId} />}
          />
          <Route
            path="executions/runs/:runId/cases/:caseId"
            element={<ExecutionPage projectId={activeProjectId} />}
          />
          <Route path="ci" element={<Runs projectId={activeProjectId} />} />
          <Route
            path="reports"
            element={<Reports projectId={activeProjectId} />}
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
          <Route path="appearance" element={<Appearance />} />
          <Route path="settings" element={<Settings />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </>
  )
}
