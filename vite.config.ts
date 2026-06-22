/**
 * Vite config for the HookBox SPA (PRD §7 FE lane; architecture.md "Component &
 * file design" — `src/` is a fresh Vite + React + TS SPA served from dist/).
 * - React plugin for JSX/Fast Refresh.
 * - `@` alias → ./src (components import via `@/...`, mirroring ../shortener-link).
 * - Dev proxy: same-origin /api, /ws, /sse → the Rust backend on :8080 (OQ-4).
 * - Build output → dist/ (the Rust `spa.rs` ServeDir + index.html fallback).
 */
import { fileURLToPath, URL } from 'node:url'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  server: {
    proxy: {
      '/api': { target: 'http://localhost:8080', changeOrigin: true },
      '/ws': { target: 'ws://localhost:8080', ws: true },
      '/sse': { target: 'http://localhost:8080', changeOrigin: true },
      '/healthz': { target: 'http://localhost:8080', changeOrigin: true },
    },
  },
  build: {
    outDir: 'dist',
  },
})
