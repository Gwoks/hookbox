/**
 * AppShell — the owner chrome around every owner screen: skip link, the
 * sticky TopNav, `<main id="main">`, and the two dialogs the nav opens
 * (Share, Sign out). Rendered by OwnerLayout inside a ready
 * EndpointProvider; the screens below it read the endpoint from context.
 *
 * The Share count is one glance-only GET per endpoint (AC-98): a failure
 * just leaves the plain "Share" label.
 */
import { useState, type ReactNode } from 'react'
import { api } from '@/api'
import { useResource } from '@/lib/hooks/use-resource'
import { SkipLink } from '@/components/layout/skip-link'
import { useEndpoints } from './endpoint-context'
import { ShareDialog } from './share-dialog'
import { SignOutDialog } from './sign-out-dialog'
import { TopNav } from './top-nav'

export function AppShell({ children }: { children: ReactNode }) {
  const { token } = useEndpoints()
  const [shareOpen, setShareOpen] = useState(false)
  const [signOutOpen, setSignOutOpen] = useState(false)
  const shares = useResource((signal) => api.listShares(token, { signal }), [token])
  // The dialog reports the count after every create/revoke.
  const [liveShareCount, setLiveShareCount] = useState<number | null>(null)
  const shareCount = liveShareCount ?? shares.data?.length ?? null

  return (
    <div className="flex min-h-screen flex-col bg-canvas">
      <SkipLink />
      <TopNav shareCount={shareCount} onShare={() => setShareOpen(true)} onSignOut={() => setSignOutOpen(true)} />
      <main id="main" className="flex min-h-0 flex-1 flex-col">
        {children}
      </main>
      <ShareDialog token={token} open={shareOpen} onOpenChange={setShareOpen} onCountChange={setLiveShareCount} />
      <SignOutDialog open={signOutOpen} onClose={() => setSignOutOpen(false)} />
    </div>
  )
}
