import { afterEach, describe, expect, it, vi } from 'vitest'

describe('test id attrs', () => {
  afterEach(() => {
    vi.unstubAllEnvs()
    vi.resetModules()
  })

  it('emits data-testid by default', async () => {
    vi.stubEnv('VITE_STRIP_TEST_IDS', '')
    const { testIdProps, listItemProps } = await import('./attrs')
    expect(testIdProps('login-email-input')).toEqual({
      'data-testid': 'login-email-input',
    })
    expect(
      listItemProps('case-row', {
        entityId: 'uuid-1',
        entityKey: 'KOC-12',
        index: 0,
      }),
    ).toEqual({
      'data-testid': 'case-row',
      'data-entity-id': 'uuid-1',
      'data-entity-key': 'KOC-12',
      'data-index': '0',
    })
  })

  it('strips when VITE_STRIP_TEST_IDS=true', async () => {
    vi.stubEnv('VITE_STRIP_TEST_IDS', 'true')
    const { testIdProps, listItemProps, stripTestIdsEnabled } =
      await import('./attrs')
    expect(stripTestIdsEnabled()).toBe(true)
    expect(testIdProps('login-email-input')).toEqual({})
    expect(
      listItemProps('case-row', {
        entityId: 'uuid-1',
        entityKey: 'KOC-12',
        index: 0,
      }),
    ).toEqual({})
  })

  it('omits freeform entityKey (spaces) instead of crashing', async () => {
    vi.stubEnv('VITE_STRIP_TEST_IDS', '')
    const { listItemProps } = await import('./attrs')
    expect(
      listItemProps('run-row', {
        entityId: '550e8400-e29b-41d4-a716-446655440000',
        entityKey: 'Smoke run with spaces',
        index: 1,
      }),
    ).toEqual({
      'data-testid': 'run-row',
      'data-entity-id': '550e8400-e29b-41d4-a716-446655440000',
      'data-index': '1',
    })
  })
})
