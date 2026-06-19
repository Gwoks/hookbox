"""MITM forward (arch §4.4, §5.5; AC-14..17, AC-S6..S9; security §4.3).

When a P1 request matches no rule (and Auto-CRUD/tunnel do not handle it) and the
endpoint has a ``target_url`` set, the request is forwarded to the real upstream
via a **shared** ``httpx.AsyncClient`` and the real response is captured and
returned to the caller, labeled ``served_by="mitm"`` (AC-14/15).

SSRF guard (AC-S7/S8, security §4.3) — the core of this module:
  * The target scheme must be ``http``/``https`` (AC-S6 is enforced at config time
    with 422; this is a defensive backstop returning 502 if a bad scheme slips in).
  * The target hostname is **resolved to its IP(s)** and every resolved address is
    checked against a block-list (loopback / private / link-local / multicast /
    reserved / the cloud-metadata address ``169.254.169.254``). A match is refused
    with ``502`` and logged — evaluated on the **resolved IP**, not the hostname
    string, unless ``MITM_ALLOW_PRIVATE=true``.
  * The connection is then **pinned to that validated IP** (we connect to the IP
    literal, preserving the ``Host`` header + TLS SNI / certificate verification for
    the original hostname), so a rebinding DNS record cannot swap in an internal
    address between the check and the TCP connect (DNS-rebinding TOCTOU, hookbox-zqd).
  * If redirect-following is enabled (``MITM_FOLLOW_REDIRECTS``), redirects are
    followed manually up to ``MITM_MAX_REDIRECTS`` and **AC-S7 is re-applied to
    every hop's resolved IP** (AC-S8). By default redirects are **not** followed.

Caps & header hygiene (AC-S9):
  * ``MITM_TIMEOUT_S`` (connect/read) → timeout maps to ``504``.
  * Connection / DNS / SSRF-block errors map to ``502 upstream_unreachable``.
  * The response body is capped at ``MITM_MAX_BODY_BYTES``.
  * Hop-by-hop **and sensitive** request headers are stripped before forwarding
    (``strip_forward_headers``) — the owner capability is **never** forwarded.
  * Upstream hop-by-hop / ``Set-Cookie`` / upstream CORS headers are stripped from
    the response so they cannot break our own auto-CORS injection (done by the
    engine after this returns).

This path is **exempt from the <10ms budget** (dominated by the upstream).
"""

from __future__ import annotations

import ipaddress
import logging
import socket
from typing import Dict, List, Optional, Tuple
from urllib.parse import urlencode, urlsplit, urlunsplit

import httpx
from fastapi.responses import JSONResponse, Response

import config
from app.utils.helpers import strip_forward_headers

logger = logging.getLogger("hookbox.proxy")

# Allowed upstream schemes (everything else is refused).
_ALLOWED_SCHEMES = {"http", "https"}

# Response headers we never copy back from the upstream: hop-by-hop (would corrupt
# our connection framing) + Set-Cookie (security: don't relay upstream sessions) +
# upstream CORS (we inject our own wide-open P1 CORS, §5.6) + content-length /
# content-encoding (we re-send a decoded, possibly-truncated body).
_STRIP_RESPONSE_HEADERS = {
    "connection", "keep-alive", "proxy-authenticate", "proxy-authorization",
    "te", "trailers", "transfer-encoding", "upgrade",
    "content-length", "content-encoding",
    "set-cookie",
    "access-control-allow-origin", "access-control-allow-methods",
    "access-control-allow-headers", "access-control-allow-credentials",
    "access-control-expose-headers", "access-control-max-age",
    "vary",
}

# Shared async client (one per process). Lazily created; redirects disabled — we
# follow them manually so we can re-validate each hop (AC-S8).
_client: Optional[httpx.AsyncClient] = None


def _get_client() -> httpx.AsyncClient:
    global _client
    if _client is None or _client.is_closed:
        timeout = httpx.Timeout(float(config.MITM_TIMEOUT_S))
        _client = httpx.AsyncClient(
            timeout=timeout,
            follow_redirects=False,  # we re-validate each hop ourselves
            limits=httpx.Limits(max_connections=100, max_keepalive_connections=20),
        )
    return _client


async def aclose() -> None:
    """Close the shared client (call from the app lifespan shutdown)."""
    global _client
    if _client is not None and not _client.is_closed:
        try:
            await _client.aclose()
        finally:
            _client = None


# --- SSRF guard ---------------------------------------------------------------
class SSRFBlocked(Exception):
    """A target (or redirect hop) resolved to a blocked address (AC-S7)."""


def _ip_is_blocked(ip: ipaddress._BaseAddress) -> bool:
    """True if ``ip`` is loopback / private / link-local / metadata / reserved.

    Covers AC-S7: ``127.0.0.0/8``, ``::1``, ``10/8`` ``172.16/12`` ``192.168/16``
    ``fc00::/7``, ``169.254.0.0/16`` (incl. ``169.254.169.254``) ``fe80::/10``,
    plus multicast / unspecified / reserved as a defensive superset.
    """
    return bool(
        ip.is_loopback
        or ip.is_private
        or ip.is_link_local
        or ip.is_multicast
        or ip.is_reserved
        or ip.is_unspecified
    )


def _resolve_and_check(host: str) -> List[str]:
    """Resolve ``host`` to IP literals and reject if **any** resolved address is in
    a blocked range (AC-S7, evaluated on the resolved IP). Returns the resolved IPs.

    Raises :class:`SSRFBlocked` on a blocked address or a resolution failure
    (an unresolvable host is treated as unreachable → 502 upstream).
    """
    if config.MITM_ALLOW_PRIVATE:
        # Operator opted out of the guard (e.g. forwarding to a private upstream in
        # a trusted network). Still ensure the host resolves.
        host = host.strip("[]")
        try:
            ipaddress.ip_address(host)
            return [host]
        except ValueError:
            pass
        try:
            infos = socket.getaddrinfo(host, None, proto=socket.IPPROTO_TCP)
        except (socket.gaierror, UnicodeError, OSError) as exc:
            raise SSRFBlocked(f"cannot resolve host {host!r}") from exc
        return [info[4][0] for info in infos]

    host = host.strip("[]")  # strip IPv6 brackets if present
    # Literal IP target: check directly (no DNS).
    try:
        literal = ipaddress.ip_address(host)
        if _ip_is_blocked(literal):
            raise SSRFBlocked(f"target IP {host} is in a blocked range")
        return [str(literal)]
    except ValueError:
        pass  # not a literal IP — resolve the hostname

    try:
        infos = socket.getaddrinfo(host, None, proto=socket.IPPROTO_TCP)
    except (socket.gaierror, UnicodeError, OSError) as exc:
        raise SSRFBlocked(f"cannot resolve host {host!r}") from exc

    resolved: List[str] = []
    for info in infos:
        addr = info[4][0]
        # Strip IPv6 zone id if present (e.g. "fe80::1%eth0").
        addr_clean = addr.split("%", 1)[0]
        try:
            ip = ipaddress.ip_address(addr_clean)
        except ValueError:
            continue
        if _ip_is_blocked(ip):
            raise SSRFBlocked(f"host {host!r} resolves to blocked address {addr_clean}")
        resolved.append(addr_clean)
    if not resolved:
        raise SSRFBlocked(f"host {host!r} produced no usable address")
    return resolved


def _validate_url(url: str) -> List[str]:
    """Validate scheme + host and SSRF-check the **resolved IPs**. Returns the list
    of validated (block-list-passed) IP literals so the caller can PIN the
    connection to one of them — closing the DNS-rebinding TOCTOU between this check
    and httpx's connect-time re-resolution (hookbox-zqd)."""
    parts = urlsplit(url)
    if parts.scheme.lower() not in _ALLOWED_SCHEMES:
        raise SSRFBlocked(f"scheme {parts.scheme!r} not allowed")
    if not parts.hostname:
        raise SSRFBlocked("target has no host")
    return _resolve_and_check(parts.hostname)


def _pin_target(url: str, ip: str) -> Tuple[str, str, Optional[str]]:
    """Rewrite ``url`` to connect to the pre-validated ``ip`` literal while keeping
    the original hostname for the upstream ``Host`` header and (https) the TLS SNI /
    certificate verification. Returns ``(ip_url, host_header, sni_hostname)``.

    Because httpx connects to the URL host verbatim, putting the checked IP there
    means no second DNS resolution can occur — a rebinding record cannot swap in an
    internal address between :func:`_validate_url` and the TCP connect.
    """
    parts = urlsplit(url)
    host = parts.hostname or ""
    port = parts.port
    ip_host = f"[{ip}]" if ":" in ip else ip                 # bracket IPv6 literals
    netloc = f"{ip_host}:{port}" if port else ip_host
    ip_url = urlunsplit((parts.scheme, netloc, parts.path or "/", parts.query, ""))
    disp_host = f"[{host}]" if ":" in host else host          # bracket IPv6 in Host
    host_header = f"{disp_host}:{port}" if port else disp_host
    sni_hostname = host if parts.scheme.lower() == "https" else None
    return ip_url, host_header, sni_hostname


# --- URL assembly -------------------------------------------------------------
def _build_target_url(target_url: str, mock_path: str, query: Dict[str, str]) -> str:
    """``target_url + mock_path + ?querystring`` (arch §4.4).

    The endpoint's ``target_url`` is treated as a base; the incoming mock path is
    appended (avoiding a double slash) and the original query string is preserved.
    """
    base = urlsplit(target_url)
    base_path = base.path.rstrip("/")
    if mock_path and not mock_path.startswith("/"):
        mock_path = "/" + mock_path
    full_path = (base_path + mock_path) or "/"
    qs = urlencode(query, doseq=True) if query else base.query
    return urlunsplit((base.scheme, base.netloc, full_path, qs, ""))


def _safe_response_headers(upstream_headers) -> Dict[str, str]:
    out: Dict[str, str] = {}
    for k, v in upstream_headers.items():
        if k.lower() in _STRIP_RESPONSE_HEADERS:
            continue
        out[k] = v
    return out


# --- the forward --------------------------------------------------------------
async def forward(
    ep,
    method: str,
    mock_path: str,
    query: Dict[str, str],
    headers_lower: Dict[str, str],
    body_bytes: bytes,
) -> Response:
    """Forward an unmatched request to ``ep.target_url`` and return the upstream
    response captured for the caller (AC-14/15).

    Always returns a :class:`Response`:
      * upstream success → its status / safe headers / (capped) body
      * SSRF-blocked target/hop → ``502 upstream_unreachable`` (logged, AC-S7/S8)
      * connection / DNS error → ``502 upstream_unreachable`` (logged, AC-17)
      * timeout → ``504`` (logged, AC-17)
      * oversize body → truncated to ``MITM_MAX_BODY_BYTES`` (AC-S9)
    """
    target_url = (ep.target_url or "").strip()
    if not target_url:
        # Defensive: caller only invokes us when target_url is set.
        return JSONResponse(status_code=502, content={"error": "upstream_unreachable",
                            "detail": "No upstream target configured."})

    url = _build_target_url(target_url, mock_path, query)

    # Strip hop-by-hop + sensitive headers; the owner capability never leaves here.
    fwd_headers = strip_forward_headers(headers_lower)

    client = _get_client()
    redirects_left = config.MITM_MAX_REDIRECTS if config.MITM_FOLLOW_REDIRECTS else 0
    current_url = url
    current_method = method
    current_body = body_bytes

    try:
        while True:
            # SSRF guard on the (initial or redirected) target — checks every
            # resolved IP, re-applied on each hop (AC-S7 / AC-S8).
            try:
                validated_ips = _validate_url(current_url)
            except SSRFBlocked as exc:
                logger.warning("MITM SSRF block: %s (url=%s)", exc, current_url)
                return JSONResponse(
                    status_code=502,
                    content={"error": "upstream_unreachable",
                             "detail": "Upstream target is not permitted."},
                )

            # PIN to a pre-validated IP so httpx cannot re-resolve the hostname to a
            # different (internal) address between the check above and the connect —
            # a DNS-rebinding TOCTOU bypass (hookbox-zqd). Host header + (https) TLS
            # SNI / cert verification still use the original hostname.
            ip_url, host_header, sni_hostname = _pin_target(current_url, validated_ips[0])
            req = client.build_request(
                current_method,
                ip_url,
                headers={**fwd_headers, "host": host_header},
                content=current_body if current_body else None,
            )
            if sni_hostname:
                req.extensions["sni_hostname"] = sni_hostname
            resp = await client.send(req)

            # Manual redirect handling so each hop is re-validated (AC-S8).
            if resp.is_redirect and redirects_left > 0:
                location = resp.headers.get("location")
                if not location:
                    break
                redirects_left -= 1
                # Resolve relative redirects against the current URL.
                current_url = str(httpx.URL(current_url).join(location))
                # Per the HTTP spec, 303 (and commonly 301/302 for non-GET) switch
                # to GET with no body.
                if resp.status_code in (301, 302, 303) and current_method not in ("GET", "HEAD"):
                    current_method = "GET"
                    current_body = b""
                # Drop the body on continued redirects to avoid re-POSTing.
                continue

            # Capture the (final) response.
            raw = resp.content
            truncated = False
            if len(raw) > config.MITM_MAX_BODY_BYTES:
                raw = raw[: config.MITM_MAX_BODY_BYTES]
                truncated = True
            out_headers = _safe_response_headers(resp.headers)
            if truncated:
                out_headers["X-HookBox-Truncated"] = "true"
            return Response(
                content=raw,
                status_code=resp.status_code,
                headers=out_headers,
                media_type=resp.headers.get("content-type"),
            )

        # Fell out of the loop (redirect with no Location) — return last response.
        raw = resp.content[: config.MITM_MAX_BODY_BYTES]
        return Response(content=raw, status_code=resp.status_code,
                        headers=_safe_response_headers(resp.headers),
                        media_type=resp.headers.get("content-type"))

    except httpx.TimeoutException as exc:
        logger.info("MITM timeout to %s: %s", url, exc)
        return JSONResponse(status_code=504, content={"error": "upstream_timeout",
                            "detail": "Upstream did not respond in time."})
    except (httpx.ConnectError, httpx.NetworkError, httpx.RemoteProtocolError,
            httpx.UnsupportedProtocol, httpx.InvalidURL) as exc:
        logger.info("MITM connection error to %s: %s", url, exc)
        return JSONResponse(status_code=502, content={"error": "upstream_unreachable",
                            "detail": "Could not reach the upstream target."})
    except Exception as exc:  # noqa: BLE001 - last-resort: never crash the engine
        logger.exception("MITM unexpected error to %s", url)
        return JSONResponse(status_code=502, content={"error": "upstream_unreachable",
                            "detail": "Upstream forward failed."})
