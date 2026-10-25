import { spawn, type ChildProcess } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const PID_FILE = path.join(__dirname, '.mock-oidc.pid')
const PORT = Number(process.env.E2E_OIDC_PORT ?? 9090)

async function waitForOidc(port: number, timeoutMs = 15_000) {
  const started = Date.now()
  while (Date.now() - started < timeoutMs) {
    try {
      const res = await fetch(
        `http://127.0.0.1:${port}/.well-known/openid-configuration`,
      )
      if (res.ok) return
    } catch {
      // retry
    }
    await new Promise((r) => setTimeout(r, 200))
  }
  throw new Error(`Mock OIDC did not become ready on port ${port}`)
}

export default async function globalSetup() {
  if (process.env.E2E_SKIP_MOCK_OIDC === '1') return

  const serverPath = path.join(__dirname, 'mock-oidc', 'server.mjs')
  const child: ChildProcess = spawn(process.execPath, [serverPath], {
    env: { ...process.env, OIDC_PORT: String(PORT) },
    stdio: ['ignore', 'pipe', 'pipe'],
    detached: true,
  })

  if (!child.pid) {
    throw new Error('Failed to start mock OIDC server')
  }

  fs.writeFileSync(PID_FILE, String(child.pid), 'utf8')
  child.unref()

  try {
    await waitForOidc(PORT)
    console.log(`[e2e] mock OIDC ready on http://127.0.0.1:${PORT}`)
  } catch (err) {
    try {
      process.kill(child.pid, 'SIGTERM')
    } catch {
      // ignore
    }
    throw err
  }
}
