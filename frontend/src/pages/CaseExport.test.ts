import { afterEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api'

function stubBrowserGlobals() {
  const store = new Map<string, string>()
  const g = globalThis as typeof globalThis & {
    localStorage: Storage
    document: Document
  }
  g.localStorage = {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => {
      store.set(k, v)
    },
    removeItem: (k: string) => {
      store.delete(k)
    },
    clear: () => store.clear(),
    key: () => null,
    length: 0,
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  g.document = {
    createElement: () => ({
      href: '',
      download: '',
      click: () => undefined,
    }),
  } as any
  URL.createObjectURL = vi.fn(() => 'blob:mock') as typeof URL.createObjectURL
  URL.revokeObjectURL = vi.fn() as typeof URL.revokeObjectURL
}

describe('case PDF export API helpers', () => {
  const originalFetch = globalThis.fetch

  afterEach(() => {
    globalThis.fetch = originalFetch
    vi.restoreAllMocks()
  })

  it('exposes downloadCasePdf and exportCases', () => {
    expect(typeof api.downloadCasePdf).toBe('function')
    expect(typeof api.exportCases).toBe('function')
  })

  it('downloadCasePdf requests export.pdf with results and locale', async () => {
    stubBrowserGlobals()
    globalThis.fetch = vi.fn(async () => {
      return new Response(new Blob(['%PDF'], { type: 'application/pdf' }), {
        status: 200,
        headers: {
          'content-disposition': 'attachment; filename="VQA-001.pdf"',
        },
      })
    }) as typeof fetch

    await api.downloadCasePdf('proj-1', 'VQA-001', {
      results: 5,
      locale: 'uk',
    })

    expect(globalThis.fetch).toHaveBeenCalled()
    const [url, init] = (globalThis.fetch as ReturnType<typeof vi.fn>).mock
      .calls[0] as [string, RequestInit]
    expect(url).toContain('/api/v1/projects/proj-1/cases/VQA-001/export.pdf')
    expect(url).toContain('results=5')
    expect(url).toContain('locale=uk')
    expect((init.headers as Record<string, string>)['Accept-Language']).toBe(
      'uk',
    )
  })

  it('exportCases posts bulk export body', async () => {
    stubBrowserGlobals()
    globalThis.fetch = vi.fn(async () => {
      return new Response(new Uint8Array([0x50, 0x4b]), {
        status: 200,
        headers: {
          'content-type': 'application/zip',
          'content-disposition':
            'attachment; filename="vulyk-cases-export.zip"',
        },
      })
    }) as typeof fetch

    await api.exportCases('proj-1', {
      case_ids: ['a', 'b'],
      results: 5,
      locale: 'en',
    })

    const [url, init] = (globalThis.fetch as ReturnType<typeof vi.fn>).mock
      .calls[0] as [string, RequestInit]
    expect(url).toContain('/api/v1/projects/proj-1/cases/export')
    expect(init.method).toBe('POST')
    const body = JSON.parse(String(init.body))
    expect(body.case_ids).toEqual(['a', 'b'])
    expect(body.results).toBe(5)
  })
})
