import { describe, expect, it } from 'vitest'
import { ApiError } from '@/api/errors'
import { errorMessage, isEndpointGone, remapEndpointGone } from './api-errors'
import { t } from './copy'

const GONE = t('common.error.endpointGone')

describe('isEndpointGone', () => {
  it('is true only for a 404/410 ApiError', () => {
    expect(isEndpointGone(new ApiError('not_found', 'x', 404))).toBe(true)
    expect(isEndpointGone(new ApiError('gone', 'x', 410))).toBe(true)
    expect(isEndpointGone(new ApiError('server', 'x', 500))).toBe(false)
    expect(isEndpointGone(new Error('404'))).toBe(false)
    expect(isEndpointGone(undefined)).toBe(false)
  })
})

describe('errorMessage', () => {
  it("prefers the server's detail on an ApiError", () => {
    expect(errorMessage(new ApiError('bad', 'Name too long', 422), 'fallback')).toBe('Name too long')
  })

  it('falls back for non-ApiErrors and an empty detail', () => {
    expect(errorMessage(new Error('boom'), 'fallback')).toBe('fallback')
    expect(errorMessage(new ApiError('bad', '', 500), 'fallback')).toBe('fallback')
    expect(errorMessage('weird', 'fallback')).toBe('fallback')
  })

  it('keeps the detail of a 404/410 unless mapGone is set', () => {
    const gone = new ApiError('gone', 'endpoint ab12 is gone', 410)
    expect(errorMessage(gone, 'fallback')).toBe('endpoint ab12 is gone')
    expect(errorMessage(gone, 'fallback', { mapGone: true })).toBe(GONE)
    expect(errorMessage(new ApiError('nf', 'x', 404), 'fallback', { mapGone: true })).toBe(GONE)
  })
})

describe('remapEndpointGone', () => {
  it('rethrows a 404/410 with the shared copy, keeping code and status', () => {
    const thrown = (() => {
      try {
        remapEndpointGone(new ApiError('gone', 'raw detail', 410))
      } catch (err) {
        return err
      }
    })()
    expect(thrown).toBeInstanceOf(ApiError)
    expect(thrown).toMatchObject({ code: 'gone', status: 410, message: GONE })
  })

  it('rethrows anything else unchanged', () => {
    const original = new ApiError('server', 'boom', 500)
    expect(() => remapEndpointGone(original)).toThrow(original)
    const plain = new Error('x')
    expect(() => remapEndpointGone(plain)).toThrow(plain)
  })
})
