/**
 * Generate bee-themed PWA icons (any + maskable) from an inline SVG.
 * Run: node scripts/generate-icons.mjs
 */
import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import sharp from 'sharp'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const outDir = path.join(__dirname, '..', 'public', 'icons')

const SIZES = [72, 96, 128, 144, 152, 192, 384, 512]

function beeSvg(size, { maskable = false } = {}) {
  // Safe zone for maskable: keep artwork in the center ~70%
  const pad = maskable ? size * 0.15 : size * 0.08
  const inner = size - pad * 2
  const cx = size / 2
  const cy = size / 2
  const s = inner / 100

  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">
  <rect width="${size}" height="${size}" fill="#0f6b4c"/>
  <g transform="translate(${cx}, ${cy}) scale(${s})">
    <!-- wings -->
    <ellipse cx="-28" cy="-18" rx="22" ry="14" fill="#e7f3ec" opacity="0.92" transform="rotate(-28 -28 -18)"/>
    <ellipse cx="28" cy="-18" rx="22" ry="14" fill="#e7f3ec" opacity="0.92" transform="rotate(28 28 -18)"/>
    <!-- body -->
    <ellipse cx="0" cy="6" rx="26" ry="32" fill="#f2c14e"/>
    <!-- stripes -->
    <rect x="-26" y="-8" width="52" height="9" fill="#1c1914" opacity="0.88"/>
    <rect x="-24" y="8" width="48" height="9" fill="#1c1914" opacity="0.88"/>
    <!-- head -->
    <circle cx="0" cy="-30" r="16" fill="#1c1914"/>
    <!-- antennae -->
    <path d="M-8 -40 Q-18 -55 -22 -58" stroke="#1c1914" stroke-width="3" fill="none" stroke-linecap="round"/>
    <path d="M8 -40 Q18 -55 22 -58" stroke="#1c1914" stroke-width="3" fill="none" stroke-linecap="round"/>
    <circle cx="-22" cy="-58" r="3.5" fill="#f2c14e"/>
    <circle cx="22" cy="-58" r="3.5" fill="#f2c14e"/>
    <!-- eyes -->
    <circle cx="-6" cy="-32" r="3" fill="#e7f3ec"/>
    <circle cx="6" cy="-32" r="3" fill="#e7f3ec"/>
  </g>
</svg>`
}

async function writePng(name, size, maskable) {
  const svg = beeSvg(size, { maskable })
  const buf = await sharp(Buffer.from(svg)).png().toBuffer()
  await writeFile(path.join(outDir, name), buf)
  console.log('wrote', name)
}

await mkdir(outDir, { recursive: true })

for (const size of SIZES) {
  await writePng(`icon-${size}.png`, size, false)
  await writePng(`icon-maskable-${size}.png`, size, true)
}

await writeFile(path.join(outDir, 'bee.svg'), beeSvg(512, { maskable: false }))
console.log('done')
