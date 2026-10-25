import { FormEvent, useEffect, useState } from 'react'
import {
  api,
  type ApiTokenPublic,
  type LlmConfig,
  type ProviderKindInfo,
} from '../api'

type Props = {
  projectId: string | null
}

export function Admin({ projectId }: Props) {
  const [providers, setProviders] = useState<ProviderKindInfo[]>([])
  const [configs, setConfigs] = useState<LlmConfig[]>([])
  const [tokens, setTokens] = useState<ApiTokenPublic[]>([])
  const [secretOnce, setSecretOnce] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const [provider, setProvider] = useState('mock')
  const [name, setName] = useState('Default mock')
  const [model, setModel] = useState('mock-1')
  const [baseUrl, setBaseUrl] = useState('')
  const [apiKey, setApiKey] = useState('')
  const [isDefault, setIsDefault] = useState(true)
  const [tokenName, setTokenName] = useState('mcp-local')

  const refresh = async () => {
    if (!projectId) return
    try {
      const [p, c, t] = await Promise.all([
        api.listLlmProviders(),
        api.listLlmConfigs(projectId),
        api.listTokens(),
      ])
      setProviders(p)
      setConfigs(c)
      setTokens(t)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  useEffect(() => {
    void refresh()
  }, [projectId])

  useEffect(() => {
    const meta = providers.find((p) => p.id === provider)
    if (meta?.default_base_url && !baseUrl) {
      setBaseUrl(meta.default_base_url)
    }
  }, [provider, providers, baseUrl])

  const onCreateConfig = async (e: FormEvent) => {
    e.preventDefault()
    if (!projectId) return
    setBusy(true)
    try {
      await api.createLlmConfig(projectId, {
        provider,
        name,
        model,
        base_url: baseUrl || undefined,
        api_key: apiKey || undefined,
        is_default: isDefault,
      })
      setApiKey('')
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onCreateToken = async (e: FormEvent) => {
    e.preventDefault()
    setBusy(true)
    try {
      const res = await api.createToken(tokenName)
      setSecretOnce(res.secret)
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  if (!projectId) {
    return <p className="muted">Select a project to configure LLM providers.</p>
  }

  return (
    <div className="admin-page">
      <header className="panel-header">
        <h2>Admin</h2>
        <p className="muted">
          LLM providers (keys encrypted at rest) and API tokens for MCP.
        </p>
      </header>

      {error && <p className="error">{error}</p>}

      <section className="admin-section">
        <h3>LLM providers</h3>
        <form className="stack-form" onSubmit={onCreateConfig}>
          <label>
            Provider
            <select value={provider} onChange={(e) => setProvider(e.target.value)}>
              {providers.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            Name
            <input value={name} onChange={(e) => setName(e.target.value)} required />
          </label>
          <label>
            Model
            <input value={model} onChange={(e) => setModel(e.target.value)} />
          </label>
          <label>
            Base URL
            <input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="optional"
            />
          </label>
          <label>
            API key
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="stored encrypted — never returned"
              autoComplete="off"
            />
          </label>
          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={isDefault}
              onChange={(e) => setIsDefault(e.target.checked)}
            />
            Default for this project
          </label>
          <button type="submit" disabled={busy}>
            Save provider
          </button>
        </form>

        <ul className="plain-list">
          {configs.map((c) => (
            <li key={c.id}>
              <strong>{c.name}</strong> · {c.provider}
              {c.model ? ` · ${c.model}` : ''}
              {c.is_default ? ' · default' : ''}
              {c.has_api_key ? ' · key set' : ''}
              <button
                type="button"
                className="linkish"
                onClick={() => void api.deleteLlmConfig(projectId, c.id).then(refresh)}
              >
                Delete
              </button>
            </li>
          ))}
          {configs.length === 0 && (
            <li className="muted">No providers yet — mock is used until you add one.</li>
          )}
        </ul>
      </section>

      <section className="admin-section">
        <h3>API tokens (MCP)</h3>
        <form className="stack-form" onSubmit={onCreateToken}>
          <label>
            Token name
            <input
              value={tokenName}
              onChange={(e) => setTokenName(e.target.value)}
              required
            />
          </label>
          <button type="submit" disabled={busy}>
            Create token
          </button>
        </form>
        {secretOnce && (
          <p className="notice">
            Copy now (shown once): <code>{secretOnce}</code>
          </p>
        )}
        <ul className="plain-list">
          {tokens.map((t) => (
            <li key={t.id}>
              <strong>{t.name}</strong> · {t.token_prefix}…
              {t.revoked_at ? ' · revoked' : ''}
              {!t.revoked_at && (
                <button
                  type="button"
                  className="linkish"
                  onClick={() => void api.revokeToken(t.id).then(refresh)}
                >
                  Revoke
                </button>
              )}
            </li>
          ))}
        </ul>
      </section>
    </div>
  )
}
