"""Unit tests for the MITM SSRF guard + IP-pinning (AC-S6..S9, hookbox-zqd)."""
import ipaddress

import pytest

from app.interceptor.proxy import SSRFBlocked, _ip_is_blocked, _pin_target, _validate_url


@pytest.mark.parametrize("url", [
    "http://169.254.169.254/latest/meta-data/",   # cloud metadata
    "http://127.0.0.1:6379/",                      # loopback (redis)
    "http://10.0.0.5/",                            # private
    "http://192.168.1.1/",                         # private
    "http://[::1]/",                               # ipv6 loopback
])
def test_validate_url_blocks_internal_literals(url):
    with pytest.raises(SSRFBlocked):
        _validate_url(url)


@pytest.mark.parametrize("url", ["ftp://host/", "file:///etc/passwd", "gopher://x/9000"])
def test_validate_url_rejects_non_http_scheme(url):
    with pytest.raises(SSRFBlocked):
        _validate_url(url)


def test_validate_url_allows_public_literal():
    assert _validate_url("http://93.184.216.34/") == ["93.184.216.34"]


def test_pin_target_https_preserves_host_and_sni():
    ip_url, host_header, sni = _pin_target("https://example.com/p?q=1", "93.184.216.34")
    assert ip_url == "https://93.184.216.34/p?q=1"   # connect to the validated IP
    assert host_header == "example.com"               # upstream vhost routing
    assert sni == "example.com"                       # TLS verification stays on hostname


def test_pin_target_http_with_port_no_sni():
    ip_url, host_header, sni = _pin_target("http://svc:8080/a", "1.2.3.4")
    assert ip_url == "http://1.2.3.4:8080/a"
    assert host_header == "svc:8080"
    assert sni is None                                # no SNI for plain http


def test_ip_classification():
    assert _ip_is_blocked(ipaddress.ip_address("169.254.169.254"))
    assert _ip_is_blocked(ipaddress.ip_address("127.0.0.1"))
    assert not _ip_is_blocked(ipaddress.ip_address("93.184.216.34"))
