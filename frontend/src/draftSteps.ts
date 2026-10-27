/** Editable case step row with a stable React key (server id or client uuid). */
export type DraftStep = {
  clientKey: string
  action: string
  expected_result: string
  data: string
}

export type ApiStepInput = {
  action: string
  expected_result: string
  data: string
}

function newClientKey(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID()
  }
  return `step-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}

export function emptyDraftStep(): DraftStep {
  return {
    clientKey: newClientKey(),
    action: '',
    expected_result: '',
    data: '',
  }
}

export function draftStepsFromApi(
  steps: Array<{
    id: string
    action: string
    expected_result: string
    data: string
  }>,
): DraftStep[] {
  return steps.map((s) => ({
    clientKey: s.id,
    action: s.action,
    expected_result: s.expected_result,
    data: s.data,
  }))
}

/** Strip client-only keys before PATCH/POST. */
export function stepsForApi(steps: DraftStep[]): ApiStepInput[] {
  return steps.map(({ action, expected_result, data }) => ({
    action,
    expected_result,
    data,
  }))
}
