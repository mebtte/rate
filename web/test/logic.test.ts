import assert from 'node:assert/strict'
import test from 'node:test'
import { testEndpoints } from '../src/api.ts'
import { chooseLanguage } from '../src/messages.ts'
import { megabitsPerSecond, summarizeLatency } from '../src/metrics.ts'
import { makeDownload, requireStatus, TestFailure } from '../src/transfer.ts'

test('test URLs stay at the document origin and mount prefix', () => {
  assert.equal(testEndpoints('https://example.test/').ping, 'https://example.test/api/v1/ping')
  assert.deepEqual(testEndpoints('https://example.test/tools/rate/?lang=en#test'), {
    client: 'https://example.test/tools/rate/api/v1/client',
    ping: 'https://example.test/tools/rate/api/v1/ping',
    download: 'https://example.test/tools/rate/api/v1/download?bytes=8388608',
    upload: 'https://example.test/tools/rate/api/v1/upload',
  })
})

test('latency uses the median and jitter uses consecutive samples', () => {
  const samples = [12, 20, 10, 14]
  assert.deepEqual(summarizeLatency(samples), { latency: 13, jitter: 22 / 3 })
  assert.deepEqual(samples, [12, 20, 10, 14])
  assert.deepEqual(summarizeLatency([7]), { latency: 7, jitter: 0 })
  assert.throws(() => summarizeLatency([]), RangeError)
})

test('throughput reports decimal megabits per second', () => {
  assert.equal(megabitsPerSecond(1_000_000, 500), 16)
  assert.equal(megabitsPerSecond(100, 0), 0)
})

test('selects the first supported browser language and falls back to English', () => {
  assert.equal(chooseLanguage(['en-US', 'zh-CN']), 'en')
  assert.equal(chooseLanguage(['fr-FR', 'zh-HK']), 'zh-CN')
  assert.equal(chooseLanguage(['zh-CN', 'en-US']), 'zh-CN')
  assert.equal(chooseLanguage(['fr-FR']), 'en')
  assert.equal(chooseLanguage([]), 'en')
})

test('the request protocol distinguishes authentication and unexpected responses', () => {
  assert.doesNotThrow(() => requireStatus(204, 204))
  assert.throws(() => requireStatus(401, 204), (error) => error instanceof TestFailure && error.reason === 'unauthorized')
  assert.throws(() => requireStatus(413, 204), (error) => error instanceof TestFailure && error.reason === 'response')
})

test('download counts streamed chunks with same-origin credentials', async ({ mock }) => {
  const signal = new AbortController().signal
  const fetch = mock.method(globalThis, 'fetch', async () => new Response(new ReadableStream({
    start(controller) {
      controller.enqueue(new Uint8Array(3))
      controller.enqueue(new Uint8Array(5))
      controller.close()
    },
  })))
  const url = 'https://example.test/rate/api/v1/download?bytes=8'
  const counts: number[] = []

  await makeDownload(url)(signal, (bytes) => counts.push(bytes))

  assert.deepEqual(counts, [3, 5])
  assert.deepEqual(fetch.mock.calls[0].arguments, [url, {
    credentials: 'same-origin',
    cache: 'no-store',
    signal,
  }])
})
