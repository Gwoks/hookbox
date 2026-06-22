/**
 * SPA entry. Mounts the shared provider tree around the react-router
 * RouterProvider and loads the token-driven global stylesheet. Mirrors the
 * reference main.tsx.
 */
import React from 'react'
import ReactDOM from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { Providers } from '@/components/providers'
import { router } from '@/router'
import { setUnauthorizedHandler } from '@/api'
import './globals.css'

// A /api 401 (rotated/stale secret) bounces to the landing gate (§5.1, AC-J7).
// The reason string is carried in router state so the gate can surface the
// common.error.401 banner.
setUnauthorizedHandler((reason) => {
  if (router.state.location.pathname !== '/') {
    void router.navigate('/', { state: { reason } })
  }
})

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <Providers>
      <RouterProvider router={router} />
    </Providers>
  </React.StrictMode>,
)
