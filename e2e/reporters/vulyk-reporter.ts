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
 *   VULYK_REPORTER_DRY_RUN — when `1`/`true`/`yes`, log open/start/results/close
 *                        without POSTing to the API (local DX / CI dry-run).
 *
 * Case keys are read from each test's Playwright annotation
 * `{ type: 'vulyk', description: 'VQA-123' }` (or tag `@VQA-123`).
 *
 * Lifecycle (live): onBegin open+start (in_progress) → onTestEnd streams
 * batched results → onEnd flushes remaining and closes. Interrupted suites
 * close with cancelled=true (run status `cancelled`). A hard-killed runner
 * that never reaches onEnd leaves the run visible as `in_progress`.
 */
import type {
  FullConfig,
  FullResult,
  Reporter,
  Suite,
  TestCase,
  TestResult,
} from '@playwright/test/reporter'
import {
  RESULT_BATCH_SIZE,
  RESULT_FLUSH_MS,
  closeCancelled,
  dedupePendingResults,
  dryRunApiResponse,
  formatDryRunLog,
  formatMissingAnnotationWarning,
  formatUnmatchedKeysWarning,
  isReporterDryRun,
  mapTestStatus,
  shouldFlushNow,
  takeFlushBatch,
  type PendingResult,
} from './vulyk-reporter-helpers'

function env(...names: string[]): string {
  for (const n of names) {
    const v = process.env[n]?.trim()
    if (v) return v
  }
  return ''
}

/** Detect CI provider for open-run metadata (Forgejo/Gitea before generic GitHub). */
function detectCiProvider(): string {
  const explicit = env('VULYK_CI_PROVIDER')
  if (explicit) return explicit.toLowerCase()
  if (process.env.FORGEJO_ACTIONS || process.env.FORGEJO) return 'forgejo'
  if (process.env.GITEA_ACTIONS) return 'gitea'
  if (process.env.GITHUB_ACTIONS) {
    const server = process.env.GITHUB_SERVER_URL || ''
    if (/forgejo/i.test(server)) return 'forgejo'
    if (/gitea/i.test(server)) return 'gitea'
    return 'github'
  }
  if (process.env.GITLAB_CI) return 'gitlab'
  if (process.env.JENKINS_URL || process.env.BUILD_URL) return 'jenkins'
  return 'other'
}

function caseKeyFromTest(test: TestCase): string | null {
  const prefix = env('VULYK_CASE_KEY_PREFIX').toUpperCase() // e.g. VUE or VQA
  const keys: string[] = []
  for (const a of test.annotations) {
    if (a.type === 'vulyk' && a.description?.trim()) {
      keys.push(a.description.trim().toUpperCase())
    }
  }
  for (const tag of test.tags) {
    const m = /^@?([A-Z][A-Z0-9]{1,7}-\d+)$/i.exec(tag)
    if (m) keys.push(m[1].toUpperCase())
  }
  // Fallback: KEY-N in the title
  const titleHit = /\b([A-Z][A-Z0-9]{1,7}-\d+)\b/i.exec(test.title)
  if (titleHit) keys.push(titleHit[1].toUpperCase())

  if (prefix) {
    const hit = keys.find((k) => k.startsWith(`${prefix}-`))
    if (hit) return hit
  }
  return keys[0] ?? null
}

export default class VulykReporter implements Reporter {
  private enabled = false
  private dryRun = false
  private baseUrl = ''
  private token = ''
  private projectRef = ''
  private runName = ''
  private projectId = ''
  private runId = ''
  private pending: PendingResult[] = []
  private missingAnnotationTitles: string[] = []
  private missingAnnotationWarned = false
  private unmatchedWarned = new Set<string>()
  private pipelineUrl: string | undefined
  private plannedTotal = 0
  private caseKeyPrefix = ''
  private openReady: Promise<void> = Promise.resolve()
  private flushChain: Promise<void> = Promise.resolve()
  private flushTimer: ReturnType<typeof setTimeout> | null = null

  printsToStdio(): boolean {
    return false
  }

  async onBegin(_config: FullConfig, suite: Suite): Promise<void> {
    this.baseUrl = env('VULYK_URL', 'VULYK_API_URL').replace(/\/$/, '')
    this.token = env('VULYK_CI_TOKEN', 'VULYK_API_TOKEN')
    this.projectRef = env('VULYK_PROJECT', 'VULYK_PROJECT_KEY', 'VULYK_PROJECT_ID')
    this.runName = env('VULYK_RUN_NAME') || 'Playwright e2e'
    this.caseKeyPrefix = env('VULYK_CASE_KEY_PREFIX').toUpperCase()
    this.plannedTotal = suite.allTests().length
    this.dryRun = isReporterDryRun()
    this.enabled = Boolean(this.baseUrl && this.token && this.projectRef)
    if (!this.enabled) {
      // Intentional no-op when unset so CI / local runs stay unaffected.
      return
    }
    if (this.dryRun) {
      console.log(
        '[vulyk-reporter] dry-run enabled — logging open/start/results/close without POSTing',
      )
    }
    this.pipelineUrl = process.env.GITHUB_SERVER_URL
      ? `${process.env.GITHUB_SERVER_URL}/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}`
      : undefined

    this.openReady = this.openAndStart().catch((err) => {
      console.error('[vulyk-reporter] failed to open/start run:', err)
      this.enabled = false
    })
    await this.openReady
  }

  onTestEnd(test: TestCase, result: TestResult): void {
    if (!this.enabled) return
    // Prefer the last retry outcome only (Playwright calls onTestEnd per attempt).
    if (result.retry < test.results.length - 1) return
    const key = caseKeyFromTest(test)
    if (!key) {
      this.missingAnnotationTitles.push(test.title)
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
      status: mapTestStatus(result.status),
      message,
      duration_ms: Math.round(result.duration),
      external_id: `${rel}::${test.title}`,
    })
    this.scheduleFlush()
  }

  async onEnd(result: FullResult): Promise<void> {
    if (!this.enabled && !this.runId) {
      this.warnMissingAnnotations()
      return
    }
    try {
      await this.openReady
      if (!this.enabled || !this.runId) {
        this.warnMissingAnnotations()
        return
      }
      this.clearFlushTimer()
      await this.enqueueFlush(true)
      await this.closeRun(result)
    } catch (err) {
      console.error('[vulyk-reporter] failed to publish results:', err)
    } finally {
      this.warnMissingAnnotations()
    }
  }

  private warnMissingAnnotations(): void {
    if (this.missingAnnotationWarned || this.missingAnnotationTitles.length === 0) {
      return
    }
    this.missingAnnotationWarned = true
    console.warn(
      formatMissingAnnotationWarning(
        this.missingAnnotationTitles,
        this.missingAnnotationTitles.length,
        undefined,
        this.caseKeyPrefix || env('VULYK_CASE_KEY_PREFIX') || 'KEY',
      ),
    )
  }

  private scheduleFlush(): void {
    if (shouldFlushNow(this.pending.length, RESULT_BATCH_SIZE)) {
      this.clearFlushTimer()
      void this.enqueueFlush(false)
      return
    }
    if (this.flushTimer) return
    this.flushTimer = setTimeout(() => {
      this.flushTimer = null
      void this.enqueueFlush(false)
    }, RESULT_FLUSH_MS)
  }

  private clearFlushTimer(): void {
    if (this.flushTimer) {
      clearTimeout(this.flushTimer)
      this.flushTimer = null
    }
  }

  private enqueueFlush(drainAll: boolean): Promise<void> {
    this.flushChain = this.flushChain
      .then(() => this.flushPending(drainAll))
      .catch((err) => {
        console.error('[vulyk-reporter] flush failed:', err)
      })
    return this.flushChain
  }

  private async flushPending(drainAll: boolean): Promise<void> {
    await this.openReady
    if (!this.enabled || !this.runId || !this.projectId) return

    while (this.pending.length > 0) {
      const slice = drainAll
        ? takeFlushBatch(this.pending, this.pending.length)
        : takeFlushBatch(this.pending, RESULT_BATCH_SIZE)
      if (slice.length === 0) break
      const results = dedupePendingResults(slice)
      const resp = await this.api<{
        inserted: number
        unmatched_case_keys: string[]
      }>('POST', `/api/v1/projects/${this.projectId}/ci/runs/${this.runId}/results`, {
        results,
      })
      const unmatched = formatUnmatchedKeysWarning(
        resp.unmatched_case_keys ?? [],
        this.unmatchedWarned,
      )
      if (unmatched.message) {
        for (const k of unmatched.newlyWarned) this.unmatchedWarned.add(k)
        console.warn(unmatched.message)
      }
      if (this.dryRun) {
        console.log(
          `[vulyk-reporter] dry-run results: would insert ${results.length}`,
        )
      } else {
        console.log(
          `[vulyk-reporter] posted ${results.length} results (inserted=${resp.inserted})`,
        )
      }
      if (!drainAll) break
    }
  }

  private async api<T>(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<T> {
    if (this.dryRun) {
      const phase =
        path.endsWith('/results')
          ? 'results'
          : path.endsWith('/close')
            ? 'close'
            : path.endsWith('/start')
              ? 'start'
              : path.endsWith('/ci/runs') || /\/ci\/runs$/.test(path)
                ? 'open'
                : path === '/api/v1/projects'
                  ? 'resolve-project'
                  : 'open'
      const detail =
        body !== undefined
          ? `${method} ${path} ${JSON.stringify(body).slice(0, 400)}`
          : `${method} ${path}`
      console.log(formatDryRunLog(phase, detail))
      return dryRunApiResponse(method, path, body) as T
    }

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
    // Prefer an explicit UUID (CI tokens often cannot list all projects by key).
    const idEnv = env('VULYK_PROJECT_ID')
    if (idEnv && /^[0-9a-f-]{36}$/i.test(idEnv)) return idEnv
    const ref = this.projectRef
    if (/^[0-9a-f-]{36}$/i.test(ref)) return ref
    if (this.dryRun) {
      console.log(
        formatDryRunLog(
          'resolve-project',
          `skip project list; using dry-run id for key ${ref}`,
        ),
      )
      return 'dry-run-project-00000000-0000-4000-8000-000000000099'
    }
    const projects = await this.api<Array<{ id: string; key: string }>>(
      'GET',
      '/api/v1/projects',
    )
    const list = Array.isArray(projects) ? projects : []
    const hit = list.find((p) => p.key?.toUpperCase() === ref.toUpperCase())
    if (!hit) {
      throw new Error(
        `project key ${ref} not found (set VULYK_PROJECT_ID to the project UUID when using a CI token)`,
      )
    }
    return hit.id
  }

  private async openAndStart(): Promise<void> {
    this.projectId = await this.resolveProjectId()
    const existingRunId = env('VULYK_RUN_ID')

    if (existingRunId) {
      this.runId = existingRunId
      console.log(`[vulyk-reporter] attaching to existing run ${this.runId}`)
      await this.api(
        'POST',
        `/api/v1/projects/${this.projectId}/ci/runs/${this.runId}/start`,
        {
          pipeline_url: this.pipelineUrl,
          branch: process.env.GITHUB_REF_NAME || process.env.CI_COMMIT_REF_NAME,
          commit: process.env.GITHUB_SHA || process.env.CI_COMMIT_SHA,
        },
      )
    } else {
      const externalRef =
        env('VULYK_EXTERNAL_REF') ||
        `playwright:${process.env.GITHUB_RUN_ID || 'local'}:${Date.now()}`

      const opened = await this.api<{
        run: { id: string; status: string }
      }>('POST', `/api/v1/projects/${this.projectId}/ci/runs`, {
        provider: detectCiProvider(),
        // Display name stays the operator-facing run title; suite_name is the
        // technical suite label (may differ when both are set).
        suite_name: env('VULYK_SUITE_NAME') || 'playwright',
        name: this.runName,
        planned_total: this.plannedTotal > 0 ? this.plannedTotal : undefined,
        external_ref: externalRef,
        branch: process.env.GITHUB_REF_NAME || process.env.CI_COMMIT_REF_NAME,
        commit: process.env.GITHUB_SHA || process.env.CI_COMMIT_SHA,
        pipeline_url: this.pipelineUrl,
        // Create already in_progress so the UI shows the run during the suite.
        start: true,
      })
      this.runId = opened.run.id
      console.log(
        `[vulyk-reporter] opened run ${this.runId} (${opened.run.status})`,
      )
    }
    console.log(`[vulyk-reporter] run ${this.runId} → in_progress`)
  }

  private async closeRun(result: FullResult): Promise<void> {
    if (!this.runId || !this.projectId) return
    const cancelled = closeCancelled(result.status)
    await this.api(
      'POST',
      `/api/v1/projects/${this.projectId}/ci/runs/${this.runId}/close`,
      {
        cancelled,
        message: `playwright ${result.status}; streamed results`,
      },
    )
    console.log(
      `[vulyk-reporter] closed run ${this.runId}${cancelled ? ' (cancelled)' : ''}`,
    )
  }
}
