import { describe, expect, it } from 'vitest'
import { api } from './api'

describe('api client', () => {
  it('exposes listProjects', () => {
    expect(typeof api.listProjects).toBe('function')
  })
})
