import { useEffect, useState } from 'react'
import { TestIds, testIdProps } from '../testing'

const API_BASE = import.meta.env.VITE_API_URL ?? ''
const FALLBACK = import.meta.env.VITE_APP_VERSION ?? '0.1.0'

type HealthPayload = {
  status?: string
  version?: string
}

/** Small footer mark showing API semver from `/health` (falls back to package version). */
export function AppVersion() {
  const [version, setVersion] = useState<string>(FALLBACK)

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const res = await fetch(`${API_BASE}/health`, { credentials: 'include' })
        if (!res.ok) return
        const body = (await res.json()) as HealthPayload
        if (!cancelled && body.version) {
          setVersion(body.version)
        }
      } catch {
        // Keep package.json fallback when API is unreachable.
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <p
      className="app-version muted"
      title="Vulyk version"
      {...testIdProps(TestIds.appVersion)}
    >
      Vulyk {version}
    </p>
  )
}
