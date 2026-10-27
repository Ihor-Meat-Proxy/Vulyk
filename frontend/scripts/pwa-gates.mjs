/**
 * Static PWA installability gates used after Lighthouse ≥12 removed the
 * scored `pwa` category (and its audits). Checks the built SPA over HTTP.
 */

/**
 * @param {string} baseUrl preview origin, no trailing slash
 * @param {{ fetchFn?: typeof fetch }} [opts]
 * @returns {Promise<{ ok: boolean, failures: string[], notes: string[] }>}
 */
export async function assertPwaArtifacts(baseUrl, opts = {}) {
  const fetchFn = opts.fetchFn ?? fetch
  const root = baseUrl.replace(/\/$/, '')
  const failures = []
  const notes = []

  const indexRes = await fetchFn(`${root}/`)
  if (!indexRes.ok) {
    failures.push(`index.html HTTP ${indexRes.status}`)
    return { ok: false, failures, notes }
  }
  const indexHtml = await indexRes.text()

  const manifestHref =
    indexHtml.match(
      /<link[^>]+rel=["']manifest["'][^>]+href=["']([^"']+)["']/i,
    )?.[1] ??
    indexHtml.match(
      /<link[^>]+href=["']([^"']+)["'][^>]+rel=["']manifest["']/i,
    )?.[1] ??
    '/manifest.webmanifest'

  const manifestUrl = new URL(manifestHref, `${root}/`).href
  const manifestRes = await fetchFn(manifestUrl)
  if (!manifestRes.ok) {
    failures.push(`manifest ${manifestUrl} HTTP ${manifestRes.status}`)
    return { ok: false, failures, notes }
  }

  let manifest
  try {
    manifest = await manifestRes.json()
  } catch (e) {
    failures.push(`manifest JSON parse: ${e}`)
    return { ok: false, failures, notes }
  }

  for (const key of ['name', 'short_name', 'start_url', 'display', 'theme_color']) {
    if (!manifest[key]) failures.push(`manifest missing ${key}`)
  }
  if (!Array.isArray(manifest.icons) || manifest.icons.length === 0) {
    failures.push('manifest missing icons')
  } else {
    const hasMaskable = manifest.icons.some(
      (i) => typeof i.purpose === 'string' && i.purpose.includes('maskable'),
    )
    if (!hasMaskable) failures.push('manifest missing maskable icon')
    notes.push(`icons=${manifest.icons.length} maskable=${hasMaskable}`)
  }

  // vite-plugin-pwa injects registerSW / workbox into the built index.
  const hasSwHook =
    /registerSW|workbox|navigator\.serviceWorker/i.test(indexHtml) ||
    /<script[^>]+src=["'][^"']*sw[^"']*["']/i.test(indexHtml)
  if (!hasSwHook) {
    // Fall back: common emit paths
    const swCandidates = ['/sw.js', '/service-worker.js', '/dev-sw.js']
    let found = false
    for (const path of swCandidates) {
      const res = await fetchFn(new URL(path, `${root}/`).href)
      if (res.ok) {
        found = true
        notes.push(`service worker at ${path}`)
        break
      }
    }
    if (!found) failures.push('no service-worker registration found in index or /sw.js')
  } else {
    notes.push('service-worker hook present in index.html')
  }

  return { ok: failures.length === 0, failures, notes }
}
