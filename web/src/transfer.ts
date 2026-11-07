import { megabitsPerSecond } from './metrics.ts'
import { measurementMs } from './protocol.ts'
import type { Failure, TracePoint } from './protocol.ts'

export class TestFailure extends Error {
  readonly reason: Failure

  constructor(reason: Failure) {
    super(reason)
    this.reason = reason
  }
}

export function requireStatus(status: number, expected: number) {
  if (status === 401 || status === 403) throw new TestFailure('unauthorized')
  if (status !== expected) throw new TestFailure('response')
}

type TransferRequest = (signal: AbortSignal, countBytes: (bytes: number) => void) => Promise<void>

export function makeDownload(url: string): TransferRequest {
  return async (signal, countBytes) => {
    const response = await fetch(url, { credentials: 'same-origin', cache: 'no-store', signal })
    requireStatus(response.status, 200)
    if (!response.body) throw new TestFailure('response')

    const reader = response.body.getReader()
    let bytes = 0
    try {
      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        bytes += value.byteLength
        countBytes(value.byteLength)
      }
    } finally {
      reader.releaseLock()
    }
    if (bytes === 0) throw new TestFailure('response')
  }
}

export function makeUpload(url: string): TransferRequest {
  // Reuse one incompressible payload across lanes. Crypto limits each fill to 64 KiB.
  const bytes = new Uint8Array(4 * 1024 * 1024)
  for (let start = 0; start < bytes.length; start += 65536) {
    crypto.getRandomValues(bytes.subarray(start, start + 65536))
  }
  const body = new Blob([bytes], { type: 'application/octet-stream' })

  return (signal, countBytes) => new Promise((resolve, reject) => {
    signal.throwIfAborted()

    const request = new XMLHttpRequest()
    request.open('POST', url)
    const abort = () => request.abort()
    let sent = 0
    function countProgress(loaded: number) {
      const next = Math.max(sent, Math.min(loaded, body.size))
      countBytes(next - sent)
      sent = next
    }

    request.upload.onprogress = (event) => countProgress(event.loaded)
    request.onload = () => {
      try {
        requireStatus(request.status, 204)
        countProgress(body.size)
        resolve()
      } catch (error) {
        reject(error)
      }
    }
    request.onerror = () => reject(new TestFailure('network'))
    request.onabort = () => reject(signal.reason)
    request.onloadend = () => signal.removeEventListener('abort', abort)

    signal.addEventListener('abort', abort, { once: true })
    request.send(body)
  })
}

const warmupMs = 1200
const sampleMs = 250

function wait(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    signal.throwIfAborted()
    const abort = () => {
      clearTimeout(timer)
      reject(signal.reason)
    }
    const timer = setTimeout(() => {
      signal.removeEventListener('abort', abort)
      resolve()
    }, ms)
    signal.addEventListener('abort', abort, { once: true })
  })
}

export async function measureTransfer(
  request: TransferRequest,
  parallel: number,
  onSample: (point: TracePoint, progress: number) => void,
) {
  const controller = new AbortController()
  const { signal } = controller
  let bytes = 0
  const lanes = Promise.all(Array.from({ length: parallel }, async () => {
    while (!signal.aborted) {
      await request(signal, (count) => { bytes += count })
    }
  }))

  // Lanes run continuously while the timed sampler measures only post-warmup bytes.
  async function sample() {
    await wait(warmupMs, signal)
    const started = performance.now()
    const baseline = bytes
    let previousTime = started
    let previousBytes = baseline

    while (true) {
      await wait(sampleMs, signal)
      const now = performance.now()
      const elapsed = now - started
      const rate = megabitsPerSecond(bytes - previousBytes, now - previousTime)
      onSample({ elapsed: elapsed / 1000, mbps: rate }, Math.min(elapsed / measurementMs, 1))

      if (elapsed >= measurementMs) {
        if (bytes === baseline) throw new TestFailure('timeout')
        return megabitsPerSecond(bytes - baseline, elapsed)
      }
      previousTime = now
      previousBytes = bytes
    }
  }

  try {
    return await Promise.race([sample(), lanes.then(() => { throw new TestFailure('network') })])
  } finally {
    controller.abort()
  }
}
