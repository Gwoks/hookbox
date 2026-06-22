/** Shared provider tree (theme + tooltip + toast) wrapping the router. Mirrors
 * the reference providers.tsx composition. */
import type { ReactNode } from 'react'
import { ThemeProvider } from '@/theme/theme'
import { ToastProvider } from '@/components/ui/toast'
import { TooltipProvider } from '@/components/ui/tooltip'

export function Providers({ children }: { children: ReactNode }) {
  return (
    <ThemeProvider>
      <TooltipProvider delayDuration={200}>
        <ToastProvider>{children}</ToastProvider>
      </TooltipProvider>
    </ThemeProvider>
  )
}
