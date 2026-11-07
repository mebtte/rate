import assert from 'node:assert/strict'
import test from 'node:test'
import { testEndpoints } from '../src/api.ts'
import { chooseLanguage } from '../src/messages.ts'
import { megabitsPerSecond, summarizeLatency } from '../src/metrics.ts'
import { measurementMs } from '../src/protocol.ts'
import type { TracePoint } from '../src/protocol.ts'
import { makeDownload, measureTransfer, requireStatus, TestFailure } from '../src/transfer.ts'

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

test('live throughput averages post-warmup bytes and converges to the final result', async ({ mock }) => {
  mock.timers.enable({ apis: ['setTimeout', 'Date'] })
  mock.method(performance, 'now', () => Date.now())

  let countBytes!: (bytes: number) => void
  const samples: { point: TracePoint; progress: number }[] = []
  const result = measureTransfer((signal, count) => {
    countBytes = count
    return new Promise<void>((resolve) => signal.addEventListener('abort', () => resolve(), { once: true }))
  }, 1, (point, progress) => samples.push({ point, progress }))

  async function tick(ms: number) {
    mock.timers.tick(ms)
    // Resume the sampler so it can schedule the next tick against the fake clock.
    await Promise.resolve()
  }

  countBytes(2_000_000)
  await tick(1200)
  assert.equal(samples.length, 0)

  countBytes(1_000_000)
  await tick(250)
  await tick(250)
  countBytes(500_000)
  await tick(250)
  assert.deepEqual(samples, [
    { point: { elapsed: 0.25, mbps: 32 }, progress: 250 / measurementMs },
    { point: { elapsed: 0.5, mbps: 16 }, progress: 500 / measurementMs },
    { point: { elapsed: 0.75, mbps: 16 }, progress: 750 / measurementMs },
  ])

  for (let elapsed = 750; elapsed < measurementMs; elapsed += 250) await tick(250)
  const finalRate = await result
  assert.equal(finalRate, megabitsPerSecond(1_500_000, measurementMs))
  assert.deepEqual(samples.at(-1), {
    point: { elapsed: measurementMs / 1000, mbps: finalRate },
    progress: 1,
  })
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
