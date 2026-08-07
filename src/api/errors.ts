/**
 * The single `ApiError` shape thrown by both the owner client (api/client.ts)
 * and the public share-viewer client (api/public-client.ts). Zero
 * dependencies — in particular it must never import session.ts, so that the
 * viewer's module graph (which imports this file) stays session-free
 * (AC-S13).
 */
export class ApiError extends Error {
  code: string
  status: number
  retryAfter?: number
  constructor(code: string, detail: string, status: number, retryAfter?: number) {
    super(detail)
    this.name = 'ApiError'
    this.code = code
    this.status = status
    this.retryAfter = retryAfter
  }
}
