/**
 * Generate honeycomb-V PWA icons (any + maskable) from the brand mark SVG.
 * Run: node scripts/generate-icons.mjs
 */
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import sharp from 'sharp'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const publicDir = path.join(__dirname, '..', 'public')
const outDir = path.join(publicDir, 'icons')
const markPath = path.join(publicDir, 'brand', 'vulyk-mark.svg')

/** Keep legacy PWA sizes plus the apple-touch / favicon PNGs requested. */
const SIZES = [32, 72, 96, 128, 144, 152, 180, 192, 384, 512]

const BG = '#7A5C18'

function honeycombSvg(size, { maskable = false } = {}) {
  // Safe zone for maskable: keep artwork in the center ~70%
  const pad = maskable ? size * 0.18 : size * 0.12
  const inner = size - pad * 2
  // Mark viewBox is ~72×65; fit inside the padded square.
  const markW = 71.96
  const markH = 65.0
  const scale = Math.min(inner / markW, inner / markH)
  const ox = (size - markW * scale) / 2
  const oy = (size - markH * scale) / 2 - 10 * scale // cancel mark viewBox y offset

  const hexes = [
    ['-25.98,-8.80 -18.36,-4.40 -18.36,4.40 -25.98,8.80 -33.60,4.40 -33.60,-4.40', '#E3BB72'],
    ['25.98,-8.80 33.60,-4.40 33.60,4.40 25.98,8.80 18.36,4.40 18.36,-4.40', '#D9A55A'],
    ['-17.32,6.20 -9.70,10.60 -9.70,19.40 -17.32,23.80 -24.94,19.40 -24.94,10.60', '#DDB067'],
    ['17.32,6.20 24.94,10.60 24.94,19.40 17.32,23.80 9.70,19.40 9.70,10.60', '#DDB067'],
    ['-8.66,21.20 -1.04,25.60 -1.04,34.40 -8.66,38.80 -16.28,34.40 -16.28,25.60', '#D6A256'],
    ['8.66,21.20 16.28,25.60 16.28,34.40 8.66,38.80 1.04,34.40 1.04,25.60', '#D6A256'],
    ['0.00,36.20 7.62,40.60 7.62,49.40 0.00,53.80 -7.62,49.40 -7.62,40.60', '#CF9849'],
  ]

  const polys = hexes
    .map(
      ([points, fill]) =>
        `<polygon points="${points}" fill="${fill}"/>`,
    )
    .join('')

  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">
  <rect width="${size}" height="${size}" rx="${Math.round(size * 0.18)}" fill="${BG}"/>
  <g transform="translate(${ox.toFixed(2)}, ${oy.toFixed(2)}) scale(${scale.toFixed(5)})">
    ${polys}
  </g>
</svg>`
}

async function writePng(name, size, maskable) {
  const svg = honeycombSvg(size, { maskable })
  const buf = await sharp(Buffer.from(svg)).png().toBuffer()
  await writeFile(path.join(outDir, name), buf)
  console.log('wrote', name)
}

await mkdir(outDir, { recursive: true })

// Ensure source mark exists (used by docs / static assets).
await readFile(markPath)

for (const size of SIZES) {
  await writePng(`icon-${size}.png`, size, false)
  if (size >= 72) {
    await writePng(`icon-maskable-${size}.png`, size, true)
  }
}

// Keep a square SVG favicon-style mark under icons/ for offline + legacy links.
await writeFile(
  path.join(outDir, 'mark.svg'),
  honeycombSvg(512, { maskable: false }),
)
// Back-compat: bee.svg historically pointed from index/offline — replace content.
await writeFile(
  path.join(outDir, 'bee.svg'),
  honeycombSvg(512, { maskable: false }),
)
console.log('done')
