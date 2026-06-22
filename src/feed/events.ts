/**
 * §5.4 live-feed wire events. WS frames are {"type":"<event>","data":{...}}.
 * SSE frames put <event> on the `event:` line and the inner data object on the
 * `data:` line. Both decode to the same FeedEvent union here.
 */
import { z } from 'zod'
import { requestSummarySchema, type RequestSummary } from '@/api/schemas'

export const helloSchema = z.object({ token: z.string(), server_time: z.string() })
export const stateChangedSchema = z.object({ token: z.string(), key: z.string(), value: z.string() })
export const endpointUpdatedSchema = z.object({ token: z.string(), fields: z.array(z.string()) })

export type FeedEvent =
  | { type: 'hello'; data: z.infer<typeof helloSchema> }
  | { type: 'new_request'; data: RequestSummary }
  | { type: 'state_changed'; data: z.infer<typeof stateChangedSchema> }
  | { type: 'endpoint_updated'; data: z.infer<typeof endpointUpdatedSchema> }

/** Decode a WS JSON text frame ({type,data}) → FeedEvent, or null if unknown. */
export function decodeWsFrame(raw: string): FeedEvent | null {
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return null
  }
  const env = z.object({ type: z.string(), data: z.unknown() }).safeParse(parsed)
  if (!env.success) return null
  return decode(env.data.type, env.data.data)
}

/** Decode an SSE event (type from the `event:` line, data = parsed JSON). */
export function decodeSseEvent(type: string, dataJson: string): FeedEvent | null {
  let data: unknown
  try {
    data = JSON.parse(dataJson)
  } catch {
    return null
  }
  return decode(type, data)
}

function decode(type: string, data: unknown): FeedEvent | null {
  switch (type) {
    case 'hello': {
      const r = helloSchema.safeParse(data)
      return r.success ? { type: 'hello', data: r.data } : null
    }
    case 'new_request': {
      const r = requestSummarySchema.safeParse(data)
      return r.success ? { type: 'new_request', data: r.data } : null
    }
    case 'state_changed': {
      const r = stateChangedSchema.safeParse(data)
      return r.success ? { type: 'state_changed', data: r.data } : null
    }
    case 'endpoint_updated': {
      const r = endpointUpdatedSchema.safeParse(data)
      return r.success ? { type: 'endpoint_updated', data: r.data } : null
    }
    default:
      return null
  }
}
