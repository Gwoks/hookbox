/** Public API surface for the owner SPA: the typed client, session status
 * store + bootstrap, schemas. The public /s/:code viewer never imports this
 * barrel (it uses api/public-client.ts directly). */
export { api, ApiError, CSRF_HEADER, setUnauthorizedHandler } from './client'
export { session, type SessionState, type SessionStatus } from './session'
export { ensureSession, resolvePrimaryToken } from './bootstrap'
export { useSession, useSessionGate, type SessionGate, type SessionSnapshot } from './use-session'
export * from './schemas'
