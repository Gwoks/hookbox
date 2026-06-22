/** Toast (design.md §3.0/§6). Quiet success confirmation; success-fg icon.
 * Radix Toast re-themed. A tiny context exposes `toast(...)`. */
import * as ToastPrimitive from '@radix-ui/react-toast'
import { CheckCircle2, XCircle } from 'lucide-react'
import { createContext, useCallback, useContext, useState, type ReactNode } from 'react'
import { cn } from '@/lib/cn'

type ToastVariant = 'success' | 'danger'
interface ToastItem {
  id: number
  message: string
  variant: ToastVariant
}
interface ToastCtx {
  toast: (message: string, variant?: ToastVariant) => void
}
const Ctx = createContext<ToastCtx>({ toast: () => {} })

export function useToast() {
  return useContext(Ctx)
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([])
  const toast = useCallback((message: string, variant: ToastVariant = 'success') => {
    setItems((prev) => [...prev, { id: Date.now() + Math.random(), message, variant }])
  }, [])

  return (
    <Ctx.Provider value={{ toast }}>
      <ToastPrimitive.Provider swipeDirection="right">
        {children}
        {items.map((t) => (
          <ToastPrimitive.Root
            key={t.id}
            duration={3200}
            onOpenChange={(open) => {
              if (!open) setItems((prev) => prev.filter((i) => i.id !== t.id))
            }}
            className="flex items-center gap-2 rounded-md border border-border bg-surface-raised px-3 py-2 text-body-sm text-text-primary shadow-md animate-toast-in"
          >
            {t.variant === 'success' ? (
              <CheckCircle2 className="h-4 w-4 text-success-fg" aria-hidden="true" />
            ) : (
              <XCircle className="h-4 w-4 text-danger-fg" aria-hidden="true" />
            )}
            <ToastPrimitive.Description>{t.message}</ToastPrimitive.Description>
          </ToastPrimitive.Root>
        ))}
        <ToastPrimitive.Viewport
          className={cn(
            'fixed bottom-4 right-4 z-toast flex w-[min(360px,92vw)] flex-col gap-2 outline-none',
          )}
        />
      </ToastPrimitive.Provider>
    </Ctx.Provider>
  )
}
