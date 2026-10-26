import { describe, expect, it } from 'vitest'
import {
  draftStepsFromApi,
  emptyDraftStep,
  stepsForApi,
} from './draftSteps'

describe('draftSteps', () => {
  it('assigns distinct client keys for empty steps', () => {
    const a = emptyDraftStep()
    const b = emptyDraftStep()
    expect(a.clientKey).toBeTruthy()
    expect(b.clientKey).toBeTruthy()
    expect(a.clientKey).not.toBe(b.clientKey)
  })

  it('reuses server step ids as client keys', () => {
    const drafts = draftStepsFromApi([
      {
        id: 'step-uuid-1',
        action: 'Open',
        expected_result: 'Visible',
        data: '',
      },
    ])
    expect(drafts[0]?.clientKey).toBe('step-uuid-1')
    expect(stepsForApi(drafts)).toEqual([
      { action: 'Open', expected_result: 'Visible', data: '' },
    ])
  })

  it('omits clientKey from API payload', () => {
    const step = emptyDraftStep()
    step.action = 'Click'
    const payload = stepsForApi([step])
    expect(payload[0]).toEqual({
      action: 'Click',
      expected_result: '',
      data: '',
    })
    expect(payload[0]).not.toHaveProperty('clientKey')
  })
})
