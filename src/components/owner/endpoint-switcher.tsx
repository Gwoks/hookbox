/**
 * EndpointSwitcher — the compact outline menu next to the brand: the owner's
 * endpoints (current one checked) and "New endpoint", which creates one with
 * POST /api/endpoints and opens it (it used to navigate to a `/d/__new__`
 * route that never existed).
 */
import { Check, ChevronsUpDown, Plus } from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import { api } from '@/api'
import { errorMessage } from '@/lib/api-errors'
import { cn } from '@/lib/cn'
import { t } from '@/lib/copy'
import { useAsyncAction } from '@/lib/hooks/use-async-action'
import { Button } from '@/components/ui/button'
import { Menu, MenuContent, MenuItem, MenuSeparator, MenuTrigger } from '@/components/ui/menu'
import { useToast } from '@/components/ui/toast'
import { useEndpoints } from './endpoint-context'

export function EndpointSwitcher() {
  const { token, endpoint, endpoints } = useEndpoints()
  const navigate = useNavigate()
  const { toast } = useToast()
  const create = useAsyncAction(() => api.createEndpoint({}))
  const current = endpoint.name || endpoint.token

  async function handleCreate() {
    const result = await create.run()
    if (!result.ok) {
      toast(errorMessage(result.error, t('shell.nav.newEndpoint.error')), 'danger')
      return
    }
    navigate(`/d/${result.value.token}`)
  }

  return (
    <Menu>
      <MenuTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          className="min-w-0 max-w-[10rem] sm:max-w-[14rem]"
          aria-label={t('shell.nav.endpointSwitcher.label')}
          aria-busy={create.pending || undefined}
        >
          <span className="truncate font-mono text-mono-sm">{current}</span>
          <ChevronsUpDown className="h-3.5 w-3.5 shrink-0 text-text-tertiary" aria-hidden="true" />
        </Button>
      </MenuTrigger>
      <MenuContent align="start" className="max-w-[min(20rem,90vw)]">
        {endpoints.map((entry) => {
          const isCurrent = entry.token === token
          return (
            <MenuItem
              key={entry.token}
              aria-current={isCurrent ? 'true' : undefined}
              className={cn(isCurrent && 'text-text-primary')}
              onSelect={() => {
                if (!isCurrent) navigate(`/d/${entry.token}`)
              }}
            >
              <Check className={cn('h-3.5 w-3.5 shrink-0', !isCurrent && 'invisible')} aria-hidden="true" />
              <span className="truncate font-mono text-mono-sm">{entry.name || entry.token}</span>
            </MenuItem>
          )
        })}
        {endpoints.length > 0 && <MenuSeparator />}
        <MenuItem
          disabled={create.pending}
          onSelect={() => void handleCreate()}
          aria-label={t('shell.nav.newEndpoint.aria')}
        >
          <Plus className="h-3.5 w-3.5" aria-hidden="true" />
          {t('shell.nav.newEndpoint')}
        </MenuItem>
      </MenuContent>
    </Menu>
  )
}
