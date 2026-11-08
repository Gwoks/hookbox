import { describe, expect, it, vi } from 'vitest'
import { createSessionStore, type SessionChannel } from './session'

/** In-memory stand-in for BroadcastChannel: records what this tab posts and
 * lets a test deliver a message as if another tab had posted it. */
class FakeChannel implements SessionChannel {
  onmessage: ((event: MessageEvent) => void) | null = null
  posted: unknown[] = []
  postMessage(message: unknown): void {
    this.posted.push(message)
  }
  deliver(data: unknown): void {
    this.onmessage?.({ data } as MessageEvent)
  }
}

describe('session store', () => {
  it('starts unknown with no owner', () => {
    const store = createSessionStore(null)
    expect(store.getSnapshot()).toEqual({ status: 'unknown', ownerId: null })
  })

  it('moves between unknown, signed-in and signed-out', () => {
    const store = createSessionStore(null)
    store.markSignedIn('owner_a')
    expect(store.getSnapshot()).toEqual({ status: 'signed-in', ownerId: 'owner_a' })
    store.markSignedOut()
    expect(store.getSnapshot()).toEqual({ status: 'signed-out', ownerId: null })
    store.invalidate()
    expect(store.getSnapshot()).toEqual({ status: 'unknown', ownerId: null })
  })

  it('keeps the same snapshot object until something changes', () => {
    const store = createSessionStore(null)
    store.markSignedIn('owner_a')
    const first = store.getSnapshot()
    store.markSignedIn('owner_a')
    expect(store.getSnapshot()).toBe(first)
    store.markSignedIn('owner_b')
    expect(store.getSnapshot()).not.toBe(first)
  })

  it('notifies subscribers only on a real change, and stops after unsubscribe', () => {
    const store = createSessionStore(null)
    const listener = vi.fn()
    const unsubscribe = store.subscribe(listener)
    store.markSignedIn('owner_a')
    store.markSignedIn('owner_a')
    expect(listener).toHaveBeenCalledTimes(1)
    unsubscribe()
    store.markSignedOut()
    expect(listener).toHaveBeenCalledTimes(1)
  })

  it('never stores a key: the snapshot carries only status and owner id', () => {
    const store = createSessionStore(null)
    store.markSignedIn('owner_a')
    expect(Object.keys(store.getSnapshot()).sort()).toEqual(['ownerId', 'status'])
  })
})

describe('session store cross-tab sync', () => {
  it('posts only when asked to broadcast', () => {
    const channel = new FakeChannel()
    const store = createSessionStore(channel)
    store.markSignedIn('owner_a')
    store.markSignedOut()
    expect(channel.posted).toEqual([])
    store.markSignedIn('owner_b', { broadcast: true })
    store.markSignedOut({ broadcast: true })
    expect(channel.posted).toEqual([{ type: 'signed-in', ownerId: 'owner_b' }, { type: 'signed-out' }])
  })

  it('signs this tab out when another tab signs out, without re-broadcasting', () => {
    const channel = new FakeChannel()
    const store = createSessionStore(channel)
    store.markSignedIn('owner_a')
    channel.deliver({ type: 'signed-out' })
    expect(store.getSnapshot()).toEqual({ status: 'signed-out', ownerId: null })
    expect(channel.posted).toEqual([])
  })

  it('invalidates when another tab signs in as a different owner', () => {
    const channel = new FakeChannel()
    const store = createSessionStore(channel)
    store.markSignedIn('owner_a')
    channel.deliver({ type: 'signed-in', ownerId: 'owner_b' })
    expect(store.getSnapshot()).toEqual({ status: 'unknown', ownerId: null })
  })

  it('invalidates a signed-out tab when another tab signs in', () => {
    const channel = new FakeChannel()
    const store = createSessionStore(channel)
    store.markSignedOut()
    channel.deliver({ type: 'signed-in', ownerId: 'owner_b' })
    expect(store.getSnapshot().status).toBe('unknown')
  })

  it('ignores a sign-in for the owner this tab already has', () => {
    const channel = new FakeChannel()
    const store = createSessionStore(channel)
    store.markSignedIn('owner_a')
    const before = store.getSnapshot()
    channel.deliver({ type: 'signed-in', ownerId: 'owner_a' })
    expect(store.getSnapshot()).toBe(before)
  })

  it('ignores malformed messages', () => {
    const channel = new FakeChannel()
    const store = createSessionStore(channel)
    store.markSignedIn('owner_a')
    const before = store.getSnapshot()
    for (const junk of [null, 'signed-out', { type: 'signed-in' }, { type: 'signed-in', ownerId: 5 }, { type: 'x' }]) {
      channel.deliver(junk)
    }
    expect(store.getSnapshot()).toBe(before)
  })
})
