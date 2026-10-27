import { describe, expect, it, vi } from 'vitest'
import { createRequire } from 'node:module'
import { assertPwaArtifacts } from './pwa-gates.mjs'

const require = createRequire(import.meta.url)

describe('lighthouse M14 gates', () => {
  it('requires lighthouse ≥13.5', () => {
    const pkg = require('lighthouse/package.json')
    const [maj, min] = pkg.version.split('.').map((n) => Number(n))
    expect(maj).toBeGreaterThanOrEqual(13)
    if (maj === 13) expect(min).toBeGreaterThanOrEqual(5)
  })

  it('assertPwaArtifacts accepts a valid preview payload', async () => {
    const manifest = {
      name: 'Vulyk',
      short_name: 'Vulyk',
      start_url: '/',
      display: 'standalone',
      theme_color: '#0B6E4F',
      icons: [
        { src: '/icons/icon-192.png', sizes: '192x192', type: 'image/png' },
        {
          src: '/icons/icon-512.png',
          sizes: '512x512',
          type: 'image/png',
          purpose: 'any maskable',
        },
      ],
    }
    const index = `<!doctype html><html><head>
      <link rel="manifest" href="/manifest.webmanifest" />
      <script type="module">import {registerSW} from 'virtual:pwa-register'</script>
    </head><body></body></html>`

    const fetchFn = vi.fn(async (url) => {
      if (url.endsWith('/') || url.endsWith('/index.html')) {
        return new Response(index, { status: 200 })
      }
      if (url.includes('manifest.webmanifest')) {
        return new Response(JSON.stringify(manifest), {
          status: 200,
          headers: { 'content-type': 'application/manifest+json' },
        })
      }
      return new Response('missing', { status: 404 })
    })

    const result = await assertPwaArtifacts('http://127.0.0.1:5173', { fetchFn })
    expect(result.ok).toBe(true)
    expect(result.failures).toEqual([])
  })

  it('assertPwaArtifacts fails when maskable icon is missing', async () => {
    const manifest = {
      name: 'Vulyk',
      short_name: 'Vulyk',
      start_url: '/',
      display: 'standalone',
      theme_color: '#0B6E4F',
      icons: [{ src: '/icons/icon-192.png', sizes: '192x192', type: 'image/png' }],
    }
    const index = `<!doctype html><html><head>
      <link rel="manifest" href="/manifest.webmanifest" />
    </head><body></body></html>`

    const fetchFn = vi.fn(async (url) => {
      if (url.endsWith('/')) return new Response(index, { status: 200 })
      if (url.includes('manifest.webmanifest')) {
        return new Response(JSON.stringify(manifest), { status: 200 })
      }
      return new Response('missing', { status: 404 })
    })

    const result = await assertPwaArtifacts('http://example.test', { fetchFn })
    expect(result.ok).toBe(false)
    expect(result.failures.some((f) => f.includes('maskable'))).toBe(true)
  })
})
