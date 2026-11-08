# Decision: anonymous owner key in an HttpOnly cookie (supersedes AC-S14)

- **Date:** 2026-11-08
- **Status:** Accepted, implemented (bd epic `hookbox-a73`, stage S2 `hookbox-a73.3`)
- **Supersedes:** AC-S14 in `../hookbox-rust-replatform/prd.md` and `security.md`
  ("the API never reads the capability from a cookie").

## Context

Owners were identified by email: `POST /api/session {email}` derived
`owner_id = sha256(lower(trim(email)))[:16]` and rotated a random secret that the
SPA kept in `localStorage` and sent as `Authorization: Bearer`. The email was never
verified, so typing someone else's address took over their endpoints and logged
them out. Path-fallback mocks (`/e/<token>/…`) are served on the app origin with
owner-controlled HTML/JS, which could also read `localStorage`.

## Decision

An owner is an anonymous random 256-bit **owner key**, stored server-side only as
its SHA-256 (unique index), carried in an `HttpOnly; SameSite=Strict; Path=/`
cookie with no `Domain` (`__Host-hookbox_key` + `Secure` behind https). Bearer is
still accepted on `/api` (scripts) and is the only credential the tunnel accepts.

| Route | Purpose |
|---|---|
| `POST /api/session` | mint owner + primary endpoint, set cookie (200 + existing summary if already signed in) |
| `POST /api/session/restore {key}` | restore in another browser (does not rotate) |
| `GET /api/session` | current owner summary |
| `POST /api/session/key` | reveal the key (recovery / CLI), `no-store` |
| `POST /api/session/rotate` | new key; old key, tunnels and live feeds cut off |
| `DELETE /api/session` | clear this browser's cookie (key stays valid) |

Browsers from the email era are migrated silently: the SPA restores the legacy
`localStorage` secret into the cookie, then deletes the legacy keys. Migration
`0004_anonymous_owner_key.sql` drops `owners.email`.

## New acceptance criteria

- **AC-S14a — Cookie attributes.** `HttpOnly`, `SameSite=Strict`, `Path=/`, no
  `Domain`; `Secure` + `__Host-` prefix when `SESSION_COOKIE_SECURE` (default:
  https `PUBLIC_BASE_URL`). A non-localhost `APP_HOST` with an insecure cookie
  refuses to start unless `SESSION_COOKIE_SECURE=false` is explicit.
- **AC-S14b — CSRF.** Cookie-authenticated `/api` calls require `X-HookBox: 1`
  (forces a CORS preflight that `/api` never answers) and an Origin /
  `Sec-Fetch-Site` same-origin check → otherwise `403 csrf_rejected`. A present
  `Authorization` header is used exclusively (no cookie fallback).
- **AC-S14c — Feed & tunnel.** `/ws` and `/sse` take the cookie or Bearer, check
  Origin before upgrade (`403`), and never accept `?cap=`. The tunnel is
  Bearer-only. Rotation revokes bound tunnels (4401) and live feeds (WS 1012 /
  SSE end) with subscribe/bind-then-verify against the race.
- **AC-S14d — Mock-plane isolation.** Every mock response carries
  `Content-Security-Policy: sandbox …` without `allow-same-origin`; the session
  cookie is stripped from incoming mock requests before the engine, templates,
  echo, traces, MITM and tunnels; mock responses cannot set or clear it
  (`Set-Cookie` for our name and `Clear-Site-Data` dropped; all `Set-Cookie`
  dropped on the same-origin path fallback).

## Consequences and follow-ups

- Losing the key loses the account (no email recovery by design). Sign-out warns
  and offers to copy the key first.
- Any script on the app origin can still call `POST /api/session/key`; a strict
  SPA CSP is tracked in `hookbox-19v`.
- Follow-ups filed from the security review: full-origin comparison for the feed
  Origin check, bounded anonymous-owner growth + stale-owner sweep, tunnel CLI key
  via env/stdin and refusing remote `ws://`, per-device revocable sessions.
