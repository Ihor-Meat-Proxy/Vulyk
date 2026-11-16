import { describe, expect, it } from 'vitest'
import {
  campaignRef,
  executionRunPath,
  isEntityUuid,
  iterationPath,
  iterationRef,
  reportRunPath,
  runPath,
  runRef,
} from './runUrls'

describe('runUrls', () => {
  it('detects UUIDs', () => {
    expect(isEntityUuid('550e8400-e29b-41d4-a716-446655440000')).toBe(true)
    expect(isEntityUuid('VQA-R-42')).toBe(false)
  })

  it('prefers slug then display_id then id', () => {
    expect(
      runRef({
        id: 'uuid',
        slug: 'VQA-R-jenkins-42-20261116T163512Z',
        display_id: 'VQA-R-42',
      }),
    ).toBe('VQA-R-jenkins-42-20261116T163512Z')
    expect(runRef({ id: 'uuid', display_id: 'VQA-R-42' })).toBe('VQA-R-42')
    expect(runRef({ id: 'uuid' })).toBe('uuid')
  })

  it('builds paths', () => {
    const run = {
      id: 'uuid',
      slug: 'VQA-R-manual-1-20261116T000000Z',
      display_id: 'VQA-R-1',
    }
    expect(runPath(run)).toBe('/runs/VQA-R-manual-1-20261116T000000Z')
    expect(executionRunPath(run, 'case-1')).toBe(
      '/executions/runs/VQA-R-manual-1-20261116T000000Z/cases/case-1',
    )
    expect(reportRunPath(run)).toBe(
      '/reports/runs/VQA-R-manual-1-20261116T000000Z',
    )
    expect(iterationPath({ id: 'i', display_id: 'VQA-IT-17' })).toBe(
      '/executions/iterations/VQA-IT-17',
    )
    expect(iterationRef({ id: 'i' })).toBe('i')
    expect(campaignRef({ id: 'c', display_id: 'VQA-C-3' })).toBe('VQA-C-3')
  })
})
