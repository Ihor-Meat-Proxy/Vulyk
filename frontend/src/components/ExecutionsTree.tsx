import { useState } from 'react'
import type { ExecutionsTreeNode } from '../api'
import { useI18n } from '../i18n'
import { TestIds, listChildProps, listItemProps, testIdProps } from '../testing'
import { StatusBadge } from './StatusBadge'

type Props = {
  nodes: ExecutionsTreeNode[]
  selectedIterationId: string | null
  onSelectIteration: (id: string) => void
  canManage?: boolean
  onCreateCampaign?: (name: string) => void
  onCreateIteration?: (campaignId: string, name: string) => void
}

function TreeNode({
  node,
  depth,
  index,
  selectedIterationId,
  onSelectIteration,
  canManage,
  onCreateIteration,
}: {
  node: ExecutionsTreeNode
  depth: number
  index: number
  selectedIterationId: string | null
  onSelectIteration: (id: string) => void
  canManage?: boolean
  onCreateIteration?: (campaignId: string, name: string) => void
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
        entityKey: node.reference ?? node.name,
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
        {isCampaign && canManage && onCreateIteration ? (
          <button
            type="button"
            className="btn secondary exec-tree-add"
            title={t('executions.newIteration')}
            onClick={() => {
              const name = window.prompt(t('executions.iterationName'))
              if (name?.trim()) onCreateIteration(node.id, name.trim())
            }}
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
              onCreateIteration={onCreateIteration}
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
              const name = window.prompt(t('executions.campaignName'))
              if (name?.trim()) onCreateCampaign(name.trim())
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
              onCreateIteration={onCreateIteration}
            />
          ))}
        </div>
      )}
    </aside>
  )
}
