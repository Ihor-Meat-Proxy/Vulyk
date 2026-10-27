/**
 * Playwright reporter that posts results to a Vulyk project via the CI ingest API.
 *
 * Env (all required to enable; otherwise this reporter is a silent no-op):
 *   VULYK_URL          — API base, e.g. http://127.0.0.1:8080 (alias: VULYK_API_URL)
 *   VULYK_CI_TOKEN     — project CI token `vulyk_ci_…` (alias: VULYK_API_TOKEN)
 *   VULYK_PROJECT      — project key or UUID (aliases: VULYK_PROJECT_KEY, VULYK_PROJECT_ID)
 *   VULYK_RUN_NAME     — optional suite / run display name
 *   VULYK_RUN_ID       — optional existing Vulyk run id (from workflow_dispatch /
 *                        Run in CI). When set, skips open and attaches results
 *                        to that run instead of creating a new one.
 *
 * Case keys are read from each test's Playwright annotation
 * `{ type: 'vulyk', description: 'VQA-123' }` (or tag `@VQA-123`).
 *
 * Lifecycle: open (unless VULYK_RUN_ID) → start (in_progress) → POST results → close.
 */
import type {
  FullConfig,
  FullResult,
  Reporter,
  Suite,
  TestCase,
  TestResult,
} from '@playwright/test/reporter'

type PendingResult = {
  case_key: string
  status: string
  message?: string
  duration_ms?: number
  external_id: string
}

function env(...names: string[]): string {
  for (const n of names) {
    const v = process.env[n]?.trim()
    if (v) return v
  }
  return ''
}

function caseKeyFromTest(test: TestCase): string | null {
  for (const a of test.annotations) {
    if (a.type === 'vulyk' && a.description?.trim()) {
      return a.description.trim().toUpperCase()
    }
  }
  for (const tag of test.tags) {
    const m = /^@?(VQA-\d+)$/i.exec(tag)
    if (m) return m[1].toUpperCase()
  }
  // Fallback: VQA-N in the title
  const m = /\b(VQA-\d+)\b/i.exec(test.title)
  return m ? m[1].toUpperCase() : null
}

function mapStatus(result: TestResult): string {
  switch (result.status) {
    case 'passed':
      return 'passed'
    case 'failed':
    case 'timedOut':
    case 'interrupted':
      return 'failed'
    case 'skipped':
      return 'skipped'
    default:
      return 'failed'
  }
}

export default class VulykReporter implements Reporter {
  private enabled = false
  private baseUrl = ''
  private token = ''
  private projectRef = ''
  private runName = ''
  private projectId = ''
  private runId = ''
  private pending: PendingResult[] = []
  private warned = false

  printsToStdio(): boolean {
    return false
  }

  onBegin(_config: FullConfig, _suite: Suite): void {
    this.baseUrl = env('VULYK_URL', 'VULYK_API_URL').replace(/\/$/, '')
    this.token = env('VULYK_CI_TOKEN', 'VULYK_API_TOKEN')
    this.projectRef = env('VULYK_PROJECT', 'VULYK_PROJECT_KEY', 'VULYK_PROJECT_ID')
    this.runName = env('VULYK_RUN_NAME') || 'Playwright e2e'
    this.enabled = Boolean(this.baseUrl && this.token && this.projectRef)
    if (!this.enabled) {
      // Intentional no-op when unset so CI / local runs stay unaffected.
      return
    }
  }

  onTestEnd(test: TestCase, result: TestResult): void {
    if (!this.enabled) return
    // Prefer the last retry outcome only (Playwright calls onTestEnd per attempt).
    if (result.retry < test.results.length - 1) return
    const key = caseKeyFromTest(test)
    if (!key) {
      if (!this.warned) {
        this.warned = true
        console.warn(
          '[vulyk-reporter] test(s) missing vulyk annotation; results for those tests are skipped',
        )
      }
      return
    }
    const rel =
      test.location.file.split(/e2e\/tests\//).pop() || test.location.file
    const message =
      result.status === 'passed'
        ? undefined
        : result.errors.map((e) => e.message || String(e)).join('\n').slice(0, 4000)
    this.pending.push({
      case_key: key,
      status: mapStatus(result),
      message,
      duration_ms: Math.round(result.duration),
      external_id: `${rel}::${test.title}`,
    })
  }

  async onEnd(result: FullResult): Promise<void> {
    if (!this.enabled) return
    try {
      await this.publish(result)
    } catch (err) {
      console.error('[vulyk-reporter] failed to publish results:', err)
    }
  }

  private async api<T>(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<T> {
    const res = await fetch(`${this.baseUrl}${path}`, {
      method,
      headers: {
        Authorization: `Bearer ${this.token}`,
        Accept: 'application/json',
        ...(body !== undefined ? { 'Content-Type': 'application/json' } : {}),
      },
      body: body !== undefined ? JSON.stringify(body) : undefined,
    })
    const text = await res.text()
    if (!res.ok) {
      throw new Error(`${method} ${path} → ${res.status}: ${text.slice(0, 500)}`)
    }
    return text ? (JSON.parse(text) as T) : ({} as T)
  }

  private async resolveProjectId(): Promise<string> {
    const ref = this.projectRef
    if (/^[0-9a-f-]{36}$/i.test(ref)) return ref
    const projects = await this.api<Array<{ id: string; key: string }>>(
      'GET',
      '/api/v1/projects',
    )
    const hit = projects.find((p) => p.key.toUpperCase() === ref.toUpperCase())
    if (!hit) {
      throw new Error(`project key ${ref} not found`)
    }
    return hit.id
  }

  private async publish(result: FullResult): Promise<void> {
    this.projectId = await this.resolveProjectId()
    const pipelineUrl = process.env.GITHUB_SERVER_URL
      ? `${process.env.GITHUB_SERVER_URL}/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}`
      : undefined
    const existingRunId = env('VULYK_RUN_ID')

    if (existingRunId) {
      this.runId = existingRunId
      console.log(`[vulyk-reporter] attaching to existing run ${this.runId}`)
    } else {
      const externalRef =
        env('VULYK_EXTERNAL_REF') ||
        `playwright:${process.env.GITHUB_RUN_ID || 'local'}:${Date.now()}`

      const opened = await this.api<{
        run: { id: string; status: string }
      }>('POST', `/api/v1/projects/${this.projectId}/ci/runs`, {
        provider: process.env.GITHUB_ACTIONS ? 'github' : 'other',
        suite_name: this.runName,
        name: this.runName,
        external_ref: externalRef,
        branch: process.env.GITHUB_REF_NAME || process.env.CI_COMMIT_REF_NAME,
        commit: process.env.GITHUB_SHA || process.env.CI_COMMIT_SHA,
        pipeline_url: pipelineUrl,
        start: false,
      })
      this.runId = opened.run.id
      console.log(`[vulyk-reporter] opened run ${this.runId} (${opened.run.status})`)
    }

    await this.api(
      'POST',
      `/api/v1/projects/${this.projectId}/ci/runs/${this.runId}/start`,
      {
        pipeline_url: pipelineUrl,
        branch: process.env.GITHUB_REF_NAME || process.env.CI_COMMIT_REF_NAME,
        commit: process.env.GITHUB_SHA || process.env.CI_COMMIT_SHA,
      },
    )
    console.log(`[vulyk-reporter] run ${this.runId} → in_progress`)

    // Dedupe by case_key (last result wins) for serial retries / shared keys.
    const byKey = new Map<string, PendingResult>()
    for (const row of this.pending) byKey.set(row.case_key, row)
    const results = [...byKey.values()]

    if (results.length > 0) {
      // Chunk to keep payloads modest
      const CHUNK = 200
      for (let i = 0; i < results.length; i += CHUNK) {
        const slice = results.slice(i, i + CHUNK)
        const resp = await this.api<{
          inserted: number
          unmatched_case_keys: string[]
        }>('POST', `/api/v1/projects/${this.projectId}/ci/runs/${this.runId}/results`, {
          results: slice,
        })
        if (resp.unmatched_case_keys?.length) {
          console.warn(
            '[vulyk-reporter] unmatched case keys:',
            resp.unmatched_case_keys.join(', '),
          )
        }
        console.log(
          `[vulyk-reporter] posted ${slice.length} results (inserted=${resp.inserted})`,
        )
      }
    } else {
      console.warn('[vulyk-reporter] no annotated results to post')
    }

    await this.api(
      'POST',
      `/api/v1/projects/${this.projectId}/ci/runs/${this.runId}/close`,
      {
        cancelled: result.status === 'interrupted',
        message: `playwright ${result.status}; ${results.length} result(s)`,
      },
    )
    console.log(`[vulyk-reporter] closed run ${this.runId}`)
  }
}
