/**
 * The Cloudflare Turnstile site key baked in at build time
 * (`VITE_TURNSTILE_SITE_KEY`). A getter rather than a constant so tests can
 * mock this module instead of mutating `import.meta.env`.
 * Returns null when unset/blank: the landing then mints without a challenge.
 */
export function getTurnstileSiteKey(): string | null {
  const key = import.meta.env.VITE_TURNSTILE_SITE_KEY?.trim()
  return key ? key : null
}
