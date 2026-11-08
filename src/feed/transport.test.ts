import { describe, expect, it } from 'vitest'
import { feedEventSourceUrl, feedSocketUrl } from './transport'

describe('feed transport URLs', () => {
  it('builds the WebSocket URL on the page host with the matching scheme', () => {
    expect(feedSocketUrl('ab 12', { protocol: 'http:', host: 'localhost:5173' })).toBe('ws://localhost:5173/ws/ab%2012')
    expect(feedSocketUrl('tok', { protocol: 'https:', host: 'hookbox.dev' })).toBe('wss://hookbox.dev/ws/tok')
  })

  it('defaults to window.location', () => {
    expect(feedSocketUrl('tok')).toBe(`ws://${window.location.host}/ws/tok`)
  })

  it('builds the same-origin SSE URL', () => {
    expect(feedEventSourceUrl('ab/12')).toBe('/sse/ab%2F12')
  })

  it('never puts a credential in the URL (the cookie rides along instead)', () => {
    expect(feedSocketUrl('tok')).not.toContain('?')
    expect(feedEventSourceUrl('tok')).not.toContain('?')
  })
})
