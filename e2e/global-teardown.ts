import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const PID_FILE = path.join(__dirname, '.mock-oidc.pid')

export default async function globalTeardown() {
  if (!fs.existsSync(PID_FILE)) return
  const pid = Number(fs.readFileSync(PID_FILE, 'utf8').trim())
  fs.unlinkSync(PID_FILE)
  if (!Number.isFinite(pid) || pid <= 0) return
  try {
    process.kill(pid, 'SIGTERM')
  } catch {
    // already exited
  }
}
