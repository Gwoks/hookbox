/**
 * Unit tests for src/lib/config-bundle.ts (operator-toolkit F3, §5.5.6 frozen
 * ConfigBundle file format). Pure, DOM-free — Node-context Playwright tests.
 */
import { expect, test } from "@playwright/test";
import {
  buildBundle,
  computeConfigDiff,
  configBundleSchema,
  countRulesUsingRequestHeaderTag,
  parseBundle,
  toBundleEndpoint,
  type ConfigBundle,
} from "../src/lib/config-bundle";
import type { EndpointDetail, MockRule } from "../src/api/schemas";

function makeEndpoint(over: Partial<EndpointDetail> = {}): EndpointDetail {
  return {
    token: "ab12cd34",
    name: "Checkout API",
    mock_url: "https://ab12cd34.hookbox.test",
    path_url: "http://localhost:8080/m/ab12cd34",
    auto_crud: true,
    target_url: null,
    default_mode: "mock_404",
    latency_ms: 0,
    rate_limit_per_min: 0,
    chaos_pct: 0,
    chaos_mode: "error",
    cors_enabled: false,
    tunnel_active: false,
    created_at: "2026-06-21T12:00:00Z",
    last_hit: null,
    request_count: 3,
    ...over,
  };
}

function makeRule(over: Partial<MockRule> = {}): MockRule {
  return {
    id: 1,
    token: "ab12cd34",
    name: "A rule",
    priority: 100,
    enabled: true,
    match: {
      method: "GET",
      path: "/x",
      headers: {},
      query: {},
      body_conditions: [],
      state_requirements: [],
    },
    response: {
      status_code: 200,
      headers: {},
      body_template: "{}",
      content_type: "application/json",
    },
    state_writes: [],
    latency_ms: null,
    rate_limit_per_min: null,
    chaos_mode: null,
    webhook_action: null,
    created_at: "2026-06-21T12:00:00Z",
    ...over,
  };
}

function validBundle(): ConfigBundle {
  return buildBundle(makeEndpoint(), [makeRule()]);
}

test.describe("configBundleSchema", () => {
  test("accepts a well-formed bundle", () => {
    expect(configBundleSchema.safeParse(validBundle()).success).toBe(true);
  });

  test("AC-13: rejects an unknown top-level key", () => {
    const bad = { ...validBundle(), extra: "nope" };
    expect(configBundleSchema.safeParse(bad).success).toBe(false);
  });

  test("AC-13: rejects the seven non-portable endpoint fields", () => {
    for (const field of [
      "token",
      "mock_url",
      "path_url",
      "created_at",
      "last_hit",
      "request_count",
      "tunnel_active",
    ]) {
      const bundle = validBundle();
      const bad = {
        ...bundle,
        endpoint: { ...bundle.endpoint, [field]: "x" },
      };
      expect(configBundleSchema.safeParse(bad).success, field).toBe(false);
    }
  });

  test("AC-S22: rejects a NESTED unknown key inside endpoint", () => {
    const bundle = validBundle();
    const bad = {
      ...bundle,
      endpoint: { ...bundle.endpoint, owner_secret: "sneaky" },
    };
    expect(configBundleSchema.safeParse(bad).success).toBe(false);
  });

  test("rejects a bundle missing an endpoint field", () => {
    const bundle = validBundle();
    const { auto_crud: _omit, ...rest } = bundle.endpoint;
    const bad = { ...bundle, endpoint: rest };
    expect(configBundleSchema.safeParse(bad).success).toBe(false);
  });

  test("a stray id/token/created_at inside a rule is stripped, not fatal", () => {
    const bundle = validBundle();
    const bad = {
      ...bundle,
      rules: [{ ...bundle.rules[0], id: 999, token: "sneaky", created_at: "x" }],
    };
    const parsed = configBundleSchema.safeParse(bad);
    expect(parsed.success).toBe(true);
    if (parsed.success) {
      expect(parsed.data.rules[0]).not.toHaveProperty("id");
      expect(parsed.data.rules[0]).not.toHaveProperty("token");
    }
  });
});

test.describe("parseBundle", () => {
  test("valid JSON round-trips to success", () => {
    const result = parseBundle(JSON.stringify(validBundle()));
    expect(result.success).toBe(true);
  });

  test("a leading UTF-8 BOM is stripped, not surfaced as a parser error", () => {
    const result = parseBundle("﻿" + JSON.stringify(validBundle()));
    expect(result.success).toBe(true);
  });

  test("an empty file", () => {
    const result = parseBundle("");
    expect(result.success).toBe(false);
    if (!result.success) expect(result.message).toContain("empty");
  });

  test("malformed JSON is worded for a hand-editing human, not 'Unexpected token'", () => {
    const result = parseBundle("{not valid json");
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.message.toLowerCase()).not.toContain("unexpected token");
      expect(result.message).toContain("JSON");
    }
  });

  test("wrong hookbox_config_version", () => {
    const bundle = { ...validBundle(), hookbox_config_version: 2 };
    const result = parseBundle(JSON.stringify(bundle));
    expect(result.success).toBe(false);
    if (!result.success) expect(result.message).toContain("2");
  });

  test("more than 200 rules", () => {
    const bundle = validBundle();
    const many = Array.from({ length: 201 }, () => bundle.rules[0]);
    const result = parseBundle(JSON.stringify({ ...bundle, rules: many }));
    expect(result.success).toBe(false);
    if (!result.success) expect(result.message).toContain("201");
  });

  test("an invalid rule names its 1-based index", () => {
    const bundle = validBundle();
    const badRule = { ...bundle.rules[0], priority: "not a number" };
    const result = parseBundle(JSON.stringify({ ...bundle, rules: [bundle.rules[0], badRule] }));
    expect(result.success).toBe(false);
    if (!result.success) expect(result.message).toContain("Rule 2");
  });
});

test.describe("computeConfigDiff", () => {
  test("only changing fields are listed, in frozen order", () => {
    const current = toBundleEndpoint(makeEndpoint());
    const next = { ...current, name: "New name", latency_ms: 50 };
    const diff = computeConfigDiff(current, next);
    expect(diff.map((r) => r.field)).toEqual(["name", "latency_ms"]);
    expect(diff[0]).toEqual({ field: "name", from: "Checkout API", to: "New name" });
    expect(diff[1]).toEqual({ field: "latency_ms", from: "0", to: "50" });
  });

  test("a null field diffs against null, not the string 'null'", () => {
    const current = toBundleEndpoint(makeEndpoint({ target_url: null }));
    const next = { ...current, target_url: "https://example.com" };
    const diff = computeConfigDiff(current, next);
    expect(diff).toEqual([
      { field: "target_url", from: null, to: "https://example.com" },
    ]);
  });

  test("no changes -> empty diff", () => {
    const current = toBundleEndpoint(makeEndpoint());
    expect(computeConfigDiff(current, { ...current })).toEqual([]);
  });
});

test("countRulesUsingRequestHeaderTag detects the {{request.header.X}} tag (AC-S23)", () => {
  const bundle = validBundle();
  const tagged = {
    ...bundle.rules[0],
    response: {
      ...bundle.rules[0].response,
      body_template: '{"auth": "{{request.header.Authorization}}"}',
    },
  };
  expect(countRulesUsingRequestHeaderTag([bundle.rules[0], tagged])).toBe(1);
});

test("AC-14: exported rules omit id/token/created_at", () => {
  const bundle = buildBundle(makeEndpoint(), [makeRule()]);
  expect(bundle.rules[0]).not.toHaveProperty("id");
  expect(bundle.rules[0]).not.toHaveProperty("token");
  expect(bundle.rules[0]).not.toHaveProperty("created_at");
});

test("AC-22: the bundle never carries owner_secret, owner_id, token, or share material", () => {
  const bundle = buildBundle(makeEndpoint(), [makeRule()]);
  const text = JSON.stringify(bundle);
  for (const forbidden of ["owner_secret", "owner_id", "ab12cd34", "\"code\"", "\"share"]) {
    expect(text.includes(forbidden), forbidden).toBe(false);
  }
});
