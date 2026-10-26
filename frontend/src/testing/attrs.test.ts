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
})
