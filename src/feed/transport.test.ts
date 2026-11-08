import { describe, expect, it } from 'vitest'
import { feedEventSourceUrl, feedSocketUrl } from './transport'

describe('feed transport URLs', () => {
  it('builds the WebSocket URL on the page host with the matching scheme', () => {
    expect(feedSocketUrl('ab 12', 's/ecret', { protocol: 'http:', host: 'localhost:5173' })).toBe(
      'ws://localhost:5173/ws/ab%2012?cap=s%2Fecret',
    )
    expect(feedSocketUrl('tok', 'k', { protocol: 'https:', host: 'hookbox.dev' })).toBe(
      'wss://hookbox.dev/ws/tok?cap=k',
    )
  })

  it('defaults to window.location', () => {
    expect(feedSocketUrl('tok', 'k')).toBe(`ws://${window.location.host}/ws/tok?cap=k`)
  })

  it('builds the same-origin SSE URL', () => {
    expect(feedEventSourceUrl('ab/12', 'a&b')).toBe('/sse/ab%2F12?cap=a%26b')
  })
})
