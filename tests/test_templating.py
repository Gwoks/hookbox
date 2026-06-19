"""Unit tests for the sandboxed response-templating engine (§5.7, AC-20..23/S10/S11)."""
import pytest

from app.interceptor.templating import TemplateContext, render_safe


def test_random_uuid_rendered():
    out = render_safe("{{random 'uuid'}}", TemplateContext())
    assert len(out) == 36 and out.count("-") == 4 and "{{" not in out


def test_query_and_method_echo():
    ctx = TemplateContext(method="GET", query={"name": "bob"})
    assert render_safe("{{request.query.name}}", ctx) == "bob"
    assert render_safe("{{request.method}}", ctx) == "GET"


def test_state_echo():
    assert render_safe("{{state.k}}", TemplateContext(state={"k": "v"})) == "v"


def test_unknown_query_key_renders_empty():
    assert render_safe("[{{request.query.missing}}]", TemplateContext()) == "[]"


@pytest.mark.parametrize("tag", [
    "{{ 7*7 }}", "{{ ''.__class__ }}", "{{config}}", "{{ self }}",
    "{{ ().__class__.__bases__ }}", "{{ request.application }}",
])
def test_ssti_payloads_are_inert(tag):
    out = render_safe(tag, TemplateContext())
    assert "49" not in out      # no arithmetic evaluation
    assert "{{" in out          # unknown tag left literal (no eval/exec)


def test_oversize_template_returned_unrendered():
    from config import TEMPLATE_MAX_SIZE
    big = "x" * (TEMPLATE_MAX_SIZE + 10) + "{{random 'uuid'}}"
    assert render_safe(big, TemplateContext()) == big
