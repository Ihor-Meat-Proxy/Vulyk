import { useState, type FormEvent } from 'react'
import { useAuth } from '../auth'

export function Login() {
  const { status, login, bootstrap } = useAuth()
  const [email, setEmail] = useState('admin@vulyk.local')
  const [password, setPassword] = useState('')
  const [displayName, setDisplayName] = useState('Admin')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const isBootstrap = Boolean(status?.bootstrap_available)

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      if (isBootstrap) {
        await bootstrap(email, password, displayName)
      } else {
        await login(email, password)
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="login-shell">
      <form className="login-panel" onSubmit={onSubmit}>
        <div className="brand">
          <span className="brand-mark">Vulyk</span>
          <span className="brand-tag">
            {isBootstrap ? 'Create the first admin' : 'Sign in'}
          </span>
        </div>
        <p className="muted">
          {isBootstrap
            ? 'No users yet. Bootstrap a system administrator to unlock the admin panel and API tokens.'
            : 'Use your local account. Personal API tokens for CI/MCP are managed under Admin → Tokens after sign-in.'}
        </p>
        {isBootstrap && (
          <label className="field">
            <span>Display name</span>
            <input
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              required
              autoComplete="name"
            />
          </label>
        )}
        <label className="field">
          <span>Email</span>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            autoComplete="username"
          />
        </label>
        <label className="field">
          <span>Password</span>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            minLength={8}
            autoComplete={isBootstrap ? 'new-password' : 'current-password'}
          />
        </label>
        {error && <p className="error">{error}</p>}
        <button className="btn" type="submit" disabled={busy}>
          {busy ? 'Working…' : isBootstrap ? 'Create admin' : 'Sign in'}
        </button>
      </form>
    </div>
  )
}
