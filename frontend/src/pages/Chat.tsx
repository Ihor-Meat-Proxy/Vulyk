import { FormEvent, useEffect, useMemo, useState } from 'react'
import {
  api,
  type ChatPlacement,
  type ChatProposal,
  type ChatProposeResponse,
  type LlmConfig,
  type Project,
  type ProposedCaseDraft,
  type ProposedRequirementDraft,
  type RequirementFolder,
  type Suite,
} from '../api'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { useI18n } from '../i18n'
import { TestIds, listChildProps, listItemProps, testIdProps } from '../testing'

type Props = {
  projectId: string | null
}

type Turn = {
  id: string
  role: 'user' | 'assistant'
  text: string
  meta?: string
}

let chatTurnSeq = 0
function nextTurnId(): string {
  chatTurnSeq += 1
  return `turn-${chatTurnSeq}`
}

function configLabel(c: LlmConfig): string {
  const scope = c.project_id == null ? 'global' : 'project'
  const def = c.is_default ? ' · default' : ''
  return `${c.name} (${c.provider}/${c.model || '—'}, ${scope}${def})`
}

function folderLabel(
  id: string | null,
  suites: Suite[],
  reqFolders: RequirementFolder[],
  forRequirements: boolean,
): string {
  if (!id) return 'Unfiled'
  if (forRequirements) {
    return reqFolders.find((f) => f.id === id)?.name ?? id
  }
  return suites.find((s) => s.id === id)?.name ?? id
}

export function Chat({ projectId }: Props) {
  const { t } = useI18n()
  const { can } = useProjectPermissions(projectId)
  const canLlm = can('llm_assist')
  const canCreateCases = can('create_edit_cases')
  const canManageReqs = can('manage_requirements')

  const [question, setQuestion] = useState('')
  const [turns, setTurns] = useState<Turn[]>([])
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [configs, setConfigs] = useState<LlmConfig[]>([])
  const [configId, setConfigId] = useState('')
  const [projects, setProjects] = useState<Project[]>([])
  const [suites, setSuites] = useState<Suite[]>([])
  const [reqFolders, setReqFolders] = useState<RequirementFolder[]>([])
  const [proposal, setProposal] = useState<ChatProposal | null>(null)
  /** Stable React keys for editable draft rows (index keys remount on reorder/edit). */
  const [draftCaseKeys, setDraftCaseKeys] = useState<string[]>([])
  const [draftReqKeys, setDraftReqKeys] = useState<string[]>([])
  const [confirmPrompt, setConfirmPrompt] = useState<string | null>(null)
  const [draftMeta, setDraftMeta] = useState<{
    provider: string
    model: string
  } | null>(null)
  const [notice, setNotice] = useState<string | null>(null)

  useEffect(() => {
    if (!projectId) return
    void api.listLlmConfigs(projectId).then((list) => {
      setConfigs(list)
      const def =
        list.find((c) => c.project_id === projectId && c.is_default) ??
        list.find((c) => c.is_default) ??
        list[0]
      if (def) setConfigId((cur) => cur || def.id)
    })
    void api.listProjects().then(setProjects).catch(() => setProjects([]))
  }, [projectId])

  const forRequirements = Boolean(
    proposal &&
      proposal.requirements.length > 0 &&
      proposal.cases.length === 0,
  )

  useEffect(() => {
    if (!proposal) return
    const pid = proposal.placement.project_id
    void api.listSuites(pid).then(setSuites).catch(() => setSuites([]))
    void api
      .listRequirementFolders(pid)
      .then(setReqFolders)
      .catch(() => setReqFolders([]))
  }, [proposal?.placement.project_id, proposal?.cases.length, proposal?.requirements.length])

  const modelOptions = useMemo(() => configs, [configs])

  const canConfirm = Boolean(
    proposal &&
      ((proposal.cases.length > 0 && canCreateCases) ||
        (proposal.requirements.length > 0 && canManageReqs)),
  )

  const updatePlacement = (patch: Partial<ChatPlacement>) => {
    setProposal((prev) => {
      if (!prev) return prev
      const placement = { ...prev.placement, ...patch }
      if (patch.folder_id !== undefined) {
        placement.folder_path = folderLabel(
          patch.folder_id,
          suites,
          reqFolders,
          forRequirements,
        )
      }
      return { ...prev, placement }
    })
  }

  const updateCase = (index: number, patch: Partial<ProposedCaseDraft>) => {
    setProposal((prev) => {
      if (!prev) return prev
      const cases = prev.cases.map((c, i) =>
        i === index ? { ...c, ...patch } : c,
      )
      return { ...prev, cases }
    })
  }

  const updateReq = (
    index: number,
    patch: Partial<ProposedRequirementDraft>,
  ) => {
    setProposal((prev) => {
      if (!prev) return prev
      const requirements = prev.requirements.map((r, i) =>
        i === index ? { ...r, ...patch } : r,
      )
      return { ...prev, requirements }
    })
  }

  const onAsk = async (e: FormEvent) => {
    e.preventDefault()
    if (!projectId || !question.trim()) return
    const q = question.trim()
    setQuestion('')
    setTurns((prev) => [...prev, { id: nextTurnId(), role: 'user', text: q }])
    setBusy(true)
    setError(null)
    setNotice(null)
    setProposal(null)
    setDraftCaseKeys([])
    setDraftReqKeys([])
    setConfirmPrompt(null)
    try {
      const history = turns.slice(-8).map((turn) => ({
        role: turn.role === 'user' ? 'user' : 'assistant',
        content: turn.text,
      }))
      const res: ChatProposeResponse = await api.chatPropose(projectId, {
        message: q,
        history,
        config_id: configId || undefined,
      })
      setDraftMeta({ provider: res.provider, model: res.model })

      if (res.kind === 'clarify' && res.clarify_question) {
        setTurns((prev) => [
          ...prev,
          {
            id: nextTurnId(),
            role: 'assistant',
            text: res.clarify_question!,
            meta: t('chat.metaClarify', {
              provider: res.provider,
              model: res.model,
            }),
          },
        ])
        return
      }

      if (res.kind === 'answer' || !res.proposal) {
        setTurns((prev) => [
          ...prev,
          {
            id: nextTurnId(),
            role: 'assistant',
            text: res.answer || t('chat.emptyAnswer'),
            meta: t('chat.metaAnswer', {
              provider: res.provider,
              model: res.model,
            }),
          },
        ])
        return
      }

      setProposal(res.proposal)
      setDraftCaseKeys(res.proposal.cases.map(() => nextTurnId()))
      setDraftReqKeys(res.proposal.requirements.map(() => nextTurnId()))
      setConfirmPrompt(res.confirmation_prompt ?? t('chat.confirmPrompt'))
      setTurns((prev) => [
        ...prev,
        {
          id: nextTurnId(),
          role: 'assistant',
          text: res.confirmation_prompt ?? t('chat.confirmPrompt'),
          meta: t('chat.metaPropose', {
            provider: res.provider,
            model: res.model,
          }),
        },
      ])
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      setError(message)
      setTurns((prev) => [
        ...prev,
        {
          id: nextTurnId(),
          role: 'assistant',
          text: message,
          meta: t('chat.metaError'),
        },
      ])
    } finally {
      setBusy(false)
    }
  }

  const onCreate = async () => {
    if (!projectId || !proposal || !draftMeta) return
    if (!canConfirm) {
      setError(t('chat.noCreatePermission'))
      return
    }
    setBusy(true)
    setError(null)
    try {
      const targetProject = proposal.placement.project_id || projectId
      const res = await api.chatConfirm(targetProject, {
        proposal,
        config_id: configId || undefined,
        provider: draftMeta.provider,
        model: draftMeta.model,
      })
      const createdBits: string[] = []
      if (res.cases.length) {
        createdBits.push(
          t('chat.createdCases', { count: String(res.cases.length) }),
        )
      }
      if (res.requirements.length) {
        createdBits.push(
          t('chat.createdReqs', { count: String(res.requirements.length) }),
        )
      }
      setNotice(createdBits.join(' · ') || t('chat.createdOk'))
      setTurns((prev) => [
        ...prev,
        {
          id: nextTurnId(),
          role: 'assistant',
          text: createdBits.join('\n') || t('chat.createdOk'),
          meta: t('chat.metaCreated', {
            provider: draftMeta.provider,
            model: draftMeta.model,
          }),
        },
      ])
      setProposal(null)
      setDraftCaseKeys([])
      setDraftReqKeys([])
      setConfirmPrompt(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onRevise = () => {
    setQuestion(t('chat.reviseHint'))
    setProposal(null)
    setDraftCaseKeys([])
    setDraftReqKeys([])
    setConfirmPrompt(null)
  }

  if (!projectId) {
    return <p className="muted">{t('chat.selectProject')}</p>
  }

  if (!canLlm) {
    return (
      <div className="panel">
        <p className="muted">{t('chat.noPermission')}</p>
      </div>
    )
  }

  const folderOptions = forRequirements ? reqFolders : suites

  return (
    <div className="chat-page" {...testIdProps(TestIds.chatPage)}>
      <header className="panel-header">
        <h2>{t('chat.title')}</h2>
        <p className="muted">{t('chat.subtitle')}</p>
      </header>

      <label className="field chat-model">
        <span>{t('generate.model')}</span>
        <select
          value={configId}
          onChange={(e) => setConfigId(e.target.value)}
          aria-label={t('generate.model')}
          {...testIdProps(TestIds.chatModelSelect)}
        >
          {modelOptions.length === 0 && (
            <option value="">{t('generate.mockFallback')}</option>
          )}
          {modelOptions.map((c) => (
            <option key={c.id} value={c.id}>
              {configLabel(c)}
            </option>
          ))}
        </select>
      </label>

      <div
        className="chat-log"
        aria-live="polite"
        {...testIdProps(TestIds.chatMessages)}
      >
        {turns.length === 0 && (
          <p className="muted" {...testIdProps(TestIds.chatEmpty)}>
            {t('chat.emptyHint')}
          </p>
        )}
        {turns.map((turn, i) => (
          <div
            key={turn.id}
            className={`chat-bubble ${turn.role}`}
            {...listItemProps(TestIds.chatMessage, {
              entityId: turn.id,
              entityKey: turn.role,
              index: i,
            })}
          >
            <div className="chat-role">
              {turn.role === 'user' ? t('chat.you') : t('chat.assistant')}
            </div>
            <div className="chat-text">{turn.text}</div>
            {turn.meta && <div className="muted chat-meta">{turn.meta}</div>}
          </div>
        ))}
      </div>

      {proposal && (
        <section
          className="chat-proposal"
          aria-label={t('chat.proposalCard')}
          {...testIdProps(TestIds.chatProposal)}
        >
          <h3>{t('chat.proposalTitle')}</h3>
          <p className="muted">{confirmPrompt}</p>
          <p className="chat-placement-reason">{proposal.placement.reason}</p>

          <div className="chat-proposal-grid">
            <label className="field">
              <span>{t('chat.project')}</span>
              <select
                value={proposal.placement.project_id}
                onChange={(e) =>
                  updatePlacement({
                    project_id: e.target.value,
                    folder_id: null,
                    folder_path: 'Unfiled',
                  })
                }
                aria-label={t('chat.project')}
                {...testIdProps(TestIds.chatProjectSelect)}
              >
                {projects.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.key} — {p.name}
                  </option>
                ))}
              </select>
            </label>

            <label className="field">
              <span>{t('chat.folder')}</span>
              <select
                value={proposal.placement.folder_id ?? ''}
                onChange={(e) =>
                  updatePlacement({
                    folder_id: e.target.value || null,
                    new_folder_name: proposal.placement.new_folder_name,
                  })
                }
                aria-label={t('chat.folder')}
                {...testIdProps(TestIds.chatFolderSelect)}
              >
                <option value="">{t('chat.folderRoot')}</option>
                {folderOptions.map((f) => (
                  <option key={f.id} value={f.id}>
                    {f.name}
                  </option>
                ))}
              </select>
            </label>

            <label className="field">
              <span>{t('chat.newFolder')}</span>
              <input
                value={proposal.placement.new_folder_name ?? ''}
                onChange={(e) =>
                  updatePlacement({
                    new_folder_name: e.target.value.trim()
                      ? e.target.value
                      : null,
                  })
                }
                placeholder={t('chat.newFolderPlaceholder')}
                aria-label={t('chat.newFolder')}
                {...testIdProps(TestIds.chatNewFolderInput)}
              />
            </label>
          </div>

          {proposal.cases.map((c, i) => (
            <div
              key={draftCaseKeys[i] ?? `case-${i}`}
              className="chat-draft-item"
              {...listItemProps(TestIds.chatDraftCase, {
                entityId: draftCaseKeys[i] ?? String(i),
                index: i,
              })}
            >
              <h4>
                {t('chat.caseDraft')} {i + 1}
              </h4>
              <label className="field">
                <span>{t('chat.draftTitle')}</span>
                <input
                  value={c.title}
                  onChange={(e) => updateCase(i, { title: e.target.value })}
                  {...listChildProps(TestIds.chatDraftCase, 'title')}
                />
              </label>
              <label className="field">
                <span>{t('chat.draftDescription')}</span>
                <textarea
                  rows={2}
                  value={c.description}
                  onChange={(e) =>
                    updateCase(i, { description: e.target.value })
                  }
                  {...listChildProps(TestIds.chatDraftCase, 'description')}
                />
              </label>
              <label className="field">
                <span>{t('chat.draftPrecondition')}</span>
                <textarea
                  rows={2}
                  value={c.precondition}
                  onChange={(e) =>
                    updateCase(i, { precondition: e.target.value })
                  }
                  {...listChildProps(TestIds.chatDraftCase, 'precondition')}
                />
              </label>
              <div className="chat-proposal-grid">
                <label className="field">
                  <span>{t('chat.draftPriority')}</span>
                  <select
                    value={c.priority}
                    onChange={(e) =>
                      updateCase(i, { priority: e.target.value })
                    }
                    {...listChildProps(TestIds.chatDraftCase, 'priority')}
                  >
                    <option value="critical">critical</option>
                    <option value="high">high</option>
                    <option value="medium">medium</option>
                    <option value="low">low</option>
                  </select>
                </label>
                <label className="field">
                  <span>{t('chat.draftTags')}</span>
                  <input
                    value={c.tags.join(', ')}
                    onChange={(e) =>
                      updateCase(i, {
                        tags: e.target.value
                          .split(',')
                          .map((s) => s.trim())
                          .filter(Boolean),
                      })
                    }
                    {...listChildProps(TestIds.chatDraftCase, 'tags')}
                  />
                </label>
              </div>
              <div className="chat-steps">
                {c.steps.map((s, si) => (
                  <div key={si} className="chat-step">
                    <label className="field">
                      <span>
                        {t('chat.stepAction')} {si + 1}
                      </span>
                      <input
                        value={s.action}
                        onChange={(e) => {
                          const steps = c.steps.map((st, j) =>
                            j === si
                              ? { ...st, action: e.target.value }
                              : st,
                          )
                          updateCase(i, { steps })
                        }}
                        {...listChildProps(TestIds.chatDraftCase, 'step-action')}
                      />
                    </label>
                    <label className="field">
                      <span>{t('chat.stepExpected')}</span>
                      <input
                        value={s.expected_result}
                        onChange={(e) => {
                          const steps = c.steps.map((st, j) =>
                            j === si
                              ? { ...st, expected_result: e.target.value }
                              : st,
                          )
                          updateCase(i, { steps })
                        }}
                        {...listChildProps(
                          TestIds.chatDraftCase,
                          'step-expected',
                        )}
                      />
                    </label>
                  </div>
                ))}
              </div>
            </div>
          ))}

          {proposal.requirements.map((r, i) => (
            <div
              key={draftReqKeys[i] ?? `req-${i}`}
              className="chat-draft-item"
              {...listItemProps(TestIds.chatDraftRequirement, {
                entityId: draftReqKeys[i] ?? String(i),
                index: i,
              })}
            >
              <h4>
                {t('chat.reqDraft')} {i + 1}
              </h4>
              <label className="field">
                <span>{t('chat.draftTitle')}</span>
                <input
                  value={r.title}
                  onChange={(e) => updateReq(i, { title: e.target.value })}
                  {...listChildProps(TestIds.chatDraftRequirement, 'title')}
                />
              </label>
              <label className="field">
                <span>{t('chat.draftDescription')}</span>
                <textarea
                  rows={2}
                  value={r.description}
                  onChange={(e) =>
                    updateReq(i, { description: e.target.value })
                  }
                  {...listChildProps(
                    TestIds.chatDraftRequirement,
                    'description',
                  )}
                />
              </label>
              <label className="field">
                <span>{t('chat.acceptanceCriteria')}</span>
                <textarea
                  rows={3}
                  value={r.acceptance_criteria}
                  onChange={(e) =>
                    updateReq(i, { acceptance_criteria: e.target.value })
                  }
                  {...listChildProps(
                    TestIds.chatDraftRequirement,
                    'acceptance',
                  )}
                />
              </label>
            </div>
          ))}

          <div className="chat-proposal-actions">
            <button
              type="button"
              className="btn"
              disabled={busy || !canConfirm}
              onClick={() => void onCreate()}
              {...testIdProps(TestIds.chatApplyButton)}
            >
              {t('chat.create')}
            </button>
            <button
              type="button"
              className="btn secondary"
              disabled={busy}
              onClick={onRevise}
              {...testIdProps(TestIds.chatReviseButton)}
            >
              {t('chat.revise')}
            </button>
            <button
              type="button"
              className="btn secondary"
              disabled={busy}
              onClick={() => {
                setProposal(null)
                setDraftCaseKeys([])
                setDraftReqKeys([])
                setConfirmPrompt(null)
              }}
              {...testIdProps(TestIds.chatDismissButton)}
            >
              {t('chat.dismiss')}
            </button>
          </div>
          {!canConfirm && (
            <p className="muted">{t('chat.noCreatePermission')}</p>
          )}
        </section>
      )}

      {error && (
        <p className="error" {...testIdProps(TestIds.chatError)}>
          {error}
        </p>
      )}
      {notice && <p className="notice">{notice}</p>}
      {busy && (
        <p className="muted" {...testIdProps(TestIds.chatLoading)}>
          {t('chat.thinking')}
        </p>
      )}

      <form className="chat-compose" onSubmit={(e) => void onAsk(e)}>
        <input
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          placeholder={t('chat.placeholder')}
          disabled={busy}
          aria-label={t('chat.placeholder')}
          {...testIdProps(TestIds.chatInput)}
        />
        <button
          type="submit"
          disabled={busy || !question.trim()}
          {...testIdProps(TestIds.chatSendButton)}
        >
          {busy ? t('chat.thinking') : t('chat.ask')}
        </button>
      </form>
    </div>
  )
}
