/**
 * Vitest config — unit/component tests for the SPA (hookbox-a73 S0).
 * Reuses vite.config.ts (React plugin + `@` alias) so tests resolve modules
 * exactly like the app. Playwright owns e2e/ — vitest only collects
 * `src/**\/*.test.{ts,tsx}`.
 */
import { defineConfig, mergeConfig } from 'vitest/config'
import viteConfig from './vite.config'

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'jsdom',
      include: ['src/**/*.test.{ts,tsx}'],
      setupFiles: ['src/test/setup.ts'],
      restoreMocks: true,
      coverage: {
        provider: 'v8',
        include: ['src/lib/**', 'src/design/**', 'src/components/{ui,hookbox,layout}/**'],
        exclude: ['src/**/*.test.{ts,tsx}', 'src/lib/copy.ts'],
        reporter: ['text-summary', 'text'],
      },
    },
  }),
)
