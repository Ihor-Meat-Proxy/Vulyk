import { useMemo, useState } from 'react'
import type { ExecutionsTreeNode } from '../api'
import { useI18n } from '../i18n'
import { TestIds, listChildProps, listItemProps, testIdProps } from '../testing'
import {
  CampaignFormDialog,
  type CampaignFormValues,
} from './CampaignFormDialog'
import { StatusBadge } from './StatusBadge'

type Props = {
  nodes: ExecutionsTreeNode[]
  selectedIterationId: string | null
  onSelectIteration: (id: string) => void
  canManage?: boolean
  onCreateCampaign?: (values: CampaignFormValues) => void | Promise<void>
  onCreateIteration?: (
    campaignId: string,
    values: CampaignFormValues,
  ) => void | Promise<void>
}

function TreeNode({
  node,
  depth,
  index,
  selectedIterationId,
  onSelectIteration,
  canManage,
  onRequestCreateIteration,
}: {
  node: ExecutionsTreeNode
  depth: number
  index: number
  selectedIterationId: string | null
  onSelectIteration: (id: string) => void
  canManage?: boolean
  onRequestCreateIteration?: (campaign: ExecutionsTreeNode) => void
}) {
  const { t } = useI18n()
  const [open, setOpen] = useState(true)
  const isCampaign = node.kind === 'campaign'
  const isIteration = node.kind === 'iteration'
  const selected = isIteration && selectedIterationId === node.id

  return (
    <div
      className="exec-tree-node"
      {...listItemProps(TestIds.executionNode, {
        entityId: node.id,
        // Prefer stable reference; omit freeform display names (spaces)
        entityKey: node.reference,
        index,
        depth,
      })}
    >
      <div
        className={`exec-tree-row${selected ? ' active' : ''}`}
        style={{ paddingLeft: `${0.4 + depth * 0.85}rem` }}
      >
        {isCampaign ? (
          <button
            type="button"
            className="exec-tree-toggle"
            aria-expanded={open}
            onClick={() => setOpen((v) => !v)}
            {...listChildProps(TestIds.executionNode, 'toggle')}
          >
            {open ? '▾' : '▸'}
          </button>
        ) : (
          <span className="exec-tree-spacer" aria-hidden />
        )}
        <button
          type="button"
          className="exec-tree-label"
          onClick={() => {
            if (isIteration) onSelectIteration(node.id)
            else setOpen((v) => !v)
          }}
          {...listChildProps(TestIds.executionNode, 'label')}
        >
          <span className="exec-tree-name">{node.name}</span>
          {node.reference ? (
            <span className="muted exec-tree-ref">{node.reference}</span>
          ) : null}
          {isIteration && node.status ? (
            <StatusBadge status={node.status} />
          ) : null}
        </button>
        {isCampaign && canManage && onRequestCreateIteration ? (
          <button
            type="button"
            className="btn secondary exec-tree-add"
            title={t('executions.newIteration')}
            onClick={() => onRequestCreateIteration(node)}
            {...listChildProps(TestIds.executionNode, 'add-iteration')}
          >
            +
          </button>
        ) : null}
      </div>
      {isCampaign && open
        ? node.children.map((child, childIndex) => (
            <TreeNode
              key={child.id}
              node={child}
              depth={depth + 1}
              index={childIndex}
              selectedIterationId={selectedIterationId}
              onSelectIteration={onSelectIteration}
              canManage={canManage}
              onRequestCreateIteration={onRequestCreateIteration}
            />
          ))
        : null}
    </div>
  )
}

export function ExecutionsTree({
  nodes,
  selectedIterationId,
  onSelectIteration,
  canManage,
  onCreateCampaign,
  onCreateIteration,
}: Props) {
  const { t } = useI18n()
  const [campaignOpen, setCampaignOpen] = useState(false)
  const [iterationCampaign, setIterationCampaign] =
    useState<ExecutionsTreeNode | null>(null)
  const [busy, setBusy] = useState(false)
  const [apiError, setApiError] = useState<string | null>(null)

  const campaignNames = useMemo(
    () => nodes.filter((n) => n.kind === 'campaign').map((n) => n.name),
    [nodes],
  )

  const iterationNames = useMemo(() => {
    if (!iterationCampaign) return []
    return iterationCampaign.children
      .filter((c) => c.kind === 'iteration')
      .map((c) => c.name)
  }, [iterationCampaign])

  const submitCampaign = async (values: CampaignFormValues) => {
    if (!onCreateCampaign) return
    setBusy(true)
    setApiError(null)
    try {
      await onCreateCampaign(values)
      setCampaignOpen(false)
    } catch (err) {
      setApiError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setBusy(false)
    }
  }

  const submitIteration = async (values: CampaignFormValues) => {
    if (!onCreateIteration || !iterationCampaign) return
    setBusy(true)
    setApiError(null)
    try {
      await onCreateIteration(iterationCampaign.id, values)
      setIterationCampaign(null)
    } catch (err) {
      setApiError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setBusy(false)
    }
  }

  return (
    <aside
      className="exec-tree panel"
      aria-label={t('executions.tree')}
      {...testIdProps(TestIds.executionsTree)}
    >
      <div className="panel-header exec-tree-header">
        <h2>{t('executions.tree')}</h2>
        {canManage && onCreateCampaign ? (
          <button
            type="button"
            className="btn secondary"
            onClick={() => {
              setApiError(null)
              setCampaignOpen(true)
            }}
            {...testIdProps(TestIds.executionsNewCampaignButton)}
          >
            {t('executions.newCampaign')}
          </button>
        ) : null}
      </div>
      {nodes.length === 0 ? (
        <p className="muted" style={{ margin: '0.5rem 0.75rem' }}>
          {t('executions.emptyTree')}
        </p>
      ) : (
        <div className="exec-tree-list">
          {nodes.map((node, index) => (
            <TreeNode
              key={node.id}
              node={node}
              depth={0}
              index={index}
              selectedIterationId={selectedIterationId}
              onSelectIteration={onSelectIteration}
              canManage={canManage}
              onRequestCreateIteration={
                onCreateIteration
                  ? (campaign) => {
                      setApiError(null)
                      setIterationCampaign(campaign)
                    }
                  : undefined
              }
            />
          ))}
        </div>
      )}

      <CampaignFormDialog
        open={campaignOpen}
        mode="campaign"
        existingNames={campaignNames}
        busy={busy}
        apiError={campaignOpen ? apiError : null}
        onCancel={() => {
          setCampaignOpen(false)
          setApiError(null)
        }}
        onSubmit={submitCampaign}
      />
      <CampaignFormDialog
        open={iterationCampaign != null}
        mode="iteration"
        existingNames={iterationNames}
        busy={busy}
        apiError={iterationCampaign ? apiError : null}
        onCancel={() => {
          setIterationCampaign(null)
          setApiError(null)
        }}
        onSubmit={submitIteration}
      />
    </aside>
  )
}
