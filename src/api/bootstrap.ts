/**
 * Session bootstrap (hookbox-a73.3): asks the server whether this browser's
 * HttpOnly cookie holds a valid recovery key, and silently migrates a browser
 * that still has the pre-cookie localStorage secret.
 *
 *   1. GET /api/session → signed in (and any legacy keys are deleted).
 *   2. On 401, if `localStorage['hookbox-owner-secret']` exists, POST it to
 *      /api/session/restore — the server answers with the cookie.
 *   3. The three legacy keys are deleted when the restore succeeds OR is
 *      rejected (401). A network error or 429 keeps them so the next visit
 *      retries, and rejects so the gate can offer a retry.
 *
 * Concurrent callers share one in-flight request. Only owner screens call
 * this (via useSessionGate) — never app boot, never the public viewer.
 */
import { api } from './client'
import { ApiError } from './errors'
import type { SessionSummary } from './schemas'
import { session } from './session'

/** Pre-cookie identity keys; read once for migration, then deleted. */
export const LEGACY_STORAGE_KEYS = ['hookbox-owner-secret', 'hookbox-owner-id', 'hookbox-owner-email'] as const
const [LEGACY_SECRET_KEY] = LEGACY_STORAGE_KEYS

let inflight: Promise<SessionSummary | null> | null = null

/** Resolve this browser's session, signing the tab in or out. Rejects (and
 * leaves the status `unknown`) only when the server could not answer. */
export function ensureSession(): Promise<SessionSummary | null> {
  if (!inflight) {
    inflight = bootstrap().finally(() => {
      inflight = null
    })
  }
  return inflight
}

/** The owner's primary endpoint token, creating an endpoint first when the
 * owner has none left (e.g. they deleted their last one). */
export async function resolvePrimaryToken(summary: SessionSummary): Promise<string> {
  if (summary.primary) return summary.primary.token
  const created = await api.createEndpoint({})
  return created.token
}

async function bootstrap(): Promise<SessionSummary | null> {
  try {
    return signIn(await api.getSession())
  } catch (err) {
    if (!isUnauthorized(err)) throw err
  }

  const legacySecret = readLegacySecret()
  if (!legacySecret) {
    session.markSignedOut()
    return null
  }

  try {
    return signIn(await api.restoreSession(legacySecret))
  } catch (err) {
    if (!isUnauthorized(err)) throw err
    clearLegacyKeys()
    session.markSignedOut()
    return null
  }
}

function signIn(summary: SessionSummary): SessionSummary {
  clearLegacyKeys()
  session.markSignedIn(summary.owner_id)
  return summary
}

function isUnauthorized(err: unknown): boolean {
  return err instanceof ApiError && err.status === 401
}

function readLegacySecret(): string | null {
  try {
    return localStorage.getItem(LEGACY_SECRET_KEY) || null
  } catch {
    return null
  }
}

function clearLegacyKeys(): void {
  for (const key of LEGACY_STORAGE_KEYS) {
    try {
      localStorage.removeItem(key)
    } catch {
      /* storage blocked: nothing was stored there to begin with */
    }
  }
}
