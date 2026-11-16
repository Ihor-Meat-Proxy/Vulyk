import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'
import { VitePWA } from 'vite-plugin-pwa'

const rootDir = dirname(fileURLToPath(import.meta.url))
const pkgVersion = (
  JSON.parse(readFileSync(join(rootDir, 'package.json'), 'utf8')) as {
    version: string
  }
).version

const iconSizes = [72, 96, 128, 144, 152, 180, 192, 384, 512] as const
const maskableSizes = [72, 96, 128, 144, 152, 180, 192, 384, 512] as const

const icons = [
  ...iconSizes.map((size) => ({
    src: `/icons/icon-${size}.png`,
    sizes: `${size}x${size}`,
    type: 'image/png',
    purpose: 'any' as const,
  })),
  ...maskableSizes.map((size) => ({
    src: `/icons/icon-maskable-${size}.png`,
    sizes: `${size}x${size}`,
    type: 'image/png',
    purpose: 'maskable' as const,
  })),
]

export default defineConfig({
  define: {
    'import.meta.env.VITE_APP_VERSION': JSON.stringify(pkgVersion),
  },
  plugins: [
    react(),
    VitePWA({
      registerType: 'prompt',
      includeAssets: [
        'favicon.svg',
        'offline.html',
        'brand/vulyk-mark.svg',
        'brand/vulyk-logo.svg',
        'icons/bee.svg',
        'icons/mark.svg',
        'icons/*.png',
      ],
      manifest: {
        name: 'Vulyk',
        short_name: 'Vulyk',
        description:
          'Vulyk — open-source test management. Record results on the go.',
        theme_color: '#7A5C18',
        background_color: '#F7F1E6',
        display: 'standalone',
        orientation: 'any',
        start_url: '/',
        scope: '/',
        lang: 'en',
        categories: ['productivity', 'developer'],
        icons,
      },
      workbox: {
        navigateFallback: '/index.html',
        navigateFallbackDenylist: [
          /^\/api/,
          /^\/health/,
          /^\/ready/,
          /^\/offline\.html$/,
        ],
        globPatterns: ['**/*.{js,css,html,ico,png,svg,woff2,webmanifest}'],
        additionalManifestEntries: [{ url: '/offline.html', revision: null }],
        runtimeCaching: [
          {
            urlPattern: ({ request, url }) =>
              request.method === 'GET' &&
              url.pathname.startsWith('/api/v1/') &&
              (url.pathname.includes('/cases') ||
                url.pathname.includes('/runs') ||
                url.pathname.includes('/suites')),
            handler: 'NetworkFirst',
            options: {
              cacheName: 'vulyk-api-readonly',
              networkTimeoutSeconds: 4,
              expiration: {
                maxEntries: 120,
                maxAgeSeconds: 60 * 60 * 24 * 7,
              },
              cacheableResponse: { statuses: [0, 200] },
            },
          },
          {
            // Self-hosted @fontsource woff2 / CSS only (no Google Fonts CDN).
            urlPattern: ({ request, sameOrigin }) =>
              sameOrigin &&
              (request.destination === 'font' ||
                (request.destination === 'style' &&
                  /fontsource|fonts?\./i.test(request.url))),
            handler: 'CacheFirst',
            options: {
              cacheName: 'vulyk-fonts',
              expiration: {
                maxEntries: 40,
                maxAgeSeconds: 60 * 60 * 24 * 365,
              },
              cacheableResponse: { statuses: [0, 200] },
            },
          },
        ],
      },
      devOptions: {
        enabled: false,
      },
    }),
  ],
  server: {
    host: true,
    port: 5173,
    fs: {
      // Allow importing in-app Markdown from ../docs/user
      allow: [join(rootDir, '..')],
    },
    proxy: {
      '/api': 'http://127.0.0.1:8080',
      '/health': 'http://127.0.0.1:8080',
      '/ready': 'http://127.0.0.1:8080',
    },
  },
  preview: {
    host: true,
    port: 5173,
    proxy: {
      '/api': 'http://127.0.0.1:8080',
      '/health': 'http://127.0.0.1:8080',
      '/ready': 'http://127.0.0.1:8080',
    },
  },
  test: {
    environment: 'node',
    coverage: {
      provider: 'v8',
      reporter: ['text', 'text-summary', 'json-summary', 'lcov'],
      reportsDirectory: './coverage',
      // Default Vitest include = files loaded by the suite (day-one baseline).
      exclude: [
        'src/main.tsx',
        'src/**/*.d.ts',
        'src/i18n/locales/**',
        'src/testing/testIds.ts',
        'node_modules/**',
        'dist/**',
      ],
      // Floors from 2026-11-14 suite-imported baseline (~36% lines). Ratchet up.
      thresholds: {
        lines: 35,
        statements: 34,
        branches: 37,
        functions: 24,
      },
    },
  },
})
