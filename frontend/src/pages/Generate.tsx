import { FormEvent, useEffect, useMemo, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import {
  api,
  type AutomationRepo,
  type ContextFileRef,
  type DiffFileEntry,
  type DiffPreviewResponse,
  type GeneratedCaseDraft,
  type LlmConfig,
  type RemotePullRequest,
  type TestCase,
} from '../api'
import {
  CASE_PICKER_LIMIT,
  CasePicker,
  type PickedCase,
} from '../components/CasePicker'
import { DocsLink } from '../components/DocsLink'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { useI18n } from '../i18n'
import { TestIds, listItemProps, testIdProps } from '../testing'

type Props = {
  projectId: string | null
}

function configLabel(
  c: LlmConfig,
  t: (key: string, vars?: Record<string, string | number>) => string,
): string {
  const scope =
    c.project_id == null ? t('common.scopeGlobal') : t('common.scopeProject')
  const def = c.is_default ? t('common.defaultBadge') : ''
  return `${c.name} (${c.provider}/${c.model || '—'}, ${scope}${def})`
}

function repoOptionLabel(repo: AutomationRepo): string {
  const name = repo.name?.trim() || repo.remote_url
  return `${name} (${repo.provider}/${repo.branch})`
}

export function Generate({ projectId }: Props) {
  const { t } = useI18n()
  const { can } = useProjectPermissions(projectId)
  const canLlm = can('llm_assist')
  const [searchParams, setSearchParams] = useSearchParams()

  const [requirements, setRequirements] = useState('')
  const [drafts, setDrafts] = useState<GeneratedCaseDraft[]>([])
  const [code, setCode] = useState('')
  const [contextFiles, setContextFiles] = useState<ContextFileRef[]>([])
  const [framework, setFramework] = useState<'playwright' | 'pytest'>(
    'playwright',
  )
  const [picked, setPicked] = useState<PickedCase[]>([])
  const [configs, setConfigs] = useState<LlmConfig[]>([])
  const [caseConfigId, setCaseConfigId] = useState('')
  const [codeConfigId, setCodeConfigId] = useState('')
  const [linkedRepos, setLinkedRepos] = useState<AutomationRepo[]>([])
  const [repoId, setRepoId] = useState('')
  const [diffRepoId, setDiffRepoId] = useState('')
  const [diffMode, setDiffMode] = useState<'pull' | 'branch'>('pull')
  const [pulls, setPulls] = useState<RemotePullRequest[]>([])
  const [pullNumber, setPullNumber] = useState('')
  const [baseRef, setBaseRef] = useState('main')
  const [headRef, setHeadRef] = useState('')
  const [diffPreview, setDiffPreview] = useState<DiffPreviewResponse | null>(
    null,
  )
  const [excludedPaths, setExcludedPaths] = useState<string[]>([])
  const [diffDrafts, setDiffDrafts] = useState<GeneratedCaseDraft[]>([])
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)

  useEffect(() => {
    if (!projectId) return
    void api.listLlmConfigs(projectId).then((list) => {
      setConfigs(list)
      const def =
        list.find((c) => c.project_id === projectId && c.is_default) ??
        list.find((c) => c.is_default) ??
        list[0]
      if (def) {
        setCaseConfigId((cur) => cur || def.id)
        setCodeConfigId((cur) => cur || def.id)
      }
    })
    void api
      .listAutomationReposForProject(projectId)
      .then((list) => {
        setLinkedRepos(list)
        const preferred =
          list.find((r) => r.is_default)?.id ?? list[0]?.id ?? ''
        setRepoId((cur) => {
          if (cur && list.some((r) => r.id === cur)) return cur
          return preferred
        })
        setDiffRepoId((cur) => {
          if (cur && list.some((r) => r.id === cur)) return cur
          return preferred
        })
        const def = list.find((r) => r.id === preferred)
        if (def?.branch) setBaseRef(def.branch)
      })
      .catch(() => {
        setLinkedRepos([])
        setRepoId('')
        setDiffRepoId('')
      })
  }, [projectId])

  useEffect(() => {
    if (!diffRepoId || diffMode !== 'pull') {
      setPulls([])
      setPullNumber('')
      return
    }
    let cancelled = false
    void api
      .listAutomationRepoPulls(diffRepoId)
      .then((list) => {
        if (cancelled) return
        setPulls(list)
        setPullNumber((cur) => {
          if (cur && list.some((p) => String(p.number) === cur)) return cur
          return list[0] ? String(list[0].number) : ''
        })
      })
      .catch(() => {
        if (!cancelled) {
          setPulls([])
          setPullNumber('')
        }
      })
    return () => {
      cancelled = true
    }
  }, [diffRepoId, diffMode])

  // Prefill from Cases page: ?cases=id1,id2
  useEffect(() => {
    if (!projectId) return
    const raw = searchParams.get('cases')
    if (!raw) return
    const ids = raw
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
    if (ids.length === 0) return
    let cancelled = false
    void (async () => {
      try {
        const page = await api.listCases(projectId, {
          limit: CASE_PICKER_LIMIT,
        })
        if (cancelled) return
        const map = new Map(page.items.map((c: TestCase) => [c.id, c]))
        const next: PickedCase[] = []
        let fetchedExtra = 0
        for (const id of ids) {
          let c = map.get(id)
          if (!c) {
            // Prefill IDs can sit past the soft list cap — resolve by id (review #45).
            try {
              const detail = await api.getCase(projectId, id)
              c = detail.case
              fetchedExtra += 1
            } catch {
              continue
            }
          }
          next.push({ id: c.id, key: c.key, title: c.title })
        }
        if (cancelled) return
        if (next.length) setPicked(next)
        if (page.total > page.items.length || fetchedExtra > 0) {
          setNotice(
            t('generate.casesTruncatedPrefill', {
              shown: page.items.length,
              total: page.total,
              limit: CASE_PICKER_LIMIT,
            }),
          )
        }
        setSearchParams({}, { replace: true })
      } catch {
        /* ignore — user can pick manually */
      }
    })()
    return () => {
      cancelled = true
    }
  }, [projectId, searchParams, setSearchParams])

  const modelOptions = useMemo(() => configs, [configs])

  const onGenerateCases = async (e: FormEvent) => {
    e.preventDefault()
    if (!projectId) return
    setBusy(true)
    setError(null)
    setNotice(null)
    try {
      const res = await api.generateCases(projectId, {
        requirements_text: requirements,
        count: 3,
        persist: true,
        config_id: caseConfigId || undefined,
      })
      setDrafts(res.drafts)
      setNotice(
        t('generate.casesNotice', {
          count: String(res.drafts.length),
          provider: res.provider,
          model: res.model,
          persisted: String(res.persisted.length),
        }),
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const restDiffRepos = useMemo(
    () =>
      linkedRepos.filter((r) =>
        ['github', 'gitlab', 'forgejo', 'gitea'].includes(r.provider),
      ),
    [linkedRepos],
  )

  const toggleExcludePath = (path: string) => {
    setExcludedPaths((cur) =>
      cur.includes(path) ? cur.filter((p) => p !== path) : [...cur, path],
    )
  }

  const onPreviewDiff = async () => {
    if (!projectId || !diffRepoId) return
    setBusy(true)
    setError(null)
    setNotice(null)
    try {
      const body =
        diffMode === 'pull'
          ? {
              repo_id: diffRepoId,
              pull_number: Number(pullNumber),
              exclude_paths: excludedPaths,
            }
          : {
              repo_id: diffRepoId,
              base_ref: baseRef,
              head_ref: headRef,
              exclude_paths: excludedPaths,
            }
      const preview = await api.previewDiff(projectId, body)
      setDiffPreview(preview)
      setNotice(
        t('generate.diffPreviewNotice', {
          files: String(preview.files.length),
          tokens: String(preview.selected_approx_tokens),
          budget: String(preview.token_budget),
        }),
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onGenerateFromDiff = async (e: FormEvent) => {
    e.preventDefault()
    if (!projectId || !diffRepoId) return
    setBusy(true)
    setError(null)
    setNotice(null)
    try {
      const body =
        diffMode === 'pull'
          ? {
              repo_id: diffRepoId,
              pull_number: Number(pullNumber),
              exclude_paths: excludedPaths,
              persist: true,
              count: 3,
              config_id: caseConfigId || undefined,
            }
          : {
              repo_id: diffRepoId,
              base_ref: baseRef,
              head_ref: headRef,
              exclude_paths: excludedPaths,
              persist: true,
              count: 3,
              config_id: caseConfigId || undefined,
            }
      const res = await api.generateCasesFromDiff(projectId, body)
      setDiffDrafts(res.drafts)
      setNotice(
        t('generate.diffCasesNotice', {
          count: String(res.drafts.length),
          provider: res.provider,
          model: res.model,
          persisted: String(res.persisted.length),
          ref: res.pull_number
            ? `#${res.pull_number}`
            : `${res.base_ref}…${res.head_ref}`,
          sha: res.head_sha ? res.head_sha.slice(0, 8) : '—',
        }),
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onGenerateCode = async (e: FormEvent) => {
    e.preventDefault()
    if (!projectId) return
    if (picked.length === 0) {
      setError(t('generate.needCases'))
      return
    }
    setBusy(true)
    setError(null)
    setNotice(null)
    try {
      const res = await api.generateCode(projectId, {
        case_ids: picked.map((c) => c.id),
        framework,
        config_id: codeConfigId || undefined,
        ...(repoId ? { repo_id: repoId } : {}),
      })
      setCode(res.code)
      setContextFiles(res.context_files ?? [])
      setNotice(
        t('generate.codeNotice', {
          provider: res.provider,
          model: res.model,
        }),
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const downloadCode = () => {
    const ext = framework === 'pytest' ? 'py' : 'ts'
    const blob = new Blob([code], { type: 'text/plain;charset=utf-8' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `generated-automation.${ext}`
    a.click()
    URL.revokeObjectURL(url)
  }

  if (!projectId) {
    return <p className="muted">{t('generate.selectProject')}</p>
  }

  if (!canLlm) {
    return (
      <div className="panel">
        <p className="muted">{t('generate.noPermission')}</p>
      </div>
    )
  }

  return (
    <div className="generate-page" {...testIdProps(TestIds.generatePage)}>
      <header className="panel-header">
        <h2>{t('generate.title')}</h2>
        <p className="muted">
          {t('generate.subtitle')}{' '}
          <DocsLink to="/docs/ai" testIdSuffix="generate" />
        </p>
      </header>

      {error && (
        <p className="error" {...testIdProps(TestIds.generateError)}>
          {error}
        </p>
      )}
      {notice && <p className="notice">{notice}</p>}

      <section className="admin-section">
        <h3>{t('generate.casesFromRequirements')}</h3>
        <form className="stack-form" onSubmit={(e) => void onGenerateCases(e)}>
          <label className="field">
            <span>{t('generate.model')}</span>
            <select
              value={caseConfigId}
              onChange={(e) => setCaseConfigId(e.target.value)}
              aria-label={t('generate.model')}
              {...testIdProps(TestIds.generateModelSelect)}
            >
              {modelOptions.length === 0 && (
                <option value="">{t('generate.mockFallback')}</option>
              )}
              {modelOptions.map((c) => (
                <option key={c.id} value={c.id}>
                  {configLabel(c, t)}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span>{t('generate.requirements')}</span>
            <textarea
              rows={6}
              value={requirements}
              onChange={(e) => setRequirements(e.target.value)}
              required
              placeholder={t('generate.requirementsPlaceholder')}
              {...testIdProps(TestIds.generateRequirementsInput)}
            />
          </label>
          <button
            type="submit"
            disabled={busy}
            {...testIdProps(TestIds.generateSubmitButton)}
          >
            {t('generate.generateDrafts')}
          </button>
        </form>
        {drafts.length > 0 && (
          <ul className="plain-list">
            {drafts.map((d) => (
              <li key={d.key}>
                <strong>{d.key}</strong> — {d.title}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section
        className="admin-section"
        {...testIdProps(TestIds.generateDiffSection)}
      >
        <h3>{t('generate.casesFromDiff')}</h3>
        <p className="muted">{t('generate.casesFromDiffHelp')}</p>
        {restDiffRepos.length === 0 ? (
          <p className="muted">{t('generate.diffRepoNone')}</p>
        ) : (
          <form
            className="stack-form"
            onSubmit={(e) => void onGenerateFromDiff(e)}
          >
            <label className="field">
              <span>{t('generate.repoPicker')}</span>
              <select
                value={diffRepoId}
                onChange={(e) => {
                  setDiffRepoId(e.target.value)
                  setDiffPreview(null)
                  setExcludedPaths([])
                }}
                {...testIdProps(TestIds.generateDiffRepoSelect)}
              >
                {restDiffRepos.map((repo) => (
                  <option key={repo.id} value={repo.id}>
                    {repoOptionLabel(repo)}
                  </option>
                ))}
              </select>
            </label>
            <label className="field">
              <span>{t('generate.diffMode')}</span>
              <select
                value={diffMode}
                onChange={(e) => {
                  setDiffMode(e.target.value as 'pull' | 'branch')
                  setDiffPreview(null)
                }}
                {...testIdProps(TestIds.generateDiffModeSelect)}
              >
                <option value="pull">{t('generate.diffModePull')}</option>
                <option value="branch">{t('generate.diffModeBranch')}</option>
              </select>
            </label>
            {diffMode === 'pull' ? (
              <label className="field">
                <span>{t('generate.diffPull')}</span>
                <select
                  value={pullNumber}
                  onChange={(e) => {
                    setPullNumber(e.target.value)
                    setDiffPreview(null)
                  }}
                  required
                  {...testIdProps(TestIds.generateDiffPullSelect)}
                >
                  {pulls.length === 0 && (
                    <option value="">{t('generate.diffPullEmpty')}</option>
                  )}
                  {pulls.map((p) => (
                    <option key={p.number} value={p.number}>
                      #{p.number} {p.title} ({p.source_branch} →{' '}
                      {p.target_branch})
                    </option>
                  ))}
                </select>
              </label>
            ) : (
              <div className="form-row">
                <label className="field">
                  <span>{t('generate.diffBase')}</span>
                  <input
                    value={baseRef}
                    onChange={(e) => setBaseRef(e.target.value)}
                    required
                    {...testIdProps(TestIds.generateDiffBaseInput)}
                  />
                </label>
                <label className="field">
                  <span>{t('generate.diffHead')}</span>
                  <input
                    value={headRef}
                    onChange={(e) => setHeadRef(e.target.value)}
                    required
                    {...testIdProps(TestIds.generateDiffHeadInput)}
                  />
                </label>
              </div>
            )}
            <div className="form-row">
              <button
                type="button"
                className="btn secondary"
                disabled={
                  busy ||
                  !diffRepoId ||
                  (diffMode === 'pull' ? !pullNumber : !headRef.trim())
                }
                onClick={() => void onPreviewDiff()}
                {...testIdProps(TestIds.generateDiffPreviewButton)}
              >
                {t('generate.diffPreview')}
              </button>
              <button
                type="submit"
                disabled={
                  busy ||
                  !diffRepoId ||
                  (diffMode === 'pull' ? !pullNumber : !headRef.trim())
                }
                {...testIdProps(TestIds.generateDiffSubmitButton)}
              >
                {t('generate.diffGenerate')}
              </button>
            </div>
            {diffPreview && (
              <>
                <p
                  className="muted"
                  {...testIdProps(TestIds.generateDiffBudget)}
                >
                  {t('generate.diffBudget', {
                    selected: String(diffPreview.selected_approx_tokens),
                    total: String(diffPreview.total_approx_tokens),
                    budget: String(diffPreview.token_budget),
                    sha: diffPreview.head_sha
                      ? diffPreview.head_sha.slice(0, 8)
                      : '—',
                  })}
                </p>
                <ul
                  className="plain-list"
                  {...testIdProps(TestIds.generateDiffFileList)}
                >
                  {diffPreview.files.map((f: DiffFileEntry, index) => (
                    <li key={f.path}>
                      <label className="check-row">
                        <input
                          type="checkbox"
                          checked={!excludedPaths.includes(f.path)}
                          onChange={() => toggleExcludePath(f.path)}
                          {...listItemProps(TestIds.generateDiffFileCheckbox, {
                            entityId: f.path,
                            entityKey: f.path,
                            index,
                          })}
                        />
                        <code>{f.path}</code>
                        <span className="muted">
                          {' '}
                          · {f.status} · +{f.additions}/-{f.deletions} · ~
                          {f.approx_tokens} tok
                        </span>
                      </label>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </form>
        )}
        {diffDrafts.length > 0 && (
          <ul className="plain-list">
            {diffDrafts.map((d) => (
              <li key={d.key}>
                <strong>{d.key}</strong> — {d.title}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="admin-section">
        <h3>{t('generate.automationCode')}</h3>
        <form className="stack-form" onSubmit={(e) => void onGenerateCode(e)}>
          <label className="field">
            <span>{t('generate.model')}</span>
            <select
              value={codeConfigId}
              onChange={(e) => setCodeConfigId(e.target.value)}
              aria-label={t('generate.modelAutomation')}
              {...testIdProps(TestIds.generateCodeModelSelect)}
            >
              {modelOptions.length === 0 && (
                <option value="">{t('generate.mockFallback')}</option>
              )}
              {modelOptions.map((c) => (
                <option key={c.id} value={c.id}>
                  {configLabel(c, t)}
                </option>
              ))}
            </select>
          </label>
          <div
            className="generate-case-picker"
            {...testIdProps(TestIds.generateCasePicker)}
          >
            <span className="field-label">{t('generate.pickCases')}</span>
            <CasePicker
              projectId={projectId}
              selected={picked}
              onChange={setPicked}
            />
          </div>
          <label className="field">
            <span>{t('generate.framework')}</span>
            <select
              value={framework}
              onChange={(e) =>
                setFramework(e.target.value as 'playwright' | 'pytest')
              }
              {...testIdProps(TestIds.generateFrameworkSelect)}
            >
              <option value="playwright">Playwright</option>
              <option value="pytest">pytest</option>
            </select>
          </label>
          {linkedRepos.length > 1 ? (
            <label className="field">
              <span>{t('generate.repoPicker')}</span>
              <select
                value={repoId}
                onChange={(e) => setRepoId(e.target.value)}
                aria-label={t('generate.repoPicker')}
                {...testIdProps(TestIds.generateRepoSelect)}
              >
                {linkedRepos.map((repo) => (
                  <option key={repo.id} value={repo.id}>
                    {repoOptionLabel(repo)}
                  </option>
                ))}
              </select>
              <span className="muted" style={{ fontSize: '0.85rem' }}>
                {t('generate.repoPickerHint')}
              </span>
            </label>
          ) : linkedRepos.length === 0 ? (
            <p className="muted">{t('generate.repoNone')}</p>
          ) : null}
          <button
            type="submit"
            disabled={busy || picked.length === 0}
            {...testIdProps(TestIds.generateCodeSubmitButton)}
          >
            {t('generate.generateCode')}
          </button>
        </form>

        {contextFiles.length > 0 && (
          <div className="context-files">
            <h4>{t('generate.contextFiles')}</h4>
            <ul className="plain-list">
              {contextFiles.map((f) => (
                <li key={f.path}>
                  <code>{f.path}</code>
                  <span className="muted">
                    {' '}
                    · {f.language} · {f.reason} · {f.score.toFixed(2)}
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {code && (
          <div
            className="generated-code"
            {...testIdProps(TestIds.generateOutput)}
          >
            <div className="generated-code-actions">
              <button
                type="button"
                className="btn secondary"
                onClick={downloadCode}
                {...testIdProps(TestIds.generateCopyButton)}
              >
                {t('generate.download')}
              </button>
            </div>
            <pre className="code-block">{code}</pre>
          </div>
        )}
      </section>
    </div>
  )
}
