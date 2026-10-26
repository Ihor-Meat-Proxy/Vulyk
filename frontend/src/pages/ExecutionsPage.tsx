import { useCallback, useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import {
  api,
  type AutomatedSuite,
  type ExecutionsTreeNode,
  type IterationDashboard as Dashboard,
  type UpdateIteration,
} from '../api'
import { AutomatedSuitesTable } from '../components/AutomatedSuitesTable'
import { ExecutionsTree } from '../components/ExecutionsTree'
import { IterationDashboard } from '../components/IterationDashboard'
import { RunDetailsDrawer } from '../components/RunDetailsDrawer'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { useI18n } from '../i18n'
import { TestIds, testIdProps } from '../testing'

type Props = {
  projectId: string | null
}

export function ExecutionsPage({ projectId }: Props) {
  const { t } = useI18n()
  const navigate = useNavigate()
  const { iterationId: routeIterationId } = useParams()
  const { can } = useProjectPermissions(projectId)
  const canManage = can('manage_plans')
  const canDelete = can('execute_runs')
  const canRerun = can('trigger_ci')

  const [tree, setTree] = useState<ExecutionsTreeNode[]>([])
  const [dashboard, setDashboard] = useState<Dashboard | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(
    routeIterationId ?? null,
  )
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [drawerSuite, setDrawerSuite] = useState<AutomatedSuite | null>(null)
  const [suitesRefresh, setSuitesRefresh] = useState(0)

  const refreshTree = useCallback(async () => {
    if (!projectId) {
      setTree([])
      return
    }
    const nodes = await api.executionsTree(projectId)
    setTree(nodes)
  }, [projectId])

  useEffect(() => {
    if (!projectId) return
    let cancelled = false
    setLoading(true)
    refreshTree()
      .then(() => {
        if (!cancelled) setError(null)
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
  }, [projectId, refreshTree])

  useEffect(() => {
    setSelectedId(routeIterationId ?? null)
  }, [routeIterationId])

  useEffect(() => {
    if (!projectId || !selectedId) {
      setDashboard(null)
      return
    }
    let cancelled = false
    api
      .iterationDashboard(projectId, selectedId)
      .then((data) => {
        if (!cancelled) {
          setDashboard(data)
          setError(null)
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setDashboard(null)
          setError(err.message)
        }
      })
    return () => {
      cancelled = true
    }
  }, [projectId, selectedId, suitesRefresh])

  const selectIteration = (id: string) => {
    setSelectedId(id)
    navigate(`/executions/iterations/${id}`)
  }

  const createCampaign = async (name: string) => {
    if (!projectId) return
    try {
      await api.createCampaign(projectId, { name })
      await refreshTree()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  const createIteration = async (campaignId: string, name: string) => {
    if (!projectId) return
    try {
      const iteration = await api.createIteration(projectId, campaignId, {
        name,
        cadence: 'manual',
        mode: 'automated',
      })
      await refreshTree()
      selectIteration(iteration.id)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  const saveSchedule = async (body: UpdateIteration) => {
    if (!projectId || !selectedId) return
    await api.updateIteration(projectId, selectedId, body)
    const data = await api.iterationDashboard(projectId, selectedId)
    setDashboard(data)
  }

  if (!projectId) {
    return <div className="empty">Select a project to view executions.</div>
  }

  return (
    <div className="executions-page" {...testIdProps(TestIds.executionsPage)}>
      <header className="executions-page-header">
        <h1 style={{ margin: 0 }}>{t('executions.title')}</h1>
        <p className="muted" style={{ margin: '0.25rem 0 0' }}>
          {t('executions.subtitle')}
        </p>
      </header>

      {error ? (
        <p className="error" {...testIdProps(TestIds.executionsError)}>
          {error}
        </p>
      ) : null}
      {loading && tree.length === 0 ? (
        <p className="muted" {...testIdProps(TestIds.executionsLoading)}>
          {t('common.loading')}
        </p>
      ) : null}

      <div className="executions-layout">
        <ExecutionsTree
          nodes={tree}
          selectedIterationId={selectedId}
          onSelectIteration={selectIteration}
          canManage={canManage}
          onCreateCampaign={(name) => void createCampaign(name)}
          onCreateIteration={(cid, name) => void createIteration(cid, name)}
        />

        <div className="executions-content">
          {!selectedId || !dashboard ? (
            <div
              className="panel empty"
              {...testIdProps(TestIds.executionsEmpty)}
            >
              {t('executions.selectIteration')}
            </div>
          ) : (
            <>
              <IterationDashboard
                dashboard={dashboard}
                canEdit={canManage}
                onSaveSchedule={saveSchedule}
              />
              <AutomatedSuitesTable
                projectId={projectId}
                iterationId={selectedId}
                canDelete={canDelete}
                canRerun={canRerun}
                refreshKey={suitesRefresh}
                onOpenDetails={(suite) => setDrawerSuite(suite)}
              />
            </>
          )}
        </div>
      </div>

      <RunDetailsDrawer
        projectId={projectId}
        suite={drawerSuite}
        open={Boolean(drawerSuite)}
        onClose={() => {
          setDrawerSuite(null)
          setSuitesRefresh((n) => n + 1)
        }}
      />
    </div>
  )
}
