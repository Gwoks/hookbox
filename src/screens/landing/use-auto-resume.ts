/**
 * When the session gate resolves to signed-in (a returning visitor, or
 * another tab signing this browser in), open the owner's primary endpoint
 * (creating one if none is left). A signed-out → signed-in flip is this
 * tab's own mint/restore, which navigates by itself, so it is skipped.
 * Returns true once a resume failed (the page then shows a banner).
 */
import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { api, ApiError, resolvePrimaryToken, session, type SessionStatus } from '@/api'

export function useAutoResume(status: SessionStatus): boolean {
  const navigate = useNavigate()
  const previous = useRef<SessionStatus | null>(null)
  const [hasFailed, setHasFailed] = useState(false)

  useEffect(() => {
    const before = previous.current
    previous.current = status
    if (status !== 'signed-in' || before === 'signed-out') return
    let cancelled = false
    setHasFailed(false)
    api
      .getSession()
      .then(resolvePrimaryToken)
      .then((token) => {
        if (!cancelled) navigate(`/d/${token}`, { replace: true })
      })
      .catch((err: unknown) => {
        if (cancelled) return
        if (err instanceof ApiError && err.status === 401) session.markSignedOut()
        else setHasFailed(true)
      })
    return () => {
      cancelled = true
    }
  }, [status, navigate])

  return hasFailed
}
