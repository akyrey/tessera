/// <reference types="vitest/config" />
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { VitePWA } from 'vite-plugin-pwa'

const BACKEND_URL = process.env.TESSERA_BACKEND_URL ?? 'http://localhost:8080'
const COVERAGE_THRESHOLD = 80

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    VitePWA({
      registerType: 'autoUpdate',
      // Registration lives in src/pwa.ts; no inline script, so the CSP stays strict.
      injectRegister: false,
      pwaAssets: { image: 'public/favicon.svg', preset: 'minimal-2023' },
      manifest: {
        name: 'Tessera',
        short_name: 'Tessera',
        description: 'Your loyalty cards, always at hand.',
        lang: 'it',
        theme_color: '#0f766e',
        background_color: '#f0fdfa',
        display: 'standalone',
        start_url: '/',
        scope: '/',
      },
      workbox: {
        // Take control of the page that installed the worker, so it works offline right away.
        clientsClaim: true,
        skipWaiting: true,
        // The barcode scanner's WASM must work offline too.
        // (The manifest and icons are added by the plugin itself; listing them twice breaks precaching.)
        globPatterns: ['**/*.{js,css,html,wasm}'],
        maximumFileSizeToCacheInBytes: 4 * 1024 * 1024,
        runtimeCaching: [
          {
            // Photo ids change on every replacement, so a cached photo never goes stale.
            urlPattern: ({ url }) => url.pathname.startsWith('/api/images/'),
            handler: 'CacheFirst',
            options: {
              cacheName: 'card-photos',
              expiration: { maxEntries: 1000 },
              cacheableResponse: { statuses: [200] },
            },
          },
          {
            // Same for brand logos: a new logo gets a new id.
            urlPattern: ({ url }) => url.pathname.startsWith('/api/brands/logos/'),
            handler: 'CacheFirst',
            options: {
              cacheName: 'brand-logos',
              expiration: { maxEntries: 500 },
              cacheableResponse: { statuses: [200] },
            },
          },
        ],
        navigateFallback: '/index.html',
        // Probes and the API must always hit the network.
        navigateFallbackDenylist: [/^\/api\//, /^\/healthz$/, /^\/readyz$/],
      },
    }),
  ],
  build: {
    // bwip-js (barcodes) and the app chunk are large but load once and are precached
    // by the service worker; route-level splitting would not help an offline-first PWA.
    chunkSizeWarningLimit: 1024,
  },
  server: {
    proxy: {
      '/api': BACKEND_URL,
      '/healthz': BACKEND_URL,
      '/readyz': BACKEND_URL,
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
    restoreMocks: true,
    mockReset: true,
    unstubGlobals: true,
    coverage: {
      provider: 'v8',
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        'src/**/*.test.{ts,tsx}',
        'src/test/**',
        'src/api/generated/**',
        // Composition roots: covered by the Playwright suite.
        'src/main.tsx',
        'src/App.tsx',
        'src/pwa.ts',
        // Leaflet needs a real layout engine: covered by Playwright (e2e/cards.spec.ts).
        'src/features/map/ShopMap.tsx',
      ],
      thresholds: {
        lines: COVERAGE_THRESHOLD,
        functions: COVERAGE_THRESHOLD,
        branches: COVERAGE_THRESHOLD,
        statements: COVERAGE_THRESHOLD,
      },
    },
  },
})
