/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Cloudflare Turnstile site key (public). Empty/unset disables the
   * human check on the landing's first session mint. */
  readonly VITE_TURNSTILE_SITE_KEY?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
