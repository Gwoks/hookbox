/** Public API surface for the SPA: the typed client, session store, schemas. */
export { api, ApiError, setUnauthorizedHandler } from './client'
export { session, subscribe } from './session'
export { useSession, type SessionSnapshot } from './use-session'
export * from './schemas'
