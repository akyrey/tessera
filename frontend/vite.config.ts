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
        navigateFallback: '/index.html',
        // Probes and the API must always hit the network.
        navigateFallbackDenylist: [/^\/api\//, /^\/healthz$/, /^\/readyz$/],
      },
    }),
  ],
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
    coverage: {
      provider: 'v8',
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        'src/**/*.test.{ts,tsx}',
        'src/test/**',
        'src/api/generated/**',
        'src/main.tsx',
        'src/pwa.ts',
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
