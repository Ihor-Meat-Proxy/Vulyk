import { FormEvent, useEffect, useMemo, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import {
  api,
  type AutomationRepo,
  type ContextFileRef,
  type GeneratedCaseDraft,
  type LlmConfig,
  type TestCase,
} from '../api'
import { CasePicker, type PickedCase } from '../components/CasePicker'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { useI18n } from '../i18n'

type Props = {
  projectId: string | null
}

function configLabel(c: LlmConfig): string {
  const scope = c.project_id == null ? 'global' : 'project'
  const def = c.is_default ? ' · default' : ''
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
        setRepoId((cur) => {
          if (cur && list.some((r) => r.id === cur)) return cur
          return list[0]?.id ?? ''
        })
      })
      .catch(() => {
        setLinkedRepos([])
        setRepoId('')
      })
  }, [projectId])

  // Prefill from Cases page: ?cases=id1,id2
  useEffect(() => {
    if (!projectId) return
    const raw = searchParams.get('cases')
    if (!raw) return
    const ids = raw.split(',').map((s) => s.trim()).filter(Boolean)
    if (ids.length === 0) return
    let cancelled = false
    void (async () => {
      try {
        const page = await api.listCases(projectId, { limit: 500 })
        if (cancelled) return
        const map = new Map(page.items.map((c: TestCase) => [c.id, c]))
        const next: PickedCase[] = []
        for (const id of ids) {
          const c = map.get(id)
          if (c) next.push({ id: c.id, key: c.key, title: c.title })
        }
        if (next.length) setPicked(next)
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
    <div className="generate-page">
      <header className="panel-header">
        <h2>{t('generate.title')}</h2>
        <p className="muted">{t('generate.subtitle')}</p>
      </header>

      {error && <p className="error">{error}</p>}
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
          <label className="field">
            <span>{t('generate.requirements')}</span>
            <textarea
              rows={6}
              value={requirements}
              onChange={(e) => setRequirements(e.target.value)}
              required
              placeholder={t('generate.requirementsPlaceholder')}
            />
          </label>
          <button type="submit" disabled={busy}>
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

      <section className="admin-section">
        <h3>{t('generate.automationCode')}</h3>
        <form className="stack-form" onSubmit={(e) => void onGenerateCode(e)}>
          <label className="field">
            <span>{t('generate.model')}</span>
            <select
              value={codeConfigId}
              onChange={(e) => setCodeConfigId(e.target.value)}
              aria-label={t('generate.modelAutomation')}
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
          <div className="field">
            <span>{t('generate.pickCases')}</span>
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
          <button type="submit" disabled={busy || picked.length === 0}>
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
          <div className="generated-code">
            <div className="generated-code-actions">
              <button
                type="button"
                className="btn secondary"
                onClick={downloadCode}
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
