import { describe, expect, it } from 'vitest'
import { api } from './api'

describe('api client', () => {
  it('exposes listProjects', () => {
    expect(typeof api.listProjects).toBe('function')
  })

  it('exposes case depth and run helpers', () => {
    expect(typeof api.listCases).toBe('function')
    expect(typeof api.bulkCases).toBe('function')
    expect(typeof api.listVersions).toBe('function')
    expect(typeof api.createRun).toBe('function')
    expect(typeof api.getRunItem).toBe('function')
    expect(typeof api.createResult).toBe('function')
  })
})
