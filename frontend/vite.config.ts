import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'
import { VitePWA } from 'vite-plugin-pwa'

const iconSizes = [72, 96, 128, 144, 152, 192, 384, 512] as const

const icons = iconSizes.flatMap((size) => [
  {
    src: `/icons/icon-${size}.png`,
    sizes: `${size}x${size}`,
    type: 'image/png',
    purpose: 'any' as const,
  },
  {
    src: `/icons/icon-maskable-${size}.png`,
    sizes: `${size}x${size}`,
    type: 'image/png',
    purpose: 'maskable' as const,
  },
])

export default defineConfig({
  plugins: [
    react(),
    VitePWA({
      registerType: 'prompt',
      includeAssets: [
        'favicon.svg',
        'offline.html',
        'icons/bee.svg',
        'icons/*.png',
      ],
      manifest: {
        name: 'Vulyk',
        short_name: 'Vulyk',
        description:
          'Vulyk — open-source test management. Record results on the go.',
        theme_color: '#0B6E4F',
        background_color: '#F7F8FA',
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
        navigateFallbackDenylist: [/^\/api/, /^\/health/, /^\/ready/, /^\/offline\.html$/],
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
            urlPattern: ({ request }) =>
              request.destination === 'font' ||
              request.destination === 'style' ||
              request.url.includes('fonts.googleapis.com') ||
              request.url.includes('fonts.gstatic.com'),
            handler: 'CacheFirst',
            options: {
              cacheName: 'vulyk-fonts',
              expiration: {
                maxEntries: 30,
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
  },
})
