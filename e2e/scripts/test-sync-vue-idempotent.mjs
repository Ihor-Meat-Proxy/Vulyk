#!/usr/bin/env node
/**
 * Mock-API regression for sync-vue-project.mjs idempotency (S1).
 *
 * Spins a tiny in-memory Vulyk-shaped API (paginated {items}, PUT-only case
 * update), runs the sync twice, and asserts created=0 on the second pass.
 * No Docker / live stack required — runs in CI via e2e typecheck job hooks or
 * `node e2e/scripts/test-sync-vue-idempotent.mjs`.
 */
import http from 'node:http'
import { spawn } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { randomUUID } from 'node:crypto'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const ROOT = path.resolve(__dirname, '../..')
const SYNC = path.join(ROOT, 'e2e/scripts/sync-vue-project.mjs')
const MAP_OUT = path.join(ROOT, 'e2e/vulyk-vue-case-map.json')

function die(msg) {
  console.error(`error: ${msg}`)
  process.exit(1)
}

function readJson(req) {
  return new Promise((resolve, reject) => {
    const chunks = []
    req.on('data', (c) => chunks.push(c))
    req.on('end', () => {
      try {
        const raw = Buffer.concat(chunks).toString('utf8')
        resolve(raw ? JSON.parse(raw) : null)
      } catch (err) {
        reject(err)
      }
    })
    req.on('error', reject)
  })
}

function send(res, status, body) {
  const payload = body === undefined ? '' : JSON.stringify(body)
  res.writeHead(status, {
    'Content-Type': 'application/json',
    'Content-Length': Buffer.byteLength(payload),
  })
  res.end(payload)
}

function page(items, limit, offset) {
  const slice = items.slice(offset, offset + limit)
  return { items: slice, total: items.length, limit, offset }
}

function startMock() {
  const state = {
    project: {
      id: randomUUID(),
      key: 'VUE',
      name: 'Vulyk UI E2E',
      case_key_prefix: 'VUE',
    },
    suites: [],
    cases: [],
    nextCaseNum: 1,
    creates: 0,
    puts: 0,
    patches: 0,
  }

  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url || '/', 'http://127.0.0.1')
    const method = req.method || 'GET'
    try {
      if (method === 'POST' && url.pathname === '/api/v1/auth/login') {
        send(res, 200, { token: 'mock-token' })
        return
      }
      if (method === 'GET' && url.pathname === '/api/v1/projects') {
        const limit = Number(url.searchParams.get('limit') || 50)
        const offset = Number(url.searchParams.get('offset') || 0)
        // First page only includes the project when offset=0 — force clients
        // to honor pagination (default page size historically 50).
        send(res, 200, page([state.project], limit, offset))
        return
      }
      if (
        method === 'GET' &&
        url.pathname === `/api/v1/projects/${state.project.id}/suites`
      ) {
        const limit = Number(url.searchParams.get('limit') || 50)
        const offset = Number(url.searchParams.get('offset') || 0)
        send(res, 200, page(state.suites, limit, offset))
        return
      }
      if (
        method === 'POST' &&
        url.pathname === `/api/v1/projects/${state.project.id}/suites`
      ) {
        const body = await readJson(req)
        const suite = { id: randomUUID(), name: body.name, sort_order: 0 }
        state.suites.push(suite)
        send(res, 201, suite)
        return
      }
      if (
        method === 'GET' &&
        url.pathname === `/api/v1/projects/${state.project.id}/cases`
      ) {
        const limit = Number(url.searchParams.get('limit') || 50)
        const offset = Number(url.searchParams.get('offset') || 0)
        // Return thin list items (as the real API does).
        const items = state.cases.map((c) => ({
          id: c.id,
          key: c.key,
          title: c.title,
          external_key: c.external_key,
          suite_id: c.suite_id,
        }))
        send(res, 200, page(items, limit, offset))
        return
      }
      const caseDetail = url.pathname.match(
        new RegExp(
          `^/api/v1/projects/${state.project.id}/cases/([0-9a-f-]+)$`,
          'i',
        ),
      )
      if (caseDetail) {
        const id = caseDetail[1]
        const found = state.cases.find((c) => c.id === id)
        if (!found) {
          send(res, 404, { error: { code: 'not_found', message: 'case' } })
          return
        }
        if (method === 'GET') {
          send(res, 200, { case: found })
          return
        }
        if (method === 'PATCH') {
          state.patches += 1
          send(res, 405, { error: { code: 'method_not_allowed', message: 'use PUT' } })
          return
        }
        if (method === 'PUT') {
          const body = await readJson(req)
          Object.assign(found, {
            title: body.title ?? found.title,
            suite_id: body.suite_id ?? found.suite_id,
            description: body.description ?? found.description,
            precondition: body.precondition ?? found.precondition,
            priority: body.priority ?? found.priority,
            status: body.status ?? found.status,
            steps: body.steps ?? found.steps,
            external_key: body.external_key ?? found.external_key,
            version: (found.version || 1) + 1,
          })
          state.puts += 1
          send(res, 200, { case: found })
          return
        }
        if (method === 'DELETE') {
          state.cases = state.cases.filter((c) => c.id !== id)
          send(res, 204)
          return
        }
      }
      if (
        method === 'POST' &&
        url.pathname === `/api/v1/projects/${state.project.id}/cases`
      ) {
        const body = await readJson(req)
        const c = {
          id: randomUUID(),
          key: `VUE-${state.nextCaseNum++}`,
          title: body.title,
          suite_id: body.suite_id,
          description: body.description || '',
          precondition: body.precondition || '',
          priority: body.priority || 'high',
          status: body.status || 'ready',
          steps: body.steps || [],
          external_key: body.external_key,
          version: 1,
        }
        state.cases.push(c)
        state.creates += 1
        send(res, 201, { case: c })
        return
      }
      send(res, 404, { error: { code: 'not_found', message: url.pathname } })
    } catch (err) {
      send(res, 500, { error: { message: String(err) } })
    }
  })

  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address()
      resolve({ server, port, state })
    })
  })
}

function runSync(port) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [SYNC], {
      cwd: ROOT,
      env: {
        ...process.env,
        VULYK_API_URL: `http://127.0.0.1:${port}`,
        VULYK_ADMIN_EMAIL: 'admin@vulyk.local',
        VULYK_ADMIN_PASSWORD: 'admin12345678901',
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    let out = ''
    let err = ''
    child.stdout.on('data', (d) => {
      out += d
    })
    child.stderr.on('data', (d) => {
      err += d
    })
    child.on('close', (code) => {
      if (code !== 0) {
        reject(new Error(`sync exited ${code}: ${err || out}`))
        return
      }
      resolve(out)
    })
  })
}

function parseCreated(log) {
  const m = log.match(/created=(\d+)/)
  return m ? Number(m[1]) : null
}

async function main() {
  const prevMap = fs.existsSync(MAP_OUT)
    ? fs.readFileSync(MAP_OUT, 'utf8')
    : null
  const { server, port, state } = await startMock()
  try {
    const first = await runSync(port)
    const created1 = parseCreated(first)
    if (created1 === null || created1 < 1) {
      die(`first sync expected creates > 0, got log:\n${first}`)
    }
    if (state.patches !== 0) {
      die(`PATCH must not be used (patches=${state.patches})`)
    }
    // Force pagination stress: if client only read first empty page it would
    // re-create everything. Seed is already loaded; second run must update.
    const beforeCreates = state.creates
    const beforePuts = state.puts
    const second = await runSync(port)
    const created2 = parseCreated(second)
    if (created2 !== 0) {
      die(`second sync must create 0 cases (got created=${created2})\n${second}`)
    }
    if (state.creates !== beforeCreates) {
      die(`second sync mutated create counter (${beforeCreates} -> ${state.creates})`)
    }
    if (state.puts <= beforePuts) {
      die('second sync should PUT-update existing cases')
    }
    if (state.patches !== 0) {
      die(`PATCH must not be used (patches=${state.patches})`)
    }
    const map = JSON.parse(fs.readFileSync(MAP_OUT, 'utf8'))
    if (map.created !== 0) {
      die(`map.created must be 0 after second sync (got ${map.created})`)
    }
    console.log(
      `OK: sync-vue idempotent (first created=${created1}, second created=0, puts=${state.puts}, cases=${state.cases.length})`,
    )
  } finally {
    server.close()
    if (prevMap === null) {
      try {
        fs.unlinkSync(MAP_OUT)
      } catch {
        /* ignore */
      }
    } else {
      fs.writeFileSync(MAP_OUT, prevMap)
    }
  }
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
