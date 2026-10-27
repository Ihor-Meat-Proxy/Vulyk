import { getStoredToken } from './token'

const API_BASE = import.meta.env.VITE_API_URL ?? ''

/** Parse double-submit CSRF cookie value from a Cookie header / document.cookie string. */
export function csrfTokenFromCookieHeader(
  cookieHeader: string | null | undefined,
): string | null {
  if (!cookieHeader) return null
  const match = cookieHeader.match(/(?:^|; )vulyk_csrf=([^;]*)/)
  if (!match) return null
  try {
    return decodeURIComponent(match[1])
  } catch {
    return match[1]
  }
}

/** Read double-submit CSRF cookie set at login (`vulyk_csrf`). */
export function getCsrfToken(): string | null {
  if (typeof document === 'undefined') return null
  return csrfTokenFromCookieHeader(document.cookie ?? '')
}

export class ApiError extends Error {
  status: number
  code: string

  constructor(message: string, status: number, code = 'error') {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.code = code
  }
}

/** Optional fetch options for cancelable GETs (review #16). */
export type ApiFetchOptions = {
  signal?: AbortSignal
}

/** True when `fetch` (or a wrapper) rejected because the request was aborted. */
export function isAbortError(err: unknown): boolean {
  if (err instanceof DOMException && err.name === 'AbortError') return true
  if (err instanceof Error && err.name === 'AbortError') return true
  return false
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const token = getStoredToken()
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(init?.headers as Record<string, string> | undefined),
  }
  if (token) {
    headers.Authorization = `Bearer ${token}`
  }
  const csrf = getCsrfToken()
  if (csrf) {
    headers['X-Vulyk-Csrf'] = csrf
  }

  const res = await fetch(`${API_BASE}${path}`, {
    ...init,
    headers,
    credentials: 'include',
  })
  if (!res.ok) {
    const text = await res.text()
    try {
      const parsed = JSON.parse(text) as {
        error?: { message?: string; code?: string }
      }
      if (parsed?.error?.message) {
        throw new ApiError(
          parsed.error.message,
          res.status,
          parsed.error.code ?? 'error',
        )
      }
    } catch (err) {
      if (err instanceof ApiError) throw err
      if (err instanceof Error && err.message !== text) throw err
    }
    throw new ApiError(text || res.statusText, res.status)
  }
  if (res.status === 204) {
    return undefined as T
  }
  return res.json() as Promise<T>
}

export type Page<T> = {
  items: T[]
  total: number
  limit: number
  offset: number
}

export type Project = {
  id: string
  key: string
  name: string
  description: string
  case_key_prefix: string
  next_case_number: number
  /** Seeded demo project — show Sample badge; removable via admin. */
  is_sample?: boolean
  created_at: string
  updated_at: string
}

export type Suite = {
  id: string
  project_id: string
  parent_id: string | null
  name: string
  sort_order: number
  /** Recursive total (folder + descendants). */
  case_count?: number
  /** Cases whose suite_id is exactly this folder. */
  direct_case_count?: number
  created_at?: string
  updated_at?: string
}

export type TestCase = {
  id: string
  project_id: string
  suite_id: string | null
  key: string
  title: string
  description: string
  precondition: string
  priority: string
  status: string
  version: number
  external_key?: string | null
  sort_order?: number
  created_at: string
  updated_at: string
  /** Present on list responses: linked via external_key. */
  automated?: boolean
  /** Latest test_results.status for the case. */
  last_run_status?: string | null
  folder_name?: string | null
  tag_names?: string[]
  /** Present on list for requested cf_ids / show_in_list fields. */
  custom_fields?: CustomFieldValue[]
}

export type TestStep = {
  id: string
  case_id: string
  position: number
  action: string
  expected_result: string
  data: string
}

export type Tag = {
  id: string
  project_id: string
  name: string
  color: string
  created_at: string
}

export type CustomFieldValue = {
  field_id: string
  name: string
  field_type: string
  key?: string
  value: unknown
}

export type CustomFieldOption = {
  id?: string
  key?: string | null
  label: string
  value: string
  colour?: string | null
}

export type TestCaseDetail = {
  case: TestCase
  steps: TestStep[]
  tags: Tag[]
  custom_fields: CustomFieldValue[]
}

export type CaseVersionSummary = {
  id: string
  case_id: string
  version: number
  change_summary: string
  created_at: string
}

export type CustomFieldEntity = 'test_case' | 'requirement' | 'both'
export type CustomFieldScope = 'all' | 'selected'
export type CustomFieldType =
  | 'text'
  | 'long_text'
  | 'number'
  | 'checkbox'
  | 'dropdown'
  | 'multi_select'
  | 'date'
  | 'url'
  | string

export type CustomField = {
  id: string
  project_id: string
  key: string
  name: string
  field_type: CustomFieldType
  entity: CustomFieldEntity
  option_list_id: string | null
  options: CustomFieldOption[] | unknown
  default_value: unknown
  required: boolean
  help_text: string
  sort_order: number
  show_in_list: boolean
  show_in_filters: boolean
  active: boolean
  scope: CustomFieldScope
  created_at?: string
}

export type CustomFieldProjectOverride = {
  field_id: string
  project_id: string
  required_override: boolean | null
  default_value_override: unknown
  enabled: boolean
}

export type CustomFieldDefinition = {
  id: string
  key: string
  name: string
  field_type: CustomFieldType
  entity: CustomFieldEntity
  option_list_id: string | null
  default_value: unknown
  required: boolean
  help_text: string
  sort_order: number
  show_in_list: boolean
  show_in_filters: boolean
  active: boolean
  scope: CustomFieldScope
  created_at: string
  updated_at: string
}

export type CustomFieldDefinitionDetail = CustomFieldDefinition & {
  project_ids: string[]
  project_overrides?: CustomFieldProjectOverride[]
  in_use?: boolean
}

export type CreateCustomFieldDefinition = {
  key: string
  name: string
  field_type: string
  entity?: string
  option_list_id?: string | null
  default_value?: unknown
  required?: boolean
  help_text?: string
  sort_order?: number
  show_in_list?: boolean
  show_in_filters?: boolean
  active?: boolean
  scope?: string
  project_ids?: string[]
}

export type UpdateCustomFieldDefinition = Partial<CreateCustomFieldDefinition> & {
  acknowledge_hide_values?: boolean
}

export type UpdateFieldResponse = {
  field: CustomFieldDefinitionDetail
  warning?: {
    warning: string
    hidden_project_ids: string[]
    values_retained: boolean
  }
}

export type OptionList = {
  id: string
  key: string
  name: string
  description: string
  active: boolean
  created_at: string
  updated_at: string
}

export type OptionListItem = {
  id: string
  list_id: string
  key: string | null
  label: string
  value: string | null
  colour: string | null
  description: string
  sort_order: number
  active: boolean
  deprecated: boolean
  created_at: string
  updated_at: string
}

export type OptionListDetail = OptionList & {
  items: OptionListItem[]
}

export type ConfigExportRequest = {
  project_scope?: 'all' | 'selected' | string
  project_ids?: string[]
  project_keys?: string[]
  include_memberships?: boolean
  include_data?: boolean
  include_folder_structure?: boolean
  secrets_passphrase?: string | null
  format?: 'json' | 'yaml' | string
}

export type ConfigChangeItem = {
  kind: string
  key: string
  action: string
  detail: string
}

export type ConfigImportPreview = {
  dry_run: boolean
  strategy: string
  creates: ConfigChangeItem[]
  updates: ConfigChangeItem[]
  conflicts: ConfigChangeItem[]
  skipped: ConfigChangeItem[]
  applied: boolean
}

export type TestPlan = {
  id: string
  project_id: string
  name: string
  description: string
  created_at: string
  updated_at: string
}

export type TestPlanDetail = {
  plan: TestPlan
  case_ids: string[]
  case_count: number
}

export type TestRun = {
  id: string
  project_id: string
  plan_id: string | null
  iteration_id?: string | null
  name: string
  description: string
  status: string
  source: string
  external_ref: string | null
  meta: Record<string, unknown>
  created_by_name?: string | null
  environment?: Record<string, unknown>
  artifacts?: Record<string, unknown>
  pass_count?: number
  fail_count?: number
  skip_count?: number
  blocked_count?: number
  duration_ms?: number | null
  started_at: string | null
  finished_at: string | null
  created_at: string
  updated_at: string
  suite_name?: string | null
  ci_provider?: string | null
  ci_repo?: string | null
  ci_branch?: string | null
  ci_commit?: string | null
  pipeline_url?: string | null
  job_url?: string | null
  lifecycle?: string
  is_sample?: boolean
}

export type RunProgress = {
  total: number
  untested: number
  passed: number
  failed: number
  blocked: number
  skipped: number
  retest: number
  completed: number
  percent: number
}

export type RunItem = {
  id: string
  run_id: string
  case_id: string
  assignee: string | null
  status: string
  sort_order: number
  comment: string
  executed_at: string | null
  created_at: string
  updated_at: string
}

export type RunItemView = {
  item: RunItem
  case_key: string
  case_title: string
  case_priority: string
}

export type TestRunDetail = {
  run: TestRun
  progress: RunProgress
  items: RunItemView[]
}

export type ResultStep = {
  id: string
  result_id: string
  step_id: string | null
  position: number
  action: string
  expected_result: string
  status: string
  actual_result: string
  comment: string
}

/** Canonical execution statuses (PascalCase), matching the API/DB. */
export const RESULT_STATUSES = [
  'Untested',
  'Ready',
  'Running',
  'Passed',
  'Failed',
  'Blocked',
  'Skipped',
  'Retest',
  'Untestable',
] as const

export type ResultStatus = (typeof RESULT_STATUSES)[number]

/** Dashboard donut buckets (Ready absorbs Untested/Retest). */
export const DASHBOARD_STATUSES = [
  'Ready',
  'Running',
  'Passed',
  'Failed',
  'Blocked',
  'Skipped',
  'Untestable',
] as const

export type DashboardStatus = (typeof DASHBOARD_STATUSES)[number]

/**
 * CSS modifier for status pills. Normalizes aliases so run/CI vocabularies
 * (`completed`, `in_progress`, `cancelled`, …) share colour-blind-safe styles
 * with result statuses where appropriate.
 */
export function statusClass(status: string): string {
  const raw = status.trim().toLowerCase().replace(/-/g, '_')
  const aliases: Record<string, string> = {
    completed: 'completed',
    succeeded: 'succeeded',
    closed: 'closed',
    in_progress: 'in_progress',
    cancelled: 'cancelled',
    canceled: 'cancelled',
    open: 'open',
    queued: 'queued',
    dispatched: 'dispatched',
    draft: 'draft',
  }
  const key = aliases[raw] ?? raw
  return `status-${key}`
}

export type Campaign = {
  id: string
  project_id: string
  parent_id: string | null
  name: string
  description: string
  sort_order: number
  created_at: string
  updated_at: string
}

export type CreateCampaign = {
  name: string
  description?: string
  parent_id?: string | null
  sort_order?: number
}

export type UpdateCampaign = {
  name?: string
  description?: string
  parent_id?: string | null
  sort_order?: number
}

export type Iteration = {
  id: string
  project_id: string
  campaign_id: string
  name: string
  reference: string
  description: string
  status: string
  mode: string
  cadence: string
  scheduled_start: string | null
  scheduled_end: string | null
  created_at: string
  updated_at: string
}

export type CreateIteration = {
  name: string
  reference?: string
  description?: string
  status?: string
  mode?: string
  cadence?: string
  scheduled_start?: string | null
  scheduled_end?: string | null
  case_ids?: string[]
  suite_ids?: string[]
}

export type UpdateIteration = {
  name?: string
  reference?: string
  description?: string
  status?: string
  mode?: string
  cadence?: string
  scheduled_start?: string | null
  scheduled_end?: string | null
  campaign_id?: string
}

export type IterationStatusCounts = {
  ready: number
  running: number
  passed: number
  failed: number
  blocked: number
  skipped: number
  untestable: number
  total: number
}

export type PassRatePoint = {
  run_id: string
  run_name: string
  created_at: string
  pass_rate: number
  passed: number
  failed: number
  total: number
}

export type FlakyTestSummary = {
  case_id: string
  case_key: string
  case_title: string
  flips: number
  latest_status: string
  statuses: string[]
}

export type SlowTestSummary = {
  case_id: string
  case_key: string
  case_title: string
  duration_ms: number
  status: string
}

export type IterationDashboard = {
  iteration: Iteration
  campaign_name: string
  campaign_path: string[]
  status_counts: IterationStatusCounts
  pass_rate_trend: PassRatePoint[]
  flaky_tests: FlakyTestSummary[]
  slowest_tests: SlowTestSummary[]
}

export type ExecutionsTreeNode = {
  id: string
  kind: string
  name: string
  reference: string | null
  status: string | null
  mode: string | null
  parent_id: string | null
  children: ExecutionsTreeNode[]
}

export type AutomatedSuite = {
  id: string
  project_id: string
  iteration_id: string | null
  name: string
  status: string
  source: string
  created_by_name: string | null
  created_at: string
  started_at: string | null
  finished_at: string | null
  duration_ms: number | null
  pass_count: number
  fail_count: number
  skip_count: number
  blocked_count: number
  environment: Record<string, unknown>
  artifacts: Record<string, unknown> | unknown[]
  external_ref: string | null
  suite_name?: string | null
  ci_provider?: string | null
  pipeline_url?: string | null
  job_url?: string | null
  is_sample?: boolean
}

export type AutomatedSuitesParams = {
  status?: string
  created_by?: string
  q?: string
  limit?: number
  offset?: number
  sort?: string
}

export type SuiteResultRow = {
  result_id: string
  case_id: string | null
  case_key: string | null
  case_title: string | null
  status: string
  message: string
  environment: string | null
  environment_vars: Record<string, unknown>
  dataset: string | null
  executed_at: string
  duration_ms: number | null
  attachment_count: number
  external_id: string | null
}

export type CaseExecutionHistoryItem = {
  result_id: string
  path: string
  plan_type: string
  exec_number: number
  mode: string
  status: string
  executed_by: string | null
  executed_on: string
  linked_issues: number
  run_id: string
  iteration_id: string | null
  duration_ms: number | null
}

export type CaseExecutionHistoryPage = {
  items: CaseExecutionHistoryItem[]
  total: number
  limit: number
  offset: number
  pass_rate: number
  sparkline: string[]
}

export type AttachmentMeta = {
  id: string
  project_id: string
  entity_type: string
  entity_id: string
  filename: string
  content_type: string
  size_bytes: number
  created_at: string
}

export type TestResult = {
  id: string
  run_id: string
  case_id: string | null
  run_item_id: string | null
  status: string
  message: string
  source: string
  external_id: string | null
  duration_ms: number | null
  assignee: string | null
  elapsed_ms: number | null
  executed_by: string | null
  case_version: number | null
  executed_at: string
  created_at?: string
}

export type TestResultDetail = {
  result: TestResult
  steps: ResultStep[]
}

export type TestResultHistoryItem = {
  result: TestResult
  run_name: string | null
  executed_by_name: string | null
  executed_by_email: string | null
  attachment_count: number
  defect_count: number
  steps: ResultStep[]
}

export type CaseResultsSummary = {
  case_id: string
  latest_status: string | null
  latest_executed_at: string | null
  total_results: number
  passed: number
  failed: number
  blocked: number
  skipped: number
  retest: number
  pass_rate: number
}

export type CaseResultsPage = {
  summary: CaseResultsSummary
  items: TestResultHistoryItem[]
  total: number
  limit: number
  offset: number
}

export type RunItemExecution = {
  item: RunItemView
  steps: TestStep[]
  latest_result: TestResultDetail | null
  history: TestResultHistoryItem[]
}

export type ListCasesParams = {
  suiteId?: string | null
  /** Only cases directly in suiteId (no descendants). */
  directOnly?: boolean
  /** Only cases with no suite_id. */
  unfiledOnly?: boolean
  q?: string
  tag?: string
  status?: string
  priority?: string
  /** Server-side sort field (key, title, automated, last_run_status, cf:<uuid>, …). */
  sort?: string
  /** asc | desc */
  order?: 'asc' | 'desc'
  /** Custom field ids to include on each list row. */
  cfIds?: string[]
  /** Map of custom field_id → value for server-side filtering. */
  cf?: Record<string, unknown>
  limit?: number
  offset?: number
  /** Abort in-flight list fetch on navigation / filter change. */
  signal?: AbortSignal
}

export type CaseListColumnPref = {
  id: string
  visible: boolean
}

export type CaseListColumnsConfig = {
  columns: CaseListColumnPref[]
  sort?: string | null
  order?: 'asc' | 'desc' | null
}

export type UserPreferences = {
  theme: string
  font_size: number
  font_family?: string
  density: string
  reduced_motion: boolean | null
  notify_email?: boolean
  notify_in_app?: boolean
  notify_run_updates?: boolean
}

export type UpdatePreferences = {
  theme?: string
  font_size?: number
  font_family?: string
  density?: string
  reduced_motion?: boolean | null
  notify_email?: boolean
  notify_in_app?: boolean
  notify_run_updates?: boolean
}

export type UserPublic = {
  id: string
  email: string
  display_name: string
  is_system_admin: boolean
  must_change_password: boolean
  locale: string | null
  timezone: string | null
  preferences: UserPreferences
  disabled_at: string | null
  created_at: string
  updated_at: string
}

export type RegistrationMode = 'open' | 'invite' | 'disabled'

export type SsoProviderPublic = {
  id: string
  provider_key: string
  display_name: string
  enabled: boolean
}

export type PasswordPolicyPublic = {
  min_length: number
  require_letter: boolean
  require_digit: boolean
}

export type AuthStatus = {
  auth_required: boolean
  bootstrap_available: boolean
  setup_required: boolean
  registration_mode: RegistrationMode
  default_locale: string
  default_timezone: string
  default_theme: string
  instance_name: string
  authenticated: boolean
  user: UserPublic | null
  sso_providers: SsoProviderPublic[]
  password_policy: PasswordPolicyPublic
}

export type SetupStatus = {
  setup_completed: boolean
  setup_required: boolean
}

export type CompleteSetupRequest = {
  setup_token: string
  email: string
  password: string
  display_name: string
  instance_name?: string
  base_url?: string
  default_locale?: string
  default_timezone?: string
  default_theme?: string
  registration_mode?: RegistrationMode
  /** Seed sample projects: Kyiv concert + Vulyk QA (default true). */
  seed_sample?: boolean
}

export type AuthResponse = {
  token: string
  token_type: string
  expires_in_hours: number
  user: UserPublic
}

export type ProjectRole =
  | 'admin'
  | 'test_architect'
  | 'test_automation_engineer'
  | 'developer'
  | 'viewer'

export type SampleDataSummary = {
  projects: number
  runs: number
  campaigns: number
  cases: number
}

export type SampleDataStatus = {
  present: boolean
  projects: { id: string; key: string; name: string }[]
  summary: SampleDataSummary
}

export type InstanceSettings = {
  setup_completed: boolean
  instance_name: string
  base_url: string
  default_locale: string
  default_timezone: string
  default_theme: string
  registration_mode: RegistrationMode
  default_role: ProjectRole
  allowed_email_domains: string[]
}

export type UpdateInstanceSettings = {
  instance_name?: string
  base_url?: string
  default_locale?: string
  default_timezone?: string
  default_theme?: string
  registration_mode?: RegistrationMode
  default_role?: ProjectRole
  allowed_email_domains?: string[]
}

export type UpdatesCapability = 'compose_updater' | 'manual_only'

export type UpdateJob = {
  id: string
  target_version: string
  status: string
  progress: number
  message: string
  log_text: string
  created_at: string
  updated_at: string
  finished_at: string | null
}

export type VersionBackupInfo = {
  id: string
  version: string | null
  from_version: string | null
  to_version: string | null
  roles: string[]
  created_at: string | null
  expires_at: string | null
  size_bytes: number
  in_progress: boolean
}

export type RollbackPointInfo = {
  id: string
  version: string
  from_version: string | null
  to_version: string | null
  expires_at: string | null
  created_at: string | null
  size_bytes: number
}

export type VersionBackupRetention = {
  rollback_days: number
  version_count: number
}

export type UpdatesStatus = {
  current_version: string
  latest_version: string | null
  latest_notes: string | null
  latest_url: string | null
  update_available: boolean
  last_checked_at: string | null
  check_interval_hours: number
  capability: UpdatesCapability
  manual_command: string
  rollback_command: string
  github_repo: string
  active_job: UpdateJob | null
  rollback: RollbackPointInfo | null
  version_backups: VersionBackupInfo[]
  retention: VersionBackupRetention
}

export type UpdatesPreferences = {
  check_interval_hours: number
}

export type SsoProviderAdmin = {
  id: string
  provider_key: string
  display_name: string
  enabled: boolean
  issuer_url: string | null
  authorization_endpoint: string | null
  token_endpoint: string | null
  userinfo_endpoint: string | null
  jwks_uri: string | null
  client_id: string
  has_client_secret: boolean
  scopes: string
  extra_config: unknown
  default_role: ProjectRole
  group_role_mapping: unknown
  allowed_email_domains: string[]
  created_at: string
  updated_at: string
}

export type UpdateSsoProviderRequest = {
  display_name?: string
  enabled?: boolean
  issuer_url?: string | null
  authorization_endpoint?: string | null
  token_endpoint?: string | null
  userinfo_endpoint?: string | null
  jwks_uri?: string | null
  client_id?: string
  client_secret?: string | null
  scopes?: string
  extra_config?: unknown
  default_role?: ProjectRole
  group_role_mapping?: unknown
  allowed_email_domains?: string[]
}

export type RoleMatrixRow = {
  role: ProjectRole
  display_name: string
  permissions: string[]
}

export type RoleMatrixResponse = {
  roles: RoleMatrixRow[]
}

export type MembershipView = {
  id: string
  project_id: string
  user_id: string
  role: ProjectRole
  email: string
  display_name: string
  created_at: string
  updated_at: string
}

export type IntegrationSetting = {
  id: string
  project_id: string | null
  kind: string
  name: string
  config: Record<string, unknown>
  enabled: boolean
  created_at: string
  updated_at: string
}

export type BackupDestination = {
  id: string
  name: string
  kind: string
  config: Record<string, unknown>
  enabled: boolean
  has_secrets: boolean
  created_at: string
  updated_at: string
}

export type BackupPolicy = {
  id: string
  project_id: string | null
  destination_id: string
  schedule: string
  debounce_minutes: number
  retain_count: number | null
  retain_days: number | null
  pack_mode: string
  include_attachments: boolean
  enabled: boolean
  last_scheduled_at: string | null
  created_at: string
  updated_at: string
}

export type BackupRun = {
  id: string
  policy_id: string | null
  destination_id: string | null
  project_id: string | null
  status: string
  trigger: string
  remote_path: string | null
  local_path: string | null
  size_bytes: number
  duration_ms: number | null
  error: string | null
  summary: Record<string, unknown>
  checksum_sha256: string | null
  started_at: string
  finished_at: string | null
  created_at: string
}

export type BackupTestResult = {
  ok: boolean
  message: string
}

export type RestorePreview = {
  dry_run: boolean
  mode: string
  schema_version: string
  project_key: string
  counts: Record<string, number>
  conflicts: { entity: string; key: string; detail: string }[]
  custom_field_mappings: {
    archive_key: string
    archive_name: string
    action: string
    target_key: string | null
  }[]
  warnings: string[]
  applied: boolean
}

export type ProviderKindInfo = {
  id: string
  name: string
  default_base_url: string | null
  requires_api_key: boolean
}

export type LlmConfig = {
  id: string
  project_id: string | null
  provider: string
  name: string
  base_url: string | null
  model: string
  has_api_key: boolean
  is_default: boolean
  created_at: string
  updated_at: string
}

export type GeneratedCaseDraft = {
  key: string
  title: string
  description: string
  precondition: string
  priority: string
}

export type ChatResponseKind =
  | 'answer'
  | 'clarify'
  | 'propose_cases'
  | 'propose_requirements'

export type ProposedCaseDraft = {
  title: string
  description: string
  precondition: string
  priority: string
  tags: string[]
  steps: { action: string; expected_result: string; data?: string }[]
}

export type ProposedRequirementDraft = {
  title: string
  description: string
  acceptance_criteria: string
}

export type ProposedLinkDraft = {
  case_index: number
  requirement_index: number
}

export type ChatPlacement = {
  project_id: string
  folder_id: string | null
  folder_path: string
  new_folder_name: string | null
  reason: string
}

export type ChatProposal = {
  placement: ChatPlacement
  cases: ProposedCaseDraft[]
  requirements: ProposedRequirementDraft[]
  links: ProposedLinkDraft[]
}

export type ChatProposeResponse = {
  provider: string
  model: string
  kind: ChatResponseKind
  answer?: string
  clarify_question?: string
  proposal?: ChatProposal
  confirmation_prompt?: string
}

export type ChatCreationEvent = {
  id: string
  project_id: string
  provider: string
  model: string
  kind: string
  entity_id: string | null
  entity_key: string | null
  summary: string
  created_at: string
}

export type ChatConfirmResponse = {
  cases: TestCaseDetail[]
  requirements: Requirement[]
  created_folder_id: string | null
  links_created: number
  events: ChatCreationEvent[]
}

export type RequirementFolder = {
  id: string
  project_id: string
  parent_id: string | null
  name: string
  sort_order: number
  requirement_count: number
  created_at: string
  updated_at: string
}

export type ContextFileRef = {
  path: string
  language: string
  score: number
  reason: string
}

export type AutomationRepo = {
  id: string
  name: string
  /** Legacy singular project link (first/default); prefer `project_ids`. */
  project_id: string | null
  project_ids: string[]
  provider: string
  remote_url: string
  branch: string
  subpath: string
  has_token: boolean
  has_actions_token: boolean
  ci_workflow_file: string
  ci_workflow_ref: string
  index_status: string
  last_indexed_at: string | null
  last_index_error: string | null
  commit_sha: string | null
  file_count: number
  chunk_count: number
  created_at: string
  updated_at: string
}

export type CreateAutomationRepoBody = {
  name: string
  provider: string
  remote_url: string
  branch?: string
  subpath?: string
  token?: string
  actions_token?: string
  ci_workflow_file?: string
  ci_workflow_ref?: string
  project_ids?: string[]
}

export type UpdateAutomationRepoBody = {
  name?: string
  provider?: string
  remote_url?: string
  branch?: string
  subpath?: string
  /** Omit to leave unchanged; empty string clears. */
  token?: string
  actions_token?: string
  ci_workflow_file?: string
  ci_workflow_ref?: string
  project_ids?: string[]
}

export type SetAutomationRepoProjectsBody = {
  project_ids: string[]
  default_project_id?: string | null
}

export type ApiTokenPublic = {
  id: string
  name: string
  token_prefix: string
  created_at: string
  last_used_at: string | null
  revoked_at: string | null
}

export type ProjectCiToken = {
  id: string
  project_id: string
  name: string
  token_prefix: string
  created_at: string
  last_used_at: string | null
  revoked_at: string | null
}

export type CiConfig = {
  id: string
  project_id: string
  provider: string
  name: string
  /** Provider config with secrets stripped (`token` / `trigger_token` never returned). */
  config: Record<string, unknown>
  has_token: boolean
  has_webhook_secret: boolean
  enabled: boolean
}

export type CiDispatch = {
  id: string
  project_id: string
  ci_config_id: string
  run_id: string | null
  status: string
  external_id: string | null
  external_url: string | null
  inputs: Record<string, unknown>
  last_error: string | null
  created_at: string
  updated_at: string
}

export type Requirement = {
  id: string
  project_id: string
  folder_id?: string | null
  external_key: string | null
  external_id: string | null
  title: string
  description: string
  acceptance_criteria?: string
  source: string
  url: string
  created_at: string
  updated_at: string
  custom_fields?: CustomFieldValue[]
}

export type ImporterInfo = {
  id: string
  name: string
  supported_formats: string[]
}

export type FieldMapping = {
  title: string
  key: string
  description: string
  precondition: string
  priority: string
  status: string
  suite: string
  steps: string
}

export type ImportCaseDraft = {
  key?: string | null
  title: string
  description: string
  precondition: string
  priority: string
  status: string
  suite_name?: string | null
  steps: { action: string; expected_result: string; data: string }[]
  external_id?: string | null
}

export type ImportPreview = {
  importer_id: string
  format: string
  total: number
  cases: ImportCaseDraft[]
  warnings: string[]
  field_mapping: FieldMapping
}

export type ImportReport = {
  importer_id: string
  created_cases: number
  created_suites: number
  skipped: number
  warnings: string[]
  case_ids: string[]
}

export type IntegrationPublic = {
  id: string
  project_id: string
  kind: string
  name: string
  base_url: string
  config: Record<string, unknown>
  has_secrets: boolean
  enabled: boolean
}

export type Defect = {
  id: string
  project_id: string
  integration_id: string | null
  result_id: string | null
  case_id: string | null
  external_key: string | null
  external_url: string
  title: string
  status: string
}

export type StatusCounts = {
  total: number
  passed: number
  failed: number
  skipped: number
  blocked: number
  error: number
  untested: number
  retest: number
  other: number
}

export type RunSummaryReport = {
  run_id: string
  project_id: string
  name: string
  status: string
  started_at: string | null
  finished_at: string | null
  counts: StatusCounts
  pass_rate: number
}

export type TrendPoint = {
  bucket: string
  run_id: string | null
  run_name: string | null
  counts: StatusCounts
  pass_rate: number
}

export type PassFailTrends = {
  project_id: string
  granularity: string
  points: TrendPoint[]
}

export type RequirementCoverageRow = {
  requirement_id: string
  external_key: string | null
  title: string
  source: string
  linked_cases: number
  cases_with_results: number
  cases_passed: number
  cases_failed: number
  coverage_percent: number
  pass_percent: number
}

export type RequirementCoverageReport = {
  project_id: string
  total_requirements: number
  covered_requirements: number
  coverage_percent: number
  rows: RequirementCoverageRow[]
}

export type FlakyCase = {
  case_id: string
  case_key: string
  title: string
  runs_seen: number
  passes: number
  failures: number
  flake_score: number
  last_statuses: string[]
}

export type FlakyReport = {
  project_id: string
  lookback_runs: number
  cases: FlakyCase[]
}

export type ProjectDashboard = {
  project_id: string
  project_key: string
  project_name: string
  total_cases: number
  total_runs: number
  total_requirements: number
  recent_run_summaries: RunSummaryReport[]
  trends: PassFailTrends
  coverage: RequirementCoverageReport
  flaky: FlakyReport
}

export type RunReportDetail = {
  run_id: string
  project_id: string
  name: string
  description: string
  status: string
  source: string
  source_kind: string
  external_ref: string | null
  created_by_name: string | null
  environment: Record<string, unknown>
  artifacts: unknown
  meta: Record<string, unknown>
  branch: string | null
  commit: string | null
  pipeline_url: string | null
  environment_label: string | null
  started_at: string | null
  finished_at: string | null
  duration_ms: number | null
  counts: StatusCounts
  pass_rate: number
}

export type RunResultDefect = {
  id: string
  external_key: string | null
  external_url: string
  title: string
  status: string
}

export type RunResultAttachment = {
  id: string
  filename: string
  content_type: string
  size_bytes: number
}

export type RunCaseResultRow = {
  case_id: string
  case_key: string
  case_title: string
  folder_path: string
  suite_id: string | null
  priority: string
  status: string
  run_item_id: string | null
  result_id: string | null
  assignee: string | null
  executed_by_name: string | null
  duration_ms: number | null
  comment: string
  attachment_count: number
  defect_count: number
  defects: RunResultDefect[]
  attachments: RunResultAttachment[]
  previous_status: string | null
  comparison: string | null
}

export type RunCaseResultsPage = {
  items: RunCaseResultRow[]
  total: number
  limit: number
  offset: number
}

export type RunResultStepRow = {
  position: number
  action: string
  expected_result: string
  actual_result: string
  status: string
  comment: string
}

export type RunResultCommentEntry = {
  result_id: string
  status: string
  message: string
  executed_at: string
  executed_by_name: string | null
}

export type RunCaseResultExpand = {
  case_id: string
  run_item_id: string | null
  result_id: string | null
  steps: RunResultStepRow[]
  comment_history: RunResultCommentEntry[]
}

export type RunResultGroup = {
  key: string
  label: string
  counts: StatusCounts
  pass_rate: number
}

export type RunResultGroups = {
  group_by: string
  groups: RunResultGroup[]
}

export type CoverageLinkedCase = {
  case_id: string
  case_key: string
  title: string
  latest_status: string | null
}

export type CoverageDetailRow = {
  requirement_id: string
  external_key: string | null
  title: string
  source: string
  covered: boolean
  linked_cases: CoverageLinkedCase[]
  cases_passed: number
  cases_failed: number
  pass_percent: number
}

export type RequirementCoverageDetail = {
  project_id: string
  total_requirements: number
  covered_requirements: number
  uncovered_requirements: number
  coverage_percent: number
  covered: CoverageDetailRow[]
  uncovered: CoverageDetailRow[]
}

export type RunResultsParams = {
  status?: string
  folder?: string
  suite_id?: string
  priority?: string
  assignee?: string
  q?: string
  sort?: string
  limit?: number
  offset?: number
}

const defaultMapping = (): FieldMapping => ({
  title: 'title',
  key: 'key',
  description: 'description',
  precondition: 'precondition',
  priority: 'priority',
  status: 'status',
  suite: 'suite',
  steps: 'steps',
})

export function ssoStartUrl(providerKey: string): string {
  return `${API_BASE}/api/v1/auth/sso/${encodeURIComponent(providerKey)}/start`
}

export const api = {
  authStatus: () => request<AuthStatus>('/api/v1/auth/status'),
  login: (body: { email: string; password: string }) =>
    request<AuthResponse>('/api/v1/auth/login', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  register: (body: {
    email: string
    password: string
    display_name: string
    invite_token?: string
  }) =>
    request<AuthResponse>('/api/v1/auth/register', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  bootstrap: (body: {
    email: string
    password: string
    display_name: string
  }) =>
    request<AuthResponse>('/api/v1/auth/bootstrap', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  logout: () => request<void>('/api/v1/auth/logout', { method: 'POST' }),
  changePassword: (body: {
    current_password?: string
    new_password: string
  }) =>
    request<void>('/api/v1/auth/change-password', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  updateMe: (body: {
    display_name?: string
    locale?: string
    timezone?: string
    preferences?: UpdatePreferences
  }) =>
    request<UserPublic>('/api/v1/auth/me', {
      method: 'PATCH',
      body: JSON.stringify(body),
    }),
  getMe: () => request<UserPublic>('/api/v1/auth/me'),

  setupStatus: () => request<SetupStatus>('/api/v1/setup/status'),
  completeSetup: (body: CompleteSetupRequest) =>
    request<{ ok: boolean; setup_completed: boolean }>(
      '/api/v1/setup/complete',
      { method: 'POST', body: JSON.stringify(body) },
    ),

  getRoleMatrix: () => request<RoleMatrixResponse>('/api/v1/roles/matrix'),
  getAdminSettings: () =>
    request<InstanceSettings>('/api/v1/admin/settings'),
  updateAdminSettings: (body: UpdateInstanceSettings) =>
    request<InstanceSettings>('/api/v1/admin/settings', {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  getAdminUpdates: () => request<UpdatesStatus>('/api/v1/admin/updates'),
  checkAdminUpdates: () =>
    request<UpdatesStatus>('/api/v1/admin/updates/check', { method: 'POST' }),
  applyAdminUpdate: (version: string) =>
    request<{ job: UpdateJob }>('/api/v1/admin/updates/apply', {
      method: 'POST',
      body: JSON.stringify({ version }),
    }),
  applyAdminRollback: (body: { confirm: string; version?: string }) =>
    request<{ job: UpdateJob }>('/api/v1/admin/updates/rollback', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  listAdminUpdateJobs: () =>
    request<{ items: UpdateJob[] }>('/api/v1/admin/updates/jobs'),
  getAdminUpdateJob: (jobId: string) =>
    request<UpdateJob>(`/api/v1/admin/updates/jobs/${jobId}`),
  listAdminVersionBackups: () =>
    request<{
      items: VersionBackupInfo[]
      rollback: RollbackPointInfo | null
      retention: VersionBackupRetention
    }>('/api/v1/admin/updates/version-backups'),
  downloadAdminVersionBackupUrl: (backupId: string) =>
    `/api/v1/admin/updates/version-backups/${encodeURIComponent(backupId)}/download`,
  getAdminUpdatesPreferences: () =>
    request<UpdatesPreferences>('/api/v1/admin/updates/preferences'),
  updateAdminUpdatesPreferences: (body: { check_interval_hours: number }) =>
    request<UpdatesPreferences>('/api/v1/admin/updates/preferences', {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  getSampleData: () =>
    request<SampleDataStatus>('/api/v1/admin/sample-data'),
  removeSampleData: () =>
    request<SampleDataSummary>('/api/v1/admin/sample-data', {
      method: 'DELETE',
    }),
  reseedSampleData: (body?: { locale?: string }) =>
    request<SampleDataStatus>('/api/v1/admin/sample-data/reseed', {
      method: 'POST',
      body: JSON.stringify(body ?? {}),
    }),
  listSsoProvidersAdmin: () =>
    request<SsoProviderAdmin[]>('/api/v1/admin/sso/providers'),
  updateSsoProvider: (id: string, body: UpdateSsoProviderRequest) =>
    request<SsoProviderAdmin>(`/api/v1/admin/sso/providers/${id}`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),

  listUsers: () => request<UserPublic[]>('/api/v1/users'),
  createUser: (body: {
    email: string
    password: string
    display_name: string
    is_system_admin?: boolean
  }) =>
    request<UserPublic>('/api/v1/users', {
      method: 'POST',
      body: JSON.stringify(body),
    }),

  listMemberships: (projectId: string) =>
    request<MembershipView[]>(`/api/v1/projects/${projectId}/memberships`),
  upsertMembership: (
    projectId: string,
    body: { user_id: string; role: ProjectRole },
  ) =>
    request(`/api/v1/projects/${projectId}/memberships`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),

  listAdminIntegrations: () =>
    request<IntegrationSetting[]>('/api/v1/admin/integrations'),
  createAdminIntegration: (body: {
    kind: string
    name: string
    config?: Record<string, unknown>
    enabled?: boolean
    project_id?: string | null
  }) =>
    request<IntegrationSetting>('/api/v1/admin/integrations', {
      method: 'POST',
      body: JSON.stringify(body),
    }),

  listBackupDestinations: () =>
    request<BackupDestination[]>('/api/v1/admin/backups/destinations'),
  createBackupDestination: (body: {
    name: string
    kind: string
    config?: Record<string, unknown>
    secrets?: Record<string, unknown>
    enabled?: boolean
  }) =>
    request<BackupDestination>('/api/v1/admin/backups/destinations', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  deleteBackupDestination: (id: string) =>
    request<void>(`/api/v1/admin/backups/destinations/${id}`, {
      method: 'DELETE',
    }),
  testBackupDestination: (id: string) =>
    request<BackupTestResult>(`/api/v1/admin/backups/destinations/${id}/test`, {
      method: 'POST',
    }),
  listBackupPolicies: () =>
    request<BackupPolicy[]>('/api/v1/admin/backups/policies'),
  createBackupPolicy: (body: {
    project_id?: string | null
    destination_id: string
    schedule?: string
    debounce_minutes?: number
    retain_count?: number | null
    retain_days?: number | null
    pack_mode?: string
    include_attachments?: boolean
    enabled?: boolean
  }) =>
    request<BackupPolicy>('/api/v1/admin/backups/policies', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  listBackupRuns: (opts?: { project_id?: string; limit?: number }) => {
    const qs = new URLSearchParams()
    if (opts?.project_id) qs.set('project_id', opts.project_id)
    if (opts?.limit) qs.set('limit', String(opts.limit))
    const q = qs.toString()
    return request<BackupRun[]>(
      `/api/v1/admin/backups/runs${q ? `?${q}` : ''}`,
    )
  },
  runBackupNow: (body: {
    destination_id: string
    project_id?: string | null
    pack_mode?: string
    include_attachments?: boolean
  }) =>
    request<BackupRun>('/api/v1/admin/backups/run', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  downloadBackupRun: async (id: string) => {
    const token = getStoredToken()
    const headers: Record<string, string> = {}
    if (token) headers.Authorization = `Bearer ${token}`
    const csrf = getCsrfToken()
    if (csrf) headers['X-Vulyk-Csrf'] = csrf
    const res = await fetch(
      `${API_BASE}/api/v1/admin/backups/runs/${id}/download`,
      { headers, credentials: 'include' },
    )
    if (!res.ok) throw new Error((await res.text()) || res.statusText)
    const blob = await res.blob()
    const disposition = res.headers.get('content-disposition') ?? ''
    const match = /filename="?([^"]+)"?/.exec(disposition)
    const filename = match?.[1] ?? 'backup.zip'
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = filename
    a.click()
    URL.revokeObjectURL(url)
  },
  restoreBackup: (body: {
    run_id?: string
    archive?: unknown
    mode?: string
    conflict?: string
    target_project_id?: string
    new_project_key?: string
    dry_run?: boolean
  }) =>
    request<RestorePreview>('/api/v1/admin/backups/restore', {
      method: 'POST',
      body: JSON.stringify(body),
    }),

  listProjects: () => request<Project[]>('/api/v1/projects'),
  createProject: (body: {
    key?: string
    name: string
    description?: string
    case_key_prefix?: string
  }) =>
    request<Project>('/api/v1/projects', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  updateProject: (
    projectId: string,
    body: {
      name?: string
      description?: string
      case_key_prefix?: string
    },
  ) =>
    request<Project>(`/api/v1/projects/${projectId}`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  listSuites: (projectId: string) =>
    request<Suite[]>(`/api/v1/projects/${projectId}/suites`),
  createSuite: (
    projectId: string,
    body: { name: string; parent_id?: string | null; sort_order?: number },
  ) =>
    request<Suite>(`/api/v1/projects/${projectId}/suites`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  updateSuite: (
    projectId: string,
    suiteId: string,
    body: {
      name?: string
      parent_id?: string | null
      clear_parent?: boolean
      sort_order?: number
    },
  ) =>
    request<Suite>(`/api/v1/projects/${projectId}/suites/${suiteId}`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  deleteSuite: (
    projectId: string,
    suiteId: string,
    onNonempty: 'move_to_parent' | 'delete_cases' = 'move_to_parent',
  ) =>
    request<void>(
      `/api/v1/projects/${projectId}/suites/${suiteId}?on_nonempty=${onNonempty}`,
      { method: 'DELETE' },
    ),
  listCases: (projectId: string, params: ListCasesParams = {}) => {
    const qs = new URLSearchParams()
    if (params.suiteId) qs.set('suite_id', params.suiteId)
    if (params.directOnly) qs.set('direct_only', 'true')
    if (params.unfiledOnly) qs.set('unfiled_only', 'true')
    if (params.q) qs.set('q', params.q)
    if (params.tag) qs.set('tag', params.tag)
    if (params.status) qs.set('status', params.status)
    if (params.priority) qs.set('priority', params.priority)
    if (params.sort) qs.set('sort', params.sort)
    if (params.order) qs.set('order', params.order)
    if (params.cfIds?.length) qs.set('cf_ids', params.cfIds.join(','))
    if (params.cf && Object.keys(params.cf).length > 0) {
      qs.set('cf', JSON.stringify(params.cf))
    }
    qs.set('limit', String(params.limit ?? 100))
    qs.set('offset', String(params.offset ?? 0))
    return request<Page<TestCase>>(
      `/api/v1/projects/${projectId}/cases?${qs.toString()}`,
      params.signal ? { signal: params.signal } : undefined,
    )
  },
  getCaseListColumns: (projectId: string, opts?: ApiFetchOptions) =>
    request<CaseListColumnsConfig>(
      `/api/v1/projects/${projectId}/preferences/case-list-columns`,
      opts?.signal ? { signal: opts.signal } : undefined,
    ),
  putCaseListColumns: (projectId: string, body: CaseListColumnsConfig) =>
    request<CaseListColumnsConfig>(
      `/api/v1/projects/${projectId}/preferences/case-list-columns`,
      { method: 'PUT', body: JSON.stringify(body) },
    ),
  resetCaseListColumns: (projectId: string) =>
    request<CaseListColumnsConfig>(
      `/api/v1/projects/${projectId}/preferences/case-list-columns`,
      { method: 'DELETE' },
    ),
  getCase: (projectId: string, caseId: string, opts?: ApiFetchOptions) =>
    request<TestCaseDetail>(
      `/api/v1/projects/${projectId}/cases/${caseId}`,
      opts?.signal ? { signal: opts.signal } : undefined,
    ),
  /** Resolve a case by human-readable key (PREFIX-N), case-insensitive. */
  getCaseByKey: (key: string, opts?: ApiFetchOptions) =>
    request<TestCaseDetail>(
      `/api/v1/cases/by-key/${encodeURIComponent(key)}`,
      opts?.signal ? { signal: opts.signal } : undefined,
    ),
  updateCase: (
    projectId: string,
    caseId: string,
    body: Record<string, unknown>,
  ) =>
    request<TestCaseDetail>(`/api/v1/projects/${projectId}/cases/${caseId}`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  createCase: (projectId: string, body: Record<string, unknown>) =>
    request<TestCaseDetail>(`/api/v1/projects/${projectId}/cases`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  bulkCases: (projectId: string, body: Record<string, unknown>) =>
    request<{ affected: number; action: string }>(
      `/api/v1/projects/${projectId}/cases/bulk`,
      { method: 'POST', body: JSON.stringify(body) },
    ),
  downloadCasePdf: async (
    projectId: string,
    idOrKey: string,
    opts: { results?: number; locale?: string } = {},
  ) => {
    const qs = new URLSearchParams()
    qs.set('results', String(opts.results ?? 5))
    if (opts.locale) qs.set('locale', opts.locale)
    const token = getStoredToken()
    const headers: Record<string, string> = {}
    if (token) headers.Authorization = `Bearer ${token}`
    const csrf = getCsrfToken()
    if (csrf) headers['X-Vulyk-Csrf'] = csrf
    if (opts.locale) headers['Accept-Language'] = opts.locale
    const res = await fetch(
      `${API_BASE}/api/v1/projects/${projectId}/cases/${encodeURIComponent(idOrKey)}/export.pdf?${qs}`,
      { headers, credentials: 'include' },
    )
    if (!res.ok) {
      throw new Error((await res.text()) || res.statusText)
    }
    const blob = await res.blob()
    const disposition = res.headers.get('content-disposition') ?? ''
    const match = /filename="?([^"]+)"?/.exec(disposition)
    const filename = match?.[1] ?? `${idOrKey}.pdf`
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = filename
    a.click()
    URL.revokeObjectURL(url)
  },
  exportCases: async (
    projectId: string,
    body: { case_ids: string[]; results?: number; locale?: string },
  ) => {
    const token = getStoredToken()
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    }
    if (token) headers.Authorization = `Bearer ${token}`
    const csrf = getCsrfToken()
    if (csrf) headers['X-Vulyk-Csrf'] = csrf
    if (body.locale) headers['Accept-Language'] = body.locale
    const res = await fetch(
      `${API_BASE}/api/v1/projects/${projectId}/cases/export`,
      {
        method: 'POST',
        headers,
        credentials: 'include',
        body: JSON.stringify({
          case_ids: body.case_ids,
          results: body.results ?? 5,
          locale: body.locale,
        }),
      },
    )
    if (!res.ok) {
      throw new Error((await res.text()) || res.statusText)
    }
    const blob = await res.blob()
    const disposition = res.headers.get('content-disposition') ?? ''
    const match = /filename="?([^"]+)"?/.exec(disposition)
    const multi = body.case_ids.length > 1
    const filename =
      match?.[1] ?? (multi ? 'vulyk-cases-export.zip' : 'case.pdf')
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = filename
    a.click()
    URL.revokeObjectURL(url)
  },
  listVersions: (projectId: string, caseId: string) =>
    request<CaseVersionSummary[]>(
      `/api/v1/projects/${projectId}/cases/${caseId}/versions`,
    ),
  listCaseResults: (
    projectId: string,
    caseId: string,
    params: {
      run_id?: string
      status?: string
      user_id?: string
      from?: string
      to?: string
      limit?: number
      offset?: number
      include_steps?: boolean
    } = {},
  ) => {
    const qs = new URLSearchParams()
    if (params.run_id) qs.set('run_id', params.run_id)
    if (params.status) qs.set('status', params.status)
    if (params.user_id) qs.set('user_id', params.user_id)
    if (params.from) qs.set('from', params.from)
    if (params.to) qs.set('to', params.to)
    if (params.limit != null) qs.set('limit', String(params.limit))
    if (params.offset != null) qs.set('offset', String(params.offset))
    if (params.include_steps) qs.set('include_steps', 'true')
    const q = qs.toString()
    return request<CaseResultsPage>(
      `/api/v1/projects/${projectId}/cases/${caseId}/results${q ? `?${q}` : ''}`,
    )
  },
  listRunItemResults: (projectId: string, runId: string, itemId: string) =>
    request<TestResultHistoryItem[]>(
      `/api/v1/projects/${projectId}/runs/${runId}/items/${itemId}/results`,
    ),
  setCustomFields: (
    projectId: string,
    caseId: string,
    body: { field_id: string; value: unknown }[],
  ) =>
    request<TestCaseDetail>(
      `/api/v1/projects/${projectId}/cases/${caseId}/custom-fields`,
      { method: 'POST', body: JSON.stringify(body) },
    ),
  listTags: (projectId: string) =>
    request<Tag[]>(`/api/v1/projects/${projectId}/tags`),
  createTag: (projectId: string, body: { name: string; color?: string }) =>
    request<Tag>(`/api/v1/projects/${projectId}/tags`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  listCustomFields: (
    projectId: string,
    entity?: 'test_case' | 'requirement',
  ) => {
    const qs = entity ? `?entity=${encodeURIComponent(entity)}` : ''
    return request<CustomField[]>(
      `/api/v1/projects/${projectId}/custom-fields${qs}`,
    )
  },
  listAvailableCustomFields: (projectId: string) =>
    request<CustomFieldDefinition[]>(
      `/api/v1/projects/${projectId}/custom-fields/available`,
    ),
  addCustomFieldToProject: (
    projectId: string,
    fieldId: string,
    body: {
      required_override?: boolean
      default_value_override?: unknown
    } = {},
  ) =>
    request<CustomField>(
      `/api/v1/projects/${projectId}/custom-fields/${fieldId}/add`,
      { method: 'POST', body: JSON.stringify(body) },
    ),
  upsertCustomFieldOverride: (
    projectId: string,
    fieldId: string,
    body: {
      required_override?: boolean | null
      default_value_override?: unknown
      enabled?: boolean
    },
  ) =>
    request<CustomFieldProjectOverride>(
      `/api/v1/projects/${projectId}/custom-fields/${fieldId}/override`,
      { method: 'PUT', body: JSON.stringify(body) },
    ),

  listAdminLists: () =>
    request<OptionList[]>('/api/v1/admin/lists'),
  getAdminList: (id: string) =>
    request<OptionListDetail>(`/api/v1/admin/lists/${id}`),
  createAdminList: (body: {
    key: string
    name: string
    description?: string
    active?: boolean
    items?: Partial<OptionListItem>[]
  }) =>
    request<OptionListDetail>('/api/v1/admin/lists', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  updateAdminList: (
    id: string,
    body: {
      key?: string
      name?: string
      description?: string
      active?: boolean
    },
  ) =>
    request<OptionList>(`/api/v1/admin/lists/${id}`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  deleteAdminList: (id: string) =>
    request<void>(`/api/v1/admin/lists/${id}`, { method: 'DELETE' }),
  createAdminListItem: (
    listId: string,
    body: {
      label: string
      key?: string
      value?: string
      colour?: string
      description?: string
      sort_order?: number
      active?: boolean
      deprecated?: boolean
    },
  ) =>
    request<OptionListItem>(`/api/v1/admin/lists/${listId}/items`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  updateAdminListItem: (
    listId: string,
    itemId: string,
    body: Partial<{
      label: string
      key: string
      value: string
      colour: string
      description: string
      sort_order: number
      active: boolean
      deprecated: boolean
    }>,
  ) =>
    request<OptionListItem>(
      `/api/v1/admin/lists/${listId}/items/${itemId}`,
      { method: 'PUT', body: JSON.stringify(body) },
    ),
  deleteAdminListItem: (listId: string, itemId: string) =>
    request<void>(`/api/v1/admin/lists/${listId}/items/${itemId}`, {
      method: 'DELETE',
    }),
  reorderAdminListItems: (listId: string, itemIds: string[]) =>
    request<OptionListItem[]>(
      `/api/v1/admin/lists/${listId}/items/reorder`,
      { method: 'PUT', body: JSON.stringify({ item_ids: itemIds }) },
    ),

  listAdminCustomFields: () =>
    request<CustomFieldDefinitionDetail[]>('/api/v1/admin/custom-fields'),
  getAdminCustomField: (fieldId: string) =>
    request<CustomFieldDefinitionDetail>(
      `/api/v1/admin/custom-fields/${fieldId}`,
    ),
  createAdminCustomField: (body: CreateCustomFieldDefinition) =>
    request<CustomFieldDefinitionDetail>('/api/v1/admin/custom-fields', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  updateAdminCustomField: (
    fieldId: string,
    body: UpdateCustomFieldDefinition,
  ) =>
    request<UpdateFieldResponse>(
      `/api/v1/admin/custom-fields/${fieldId}`,
      { method: 'PUT', body: JSON.stringify(body) },
    ),
  deleteAdminCustomField: (fieldId: string) =>
    request<{ action: string }>(`/api/v1/admin/custom-fields/${fieldId}`, {
      method: 'DELETE',
    }),

  exportAdminConfig: async (body: ConfigExportRequest) => {
    const token = getStoredToken()
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    }
    if (token) headers.Authorization = `Bearer ${token}`
    const csrf = getCsrfToken()
    if (csrf) headers['X-Vulyk-Csrf'] = csrf
    const res = await fetch(`${API_BASE}/api/v1/admin/config/export`, {
      method: 'POST',
      headers,
      credentials: 'include',
      body: JSON.stringify(body),
    })
    if (!res.ok) {
      throw new Error((await res.text()) || res.statusText)
    }
    const blob = await res.blob()
    const disposition = res.headers.get('content-disposition') ?? ''
    const match = /filename="?([^"]+)"?/.exec(disposition)
    const ext = body.format === 'yaml' || body.format === 'yml' ? 'yaml' : 'json'
    const filename = match?.[1] ?? `vulyk-config.${ext}`
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = filename
    a.click()
    URL.revokeObjectURL(url)
  },
  previewAdminConfigImport: (body: {
    bundle: unknown
    strategy?: string
    secrets_passphrase?: string | null
  }) =>
    request<ConfigImportPreview>('/api/v1/admin/config/import/preview', {
      method: 'POST',
      body: JSON.stringify({ strategy: 'merge', ...body }),
    }),
  importAdminConfig: (body: {
    bundle: unknown
    strategy?: string
    dry_run?: boolean
    secrets_passphrase?: string | null
  }) =>
    request<ConfigImportPreview>('/api/v1/admin/config/import', {
      method: 'POST',
      body: JSON.stringify({ strategy: 'merge', dry_run: false, ...body }),
    }),
  listPlans: (projectId: string) =>
    request<TestPlan[]>(`/api/v1/projects/${projectId}/plans`),
  createPlan: (
    projectId: string,
    body: { name: string; description?: string; case_ids?: string[] },
  ) =>
    request<TestPlanDetail>(`/api/v1/projects/${projectId}/plans`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  listRuns: (
    projectId: string,
    limit = 50,
    offset = 0,
    opts?: ApiFetchOptions,
  ) =>
    request<Page<TestRun>>(
      `/api/v1/projects/${projectId}/runs?limit=${limit}&offset=${offset}`,
      opts?.signal ? { signal: opts.signal } : undefined,
    ),
  getRun: (projectId: string, runId: string, opts?: ApiFetchOptions) =>
    request<TestRunDetail>(
      `/api/v1/projects/${projectId}/runs/${runId}`,
      opts?.signal ? { signal: opts.signal } : undefined,
    ),
  createRun: (projectId: string, body: Record<string, unknown>) =>
    request<TestRunDetail>(`/api/v1/projects/${projectId}/runs`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  updateRun: (projectId: string, runId: string, body: Record<string, unknown>) =>
    request<TestRunDetail>(`/api/v1/projects/${projectId}/runs/${runId}`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  getRunItem: (projectId: string, runId: string, itemId: string) =>
    request<RunItemExecution>(
      `/api/v1/projects/${projectId}/runs/${runId}/items/${itemId}`,
    ),
  updateRunItem: (
    projectId: string,
    runId: string,
    itemId: string,
    body: Record<string, unknown>,
  ) =>
    request<RunItemView>(
      `/api/v1/projects/${projectId}/runs/${runId}/items/${itemId}`,
      { method: 'PUT', body: JSON.stringify(body) },
    ),
  createResult: (
    projectId: string,
    runId: string,
    body: Record<string, unknown>,
  ) =>
    request<TestResultDetail>(
      `/api/v1/projects/${projectId}/runs/${runId}/results/execute`,
      { method: 'POST', body: JSON.stringify(body) },
    ),
  createAttachment: (
    projectId: string,
    body: {
      entity_type: string
      entity_id: string
      filename: string
      content_type?: string
      content_base64: string
    },
  ) =>
    request<AttachmentMeta>(`/api/v1/projects/${projectId}/attachments`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  listAttachments: (
    projectId: string,
    params: { entity_type?: string; entity_id?: string } = {},
  ) => {
    const qs = new URLSearchParams()
    if (params.entity_type) qs.set('entity_type', params.entity_type)
    if (params.entity_id) qs.set('entity_id', params.entity_id)
    const q = qs.toString()
    return request<AttachmentMeta[]>(
      `/api/v1/projects/${projectId}/attachments${q ? `?${q}` : ''}`,
    )
  },

  executionsTree: (projectId: string) =>
    request<ExecutionsTreeNode[]>(
      `/api/v1/projects/${projectId}/executions/tree`,
    ),
  listCampaigns: (projectId: string) =>
    request<Campaign[]>(`/api/v1/projects/${projectId}/campaigns`),
  createCampaign: (projectId: string, body: CreateCampaign) =>
    request<Campaign>(`/api/v1/projects/${projectId}/campaigns`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  getCampaign: (projectId: string, campaignId: string) =>
    request<Campaign>(
      `/api/v1/projects/${projectId}/campaigns/${campaignId}`,
    ),
  updateCampaign: (
    projectId: string,
    campaignId: string,
    body: UpdateCampaign,
  ) =>
    request<Campaign>(
      `/api/v1/projects/${projectId}/campaigns/${campaignId}`,
      { method: 'PUT', body: JSON.stringify(body) },
    ),
  deleteCampaign: (projectId: string, campaignId: string) =>
    request<void>(
      `/api/v1/projects/${projectId}/campaigns/${campaignId}`,
      { method: 'DELETE' },
    ),
  listIterations: (projectId: string, campaignId?: string) => {
    const qs = campaignId
      ? `?campaign_id=${encodeURIComponent(campaignId)}`
      : ''
    return request<Iteration[]>(
      `/api/v1/projects/${projectId}/iterations${qs}`,
    )
  },
  createIteration: (
    projectId: string,
    campaignId: string,
    body: CreateIteration,
  ) =>
    request<Iteration>(
      `/api/v1/projects/${projectId}/campaigns/${campaignId}/iterations`,
      { method: 'POST', body: JSON.stringify(body) },
    ),
  getIteration: (projectId: string, iterationId: string) =>
    request<Iteration>(
      `/api/v1/projects/${projectId}/iterations/${iterationId}`,
    ),
  updateIteration: (
    projectId: string,
    iterationId: string,
    body: UpdateIteration,
  ) =>
    request<Iteration>(
      `/api/v1/projects/${projectId}/iterations/${iterationId}`,
      { method: 'PUT', body: JSON.stringify(body) },
    ),
  deleteIteration: (projectId: string, iterationId: string) =>
    request<void>(
      `/api/v1/projects/${projectId}/iterations/${iterationId}`,
      { method: 'DELETE' },
    ),
  iterationDashboard: (projectId: string, iterationId: string) =>
    request<IterationDashboard>(
      `/api/v1/projects/${projectId}/iterations/${iterationId}/dashboard`,
    ),
  listIterationSuites: (
    projectId: string,
    iterationId: string,
    params: AutomatedSuitesParams = {},
  ) => {
    const qs = new URLSearchParams()
    if (params.status) qs.set('status', params.status)
    if (params.created_by) qs.set('created_by', params.created_by)
    if (params.q) qs.set('q', params.q)
    qs.set('limit', String(params.limit ?? 25))
    qs.set('offset', String(params.offset ?? 0))
    qs.set('sort', params.sort ?? 'created_at_desc')
    return request<Page<AutomatedSuite>>(
      `/api/v1/projects/${projectId}/iterations/${iterationId}/suites?${qs}`,
    )
  },
  listSuiteResults: (
    projectId: string,
    runId: string,
    params: { limit?: number; offset?: number } = {},
  ) => {
    const qs = new URLSearchParams()
    qs.set('limit', String(params.limit ?? 50))
    qs.set('offset', String(params.offset ?? 0))
    return request<Page<SuiteResultRow>>(
      `/api/v1/projects/${projectId}/suites-runs/${runId}/results?${qs}`,
    )
  },
  deleteSuiteRun: (projectId: string, runId: string) =>
    request<void>(
      `/api/v1/projects/${projectId}/suites-runs/${runId}`,
      { method: 'DELETE' },
    ),
  runInCi: (
    projectId: string,
    body: {
      repo_id?: string
      case_ids?: string[]
      case_keys?: string[]
      source_run_id?: string
      run_name?: string
    },
  ) =>
    request<{
      run: TestRun
      dispatch: CiDispatch
      case_keys: string[]
    }>(`/api/v1/projects/${projectId}/run-in-ci`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  rerunSuite: (projectId: string, runId: string) =>
    request<AutomatedSuite>(
      `/api/v1/projects/${projectId}/suites-runs/${runId}`,
      { method: 'POST' },
    ),
  listCaseExecutions: (
    projectId: string,
    caseId: string,
    params: { limit?: number; offset?: number } = {},
  ) => {
    const qs = new URLSearchParams()
    qs.set('limit', String(params.limit ?? 25))
    qs.set('offset', String(params.offset ?? 0))
    return request<CaseExecutionHistoryPage>(
      `/api/v1/projects/${projectId}/cases/${caseId}/executions?${qs}`,
    )
  },

  listRequirements: (projectId: string) =>
    request<Requirement[]>(`/api/v1/projects/${projectId}/requirements`),
  getRequirement: (projectId: string, requirementId: string) =>
    request<Requirement>(
      `/api/v1/projects/${projectId}/requirements/${requirementId}`,
    ),
  createRequirement: (
    projectId: string,
    body: { title: string; description?: string; source?: string },
  ) =>
    request<Requirement>(`/api/v1/projects/${projectId}/requirements`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  setRequirementCustomFields: (
    projectId: string,
    requirementId: string,
    body: { field_id: string; value: unknown }[],
  ) =>
    request<Requirement>(
      `/api/v1/projects/${projectId}/requirements/${requirementId}/custom-fields`,
      { method: 'POST', body: JSON.stringify(body) },
    ),
  linkRequirement: (
    projectId: string,
    body: { case_id: string; requirement_id: string },
  ) =>
    request(`/api/v1/projects/${projectId}/requirements:link`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  listCaseRequirements: (projectId: string, caseId: string) =>
    request<Requirement[]>(
      `/api/v1/projects/${projectId}/cases/${caseId}/requirements`,
    ),
  listImporters: () => request<ImporterInfo[]>('/api/v1/importers'),
  previewImport: (
    projectId: string,
    body: {
      importer_id: string
      format: string
      content: string
      field_mapping?: FieldMapping
    },
  ) =>
    request<ImportPreview>(`/api/v1/projects/${projectId}/imports:preview`, {
      method: 'POST',
      body: JSON.stringify({
        field_mapping: defaultMapping(),
        ...body,
      }),
    }),
  runImport: (
    projectId: string,
    body: {
      importer_id: string
      format: string
      content: string
      field_mapping?: FieldMapping
      dry_run?: boolean
    },
  ) =>
    request<ImportPreview | ImportReport>(
      `/api/v1/projects/${projectId}/imports`,
      {
        method: 'POST',
        body: JSON.stringify({
          field_mapping: defaultMapping(),
          dry_run: false,
          ...body,
        }),
      },
    ),
  listIntegrations: (projectId: string) =>
    request<IntegrationPublic[]>(
      `/api/v1/projects/${projectId}/integrations`,
    ),
  listDefects: (projectId: string) =>
    request<Defect[]>(`/api/v1/projects/${projectId}/defects`),

  listLlmProviders: () => request<ProviderKindInfo[]>('/api/v1/llm/providers'),
  listLlmConfigs: (projectId: string) =>
    request<LlmConfig[]>(`/api/v1/projects/${projectId}/llm/configs`),
  createLlmConfig: (
    projectId: string,
    body: {
      provider: string
      name: string
      model?: string
      base_url?: string
      api_key?: string
      is_default?: boolean
      global?: boolean
    },
  ) =>
    request<LlmConfig>(`/api/v1/projects/${projectId}/llm/configs`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  deleteLlmConfig: (projectId: string, configId: string) =>
    request<void>(`/api/v1/projects/${projectId}/llm/configs/${configId}`, {
      method: 'DELETE',
    }),
  generateCases: (
    projectId: string,
    body: {
      requirements_text: string
      count?: number
      persist?: boolean
      suite_id?: string
      config_id?: string
    },
  ) =>
    request<{
      provider: string
      model: string
      drafts: GeneratedCaseDraft[]
      persisted: TestCaseDetail[]
    }>(`/api/v1/projects/${projectId}/llm/generate-cases`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  generateCode: (
    projectId: string,
    body: {
      case_ids: string[]
      framework: string
      config_id?: string
      repo_id?: string
    },
  ) =>
    request<{
      provider: string
      model: string
      framework: string
      code: string
      context_files: ContextFileRef[]
    }>(`/api/v1/projects/${projectId}/llm/generate-code`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  askTests: (projectId: string, question: string) =>
    request<{ provider: string; model: string; answer: string }>(
      `/api/v1/projects/${projectId}/llm/ask`,
      { method: 'POST', body: JSON.stringify({ question }) },
    ),
  chatPropose: (
    projectId: string,
    body: {
      message: string
      history?: { role: string; content: string }[]
      config_id?: string
    },
  ) =>
    request<ChatProposeResponse>(
      `/api/v1/projects/${projectId}/llm/chat`,
      { method: 'POST', body: JSON.stringify(body) },
    ),
  chatConfirm: (
    projectId: string,
    body: {
      proposal: ChatProposal
      config_id?: string
      provider?: string
      model?: string
    },
  ) =>
    request<ChatConfirmResponse>(
      `/api/v1/projects/${projectId}/llm/chat/confirm`,
      { method: 'POST', body: JSON.stringify(body) },
    ),
  listRequirementFolders: (projectId: string) =>
    request<RequirementFolder[]>(
      `/api/v1/projects/${projectId}/requirement-folders`,
    ),
  createRequirementFolder: (
    projectId: string,
    body: { name: string; parent_id?: string | null; sort_order?: number },
  ) =>
    request<RequirementFolder>(
      `/api/v1/projects/${projectId}/requirement-folders`,
      { method: 'POST', body: JSON.stringify(body) },
    ),

  /** App-level multi-repo automation connections. */
  listAutomationRepos: () =>
    request<AutomationRepo[]>('/api/v1/automation-repos'),
  createAutomationRepo: (body: CreateAutomationRepoBody) =>
    request<AutomationRepo>('/api/v1/automation-repos', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  getAutomationRepoById: (id: string) =>
    request<AutomationRepo>(`/api/v1/automation-repos/${id}`),
  updateAutomationRepo: (id: string, body: UpdateAutomationRepoBody) =>
    request<AutomationRepo>(`/api/v1/automation-repos/${id}`, {
      method: 'PATCH',
      body: JSON.stringify(body),
    }),
  deleteAutomationRepoById: (id: string) =>
    request<void>(`/api/v1/automation-repos/${id}`, { method: 'DELETE' }),
  setAutomationRepoProjects: (
    id: string,
    body: SetAutomationRepoProjectsBody,
  ) =>
    request<AutomationRepo>(`/api/v1/automation-repos/${id}/projects`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  listAutomationReposForProject: (projectId: string) =>
    request<AutomationRepo[]>(
      `/api/v1/projects/${projectId}/automation-repos`,
    ),
  testAutomationRepoPreview: (body: {
    provider: string
    remote_url: string
    branch?: string
    subpath?: string
    token?: string
    project_ids?: string[]
  }) =>
    request<{ ok: boolean; message: string; commit_sha: string | null }>(
      `/api/v1/automation-repos/test`,
      { method: 'POST', body: JSON.stringify(body) },
    ),
  testAutomationRepoById: (id: string) =>
    request<{ ok: boolean; message: string; commit_sha: string | null }>(
      `/api/v1/automation-repos/${id}/test`,
      { method: 'POST', body: '{}' },
    ),
  reindexAutomationRepoById: (id: string) =>
    request<{
      status: string
      commit_sha: string | null
      file_count: number
      chunk_count: number
      error: string | null
    }>(`/api/v1/automation-repos/${id}/reindex`, {
      method: 'POST',
      body: '{}',
    }),

  /** Legacy singular project-scoped automation repo (shim). */
  getAutomationRepo: (projectId: string) =>
    request<AutomationRepo>(`/api/v1/projects/${projectId}/automation-repo`),
  upsertAutomationRepo: (
    projectId: string,
    body: {
      provider: string
      remote_url: string
      branch?: string
      subpath?: string
      token?: string
      name?: string
    },
  ) =>
    request<AutomationRepo>(`/api/v1/projects/${projectId}/automation-repo`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  deleteAutomationRepo: (projectId: string) =>
    request<void>(`/api/v1/projects/${projectId}/automation-repo`, {
      method: 'DELETE',
    }),
  testAutomationRepo: (projectId: string) =>
    request<{ ok: boolean; message: string; commit_sha: string | null }>(
      `/api/v1/projects/${projectId}/automation-repo/test`,
      { method: 'POST', body: '{}' },
    ),
  reindexAutomationRepo: (projectId: string) =>
    request<{
      status: string
      commit_sha: string | null
      file_count: number
      chunk_count: number
      error: string | null
    }>(`/api/v1/projects/${projectId}/automation-repo/reindex`, {
      method: 'POST',
      body: '{}',
    }),

  listProjectCiTokens: (projectId: string) =>
    request<ProjectCiToken[]>(`/api/v1/projects/${projectId}/ci-tokens`),
  createProjectCiToken: (projectId: string, name: string) =>
    request<{ token: ProjectCiToken; secret: string }>(
      `/api/v1/projects/${projectId}/ci-tokens`,
      { method: 'POST', body: JSON.stringify({ name }) },
    ),
  revokeProjectCiToken: (projectId: string, tokenId: string) =>
    request<void>(`/api/v1/projects/${projectId}/ci-tokens/${tokenId}`, {
      method: 'DELETE',
    }),

  listTokens: () => request<ApiTokenPublic[]>('/api/v1/tokens'),
  createToken: (name: string) =>
    request<{ token: ApiTokenPublic; secret: string }>('/api/v1/tokens', {
      method: 'POST',
      body: JSON.stringify({ name }),
    }),
  revokeToken: (id: string) =>
    request<void>(`/api/v1/tokens/${id}`, { method: 'DELETE' }),

  listResults: (runId: string, opts?: { limit?: number; offset?: number }) => {
    const qs = new URLSearchParams()
    qs.set('limit', String(opts?.limit ?? 500))
    if (opts?.offset != null) qs.set('offset', String(opts.offset))
    return request<Page<TestResult>>(`/api/v1/runs/${runId}/results?${qs}`)
  },
  ingestRun: (
    runId: string,
    body: {
      format: string
      content: string
      async_mode?: boolean
      source?: string
    },
  ) =>
    request<{
      inserted: number
      updated: number
      unmatched_case_keys: string[]
      job_id: string | null
      parsed: number
    }>(`/api/v1/runs/${runId}/ingest`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  listCiConfigs: (projectId: string) =>
    request<CiConfig[]>(`/api/v1/projects/${projectId}/ci-configs`),
  createCiConfig: (
    projectId: string,
    body: {
      provider: string
      name: string
      config: Record<string, unknown>
      webhook_secret?: string
    },
  ) =>
    request<CiConfig>(`/api/v1/projects/${projectId}/ci-configs`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  dispatchCi: (
    configId: string,
    body?: { inputs?: Record<string, unknown>; run_name?: string },
  ) =>
    request<CiDispatch>(`/api/v1/ci-configs/${configId}/dispatch`, {
      method: 'POST',
      body: JSON.stringify(body ?? {}),
    }),
  listDispatches: (projectId: string) =>
    request<CiDispatch[]>(`/api/v1/projects/${projectId}/ci-dispatches`),

  getDashboard: (projectId: string) =>
    request<ProjectDashboard>(
      `/api/v1/projects/${projectId}/reports/dashboard`,
    ),
  getRunSummary: (projectId: string, runId: string) =>
    request<RunSummaryReport>(
      `/api/v1/projects/${projectId}/reports/runs/${runId}/summary`,
    ),
  getRunReportDetail: (projectId: string, runId: string) =>
    request<RunReportDetail>(
      `/api/v1/projects/${projectId}/reports/runs/${runId}`,
    ),
  getRunCaseResults: (projectId: string, runId: string, params: RunResultsParams = {}) => {
    const qs = new URLSearchParams()
    if (params.status) qs.set('status', params.status)
    if (params.folder) qs.set('folder', params.folder)
    if (params.suite_id) qs.set('suite_id', params.suite_id)
    if (params.priority) qs.set('priority', params.priority)
    if (params.assignee) qs.set('assignee', params.assignee)
    if (params.q) qs.set('q', params.q)
    if (params.sort) qs.set('sort', params.sort)
    qs.set('limit', String(params.limit ?? 50))
    qs.set('offset', String(params.offset ?? 0))
    return request<RunCaseResultsPage>(
      `/api/v1/projects/${projectId}/reports/runs/${runId}/results?${qs}`,
    )
  },
  getRunCaseExpand: (projectId: string, runId: string, caseId: string) =>
    request<RunCaseResultExpand>(
      `/api/v1/projects/${projectId}/reports/runs/${runId}/cases/${caseId}`,
    ),
  getRunResultGroups: (
    projectId: string,
    runId: string,
    params: Omit<RunResultsParams, 'sort' | 'limit' | 'offset'> & {
      group_by?: 'folder' | 'status'
    } = {},
  ) => {
    const qs = new URLSearchParams()
    if (params.group_by) qs.set('group_by', params.group_by)
    if (params.status) qs.set('status', params.status)
    if (params.folder) qs.set('folder', params.folder)
    if (params.suite_id) qs.set('suite_id', params.suite_id)
    if (params.priority) qs.set('priority', params.priority)
    if (params.assignee) qs.set('assignee', params.assignee)
    if (params.q) qs.set('q', params.q)
    return request<RunResultGroups>(
      `/api/v1/projects/${projectId}/reports/runs/${runId}/groups?${qs}`,
    )
  },
  getTrends: (projectId: string, granularity: 'run' | 'day' = 'run', limit = 20) =>
    request<PassFailTrends>(
      `/api/v1/projects/${projectId}/reports/trends?granularity=${granularity}&limit=${limit}`,
    ),
  getCoverage: (projectId: string) =>
    request<RequirementCoverageReport>(
      `/api/v1/projects/${projectId}/reports/coverage`,
    ),
  getCoverageDetail: (projectId: string) =>
    request<RequirementCoverageDetail>(
      `/api/v1/projects/${projectId}/reports/coverage/detail`,
    ),
  getFlaky: (projectId: string, lookbackRuns = 20, limit = 50) =>
    request<FlakyReport>(
      `/api/v1/projects/${projectId}/reports/flaky?lookback_runs=${lookbackRuns}&limit=${limit}`,
    ),
  downloadReportExport: async (
    projectId: string,
    opts: {
      format: 'csv' | 'pdf'
      report?: string
      runId?: string
      status?: string
      folder?: string
      priority?: string
      assignee?: string
      q?: string
      sort?: string
    },
  ) => {
    const qs = new URLSearchParams({
      format: opts.format,
      report: opts.report ?? 'dashboard',
    })
    if (opts.runId) qs.set('run_id', opts.runId)
    if (opts.status) qs.set('status', opts.status)
    if (opts.folder) qs.set('folder', opts.folder)
    if (opts.priority) qs.set('priority', opts.priority)
    if (opts.assignee) qs.set('assignee', opts.assignee)
    if (opts.q) qs.set('q', opts.q)
    if (opts.sort) qs.set('sort', opts.sort)
    const token = getStoredToken()
    const headers: Record<string, string> = {}
    if (token) headers.Authorization = `Bearer ${token}`
    const csrf = getCsrfToken()
    if (csrf) headers['X-Vulyk-Csrf'] = csrf
    const res = await fetch(
      `${API_BASE}/api/v1/projects/${projectId}/reports/export?${qs}`,
      { headers, credentials: 'include' },
    )
    if (!res.ok) {
      throw new Error((await res.text()) || res.statusText)
    }
    const blob = await res.blob()
    const disposition = res.headers.get('content-disposition') ?? ''
    const match = /filename="?([^"]+)"?/.exec(disposition)
    const filename =
      match?.[1] ??
      `vulyk-report.${opts.format === 'pdf' ? 'pdf' : 'csv'}`
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = filename
    a.click()
    URL.revokeObjectURL(url)
  },
}
