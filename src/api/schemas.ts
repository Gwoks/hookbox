/**
 * zod schemas mirroring the FROZEN §5.3 shared data models (PRD §5.3). Field
 * names, types, defaults, and clamps match the contract exactly — these are the
 * TS ⇄ Rust serde boundary. Optional fields serialize as `null` when absent
 * (Rust Option<T> with serde(default)); we model them as `.nullable()` where the
 * JSON key is present-with-null, and `.optional()` where a request body may omit
 * the key entirely (PATCH partial updates / Create defaults).
 *
 * The `chaos_mode` field is the only [new] addition (OQ-2) — additive + defaulted.
 */
import { z } from 'zod'

export const chaosModeSchema = z.enum(['error', 'dropout'])
export const defaultModeSchema = z.enum(['mock_404', 'echo'])
export const servedBySchema = z.enum([
  'rule',
  'crud',
  'mitm',
  'tunnel',
  'default',
  'cors',
  'chaos',
  'ratelimit',
])

// ── Session / endpoints ──
export const sessionCreateSchema = z.object({ email: z.string() })

export const endpointSummarySchema = z.object({
  token: z.string(),
  name: z.string().nullable(),
  mock_url: z.string(),
  path_url: z.string(),
  created_at: z.string(),
  last_hit: z.string().nullable(),
  request_count: z.number().int(),
})

export const sessionResponseSchema = z.object({
  owner_id: z.string(),
  owner_secret: z.string(),
  endpoints: z.array(endpointSummarySchema),
  primary: endpointSummarySchema,
})

export const endpointCreateSchema = z.object({ name: z.string().nullable().optional() })

export const endpointConfigPatchSchema = z.object({
  name: z.string().nullable().optional(),
  auto_crud: z.boolean().optional(),
  target_url: z.string().nullable().optional(),
  default_mode: defaultModeSchema.optional(),
  latency_ms: z.number().int().optional(),
  rate_limit_per_min: z.number().int().optional(),
  chaos_pct: z.number().int().optional(),
  chaos_mode: chaosModeSchema.optional(),
  cors_enabled: z.boolean().optional(),
})

export const endpointDetailSchema = z.object({
  token: z.string(),
  name: z.string().nullable(),
  mock_url: z.string(),
  path_url: z.string(),
  auto_crud: z.boolean(),
  target_url: z.string().nullable(),
  default_mode: defaultModeSchema,
  latency_ms: z.number().int(),
  rate_limit_per_min: z.number().int(),
  chaos_pct: z.number().int(),
  chaos_mode: chaosModeSchema,
  cors_enabled: z.boolean(),
  tunnel_active: z.boolean(),
  created_at: z.string(),
  last_hit: z.string().nullable(),
  request_count: z.number().int(),
})

// ── Rule shapes ──
export const bodyConditionSchema = z.object({
  path: z.string(),
  op: z.enum(['eq', 'neq', 'contains', 'exists']),
  value: z.string().nullable(),
})

export const stateRequirementSchema = z.object({
  key: z.string(),
  op: z.enum(['eq', 'neq', 'exists', 'absent']),
  value: z.string().nullable(),
})

export const matchCriteriaSchema = z.object({
  method: z.string().default('ANY'),
  path: z.string().default('/*'),
  headers: z.record(z.string()).default({}),
  query: z.record(z.string()).default({}),
  body_conditions: z.array(bodyConditionSchema).default([]),
  state_requirements: z.array(stateRequirementSchema).default([]),
})

export const stateWriteSchema = z.object({ key: z.string(), value: z.string() })

export const responseSpecSchema = z.object({
  status_code: z.number().int().default(200),
  headers: z.record(z.string()).default({}),
  body_template: z.string().default(''),
  content_type: z.string().default('application/json'),
})

export const webhookActionSchema = z.object({
  url: z.string(),
  body_template: z.string().default(''),
})

export const mockRuleCreateSchema = z.object({
  name: z.string().nullable().optional(),
  priority: z.number().int().default(100),
  enabled: z.boolean().default(true),
  match: matchCriteriaSchema.default({}),
  response: responseSpecSchema.default({}),
  state_writes: z.array(stateWriteSchema).default([]),
  latency_ms: z.number().int().nullable().optional(),
  rate_limit_per_min: z.number().int().nullable().optional(),
  chaos_mode: chaosModeSchema.nullable().optional(),
  webhook_action: webhookActionSchema.nullable().optional(),
})

// MockRulePatch = MockRuleCreate, all fields optional.
export const mockRulePatchSchema = mockRuleCreateSchema.partial()

// MockRule = MockRuleCreate + { id, token, created_at }.
export const mockRuleSchema = z.object({
  id: z.number().int(),
  token: z.string(),
  name: z.string().nullable(),
  priority: z.number().int(),
  enabled: z.boolean(),
  match: matchCriteriaSchema,
  response: responseSpecSchema,
  state_writes: z.array(stateWriteSchema),
  latency_ms: z.number().int().nullable(),
  rate_limit_per_min: z.number().int().nullable(),
  chaos_mode: chaosModeSchema.nullable(),
  webhook_action: webhookActionSchema.nullable(),
  created_at: z.string(),
})

// ── Traces ──
export const requestSummarySchema = z.object({
  id: z.number().int(),
  token: z.string(),
  method: z.string(),
  path: z.string(),
  status_code: z.number().int(),
  served_by: servedBySchema,
  matched_rule_id: z.number().int().nullable(),
  duration_ms: z.number().int(),
  overhead_ms: z.number().int(),
  timestamp: z.string(),
})

export const traceEventSchema = z.object({ step: z.string(), detail: z.string() })

export const requestDetailSchema = requestSummarySchema.extend({
  request_headers: z.record(z.string()),
  query_params: z.record(z.string()),
  request_body: z.string().nullable(),
  response_headers: z.record(z.string()),
  response_body: z.string().nullable(),
  trace: z.array(traceEventSchema),
  state_snapshot: z.record(z.string()),
})

export const messageSchema = z.object({
  message: z.string(),
  success: z.boolean().default(true),
})

export const stateResponseSchema = z.object({ state: z.record(z.string()) })
export const collectionResponseSchema = z.object({ items: z.array(z.record(z.unknown())) })

// Flat error envelope (AC-60): {"error": "<code>", "detail": "<human>"}.
export const errorEnvelopeSchema = z.object({
  error: z.string(),
  detail: z.string().optional(),
})

// ── Inferred TS types (the TS half of the §5.3 contract) ──
export type SessionCreate = z.infer<typeof sessionCreateSchema>
export type EndpointSummary = z.infer<typeof endpointSummarySchema>
export type SessionResponse = z.infer<typeof sessionResponseSchema>
export type EndpointCreate = z.infer<typeof endpointCreateSchema>
export type EndpointConfigPatch = z.infer<typeof endpointConfigPatchSchema>
export type EndpointDetail = z.infer<typeof endpointDetailSchema>
export type BodyCondition = z.infer<typeof bodyConditionSchema>
export type StateRequirement = z.infer<typeof stateRequirementSchema>
export type MatchCriteria = z.infer<typeof matchCriteriaSchema>
export type StateWrite = z.infer<typeof stateWriteSchema>
export type ResponseSpec = z.infer<typeof responseSpecSchema>
export type WebhookAction = z.infer<typeof webhookActionSchema>
export type MockRuleCreate = z.infer<typeof mockRuleCreateSchema>
export type MockRulePatch = z.infer<typeof mockRulePatchSchema>
export type MockRule = z.infer<typeof mockRuleSchema>
export type RequestSummary = z.infer<typeof requestSummarySchema>
export type TraceEvent = z.infer<typeof traceEventSchema>
export type RequestDetail = z.infer<typeof requestDetailSchema>
export type Message = z.infer<typeof messageSchema>
export type ChaosMode = z.infer<typeof chaosModeSchema>
export type DefaultMode = z.infer<typeof defaultModeSchema>
export type ServedBy = z.infer<typeof servedBySchema>
export type ErrorEnvelope = z.infer<typeof errorEnvelopeSchema>
