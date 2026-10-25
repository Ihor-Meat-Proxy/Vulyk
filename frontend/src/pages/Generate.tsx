import { FormEvent, useState } from 'react'
import { api, type GeneratedCaseDraft } from '../api'

type Props = {
  projectId: string | null
}

export function Generate({ projectId }: Props) {
  const [requirements, setRequirements] = useState('')
  const [drafts, setDrafts] = useState<GeneratedCaseDraft[]>([])
  const [code, setCode] = useState('')
  const [framework, setFramework] = useState<'playwright' | 'pytest'>('playwright')
  const [caseIds, setCaseIds] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)

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
      })
      setDrafts(res.drafts)
      setNotice(
        `Generated ${res.drafts.length} draft(s) via ${res.provider}/${res.model}; persisted ${res.persisted.length}.`,
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
    const ids = caseIds
      .split(/[\s,]+/)
      .map((s) => s.trim())
      .filter(Boolean)
    if (ids.length === 0) {
      setError('Provide at least one case id')
      return
    }
    setBusy(true)
    setError(null)
    try {
      const res = await api.generateCode(projectId, {
        case_ids: ids,
        framework,
      })
      setCode(res.code)
      setNotice(`Code from ${res.provider}/${res.model}`)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  if (!projectId) {
    return <p className="muted">Select a project to generate cases or code.</p>
  }

  return (
    <div className="generate-page">
      <header className="panel-header">
        <h2>Generate</h2>
        <p className="muted">
          Turn requirements into case drafts, or cases into Playwright/pytest
          stubs.
        </p>
      </header>

      {error && <p className="error">{error}</p>}
      {notice && <p className="notice">{notice}</p>}

      <section className="admin-section">
        <h3>Cases from requirements</h3>
        <form className="stack-form" onSubmit={onGenerateCases}>
          <label>
            Requirements
            <textarea
              rows={6}
              value={requirements}
              onChange={(e) => setRequirements(e.target.value)}
              required
              placeholder="Describe the behaviour to cover…"
            />
          </label>
          <button type="submit" disabled={busy}>
            Generate &amp; save drafts
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
        <h3>Automation code</h3>
        <form className="stack-form" onSubmit={onGenerateCode}>
          <label>
            Case IDs
            <input
              value={caseIds}
              onChange={(e) => setCaseIds(e.target.value)}
              placeholder="uuid, uuid…"
              required
            />
          </label>
          <label>
            Framework
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
          <button type="submit" disabled={busy}>
            Generate code
          </button>
        </form>
        {code && <pre className="code-block">{code}</pre>}
      </section>
    </div>
  )
}
