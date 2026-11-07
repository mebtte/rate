import { summarizeLatency } from './metrics.ts'
import type { TestCommand, TestMessage } from './protocol.ts'
import { makeDownload, makeUpload, measureTransfer, requireStatus, TestFailure } from './transfer.ts'

declare const self: DedicatedWorkerGlobalScope

function send(message: TestMessage) {
  self.postMessage(message)
}

async function measureLatency(url: string) {
  const samples: number[] = []
  const signal = AbortSignal.timeout(12_000)
  for (let attempt = 0; attempt <= 10; attempt += 1) {
    const started = performance.now()
    const response = await fetch(url, { credentials: 'same-origin', cache: 'no-store', signal })
    requireStatus(response.status, 204)
    // Discard the first request so connection setup does not bias the round trips.
    if (attempt > 0) samples.push(performance.now() - started)
    send({ type: 'progress', progress: attempt / 10 })
  }
  return summarizeLatency(samples)
}

self.onmessage = async (event: MessageEvent<TestCommand>) => {
  const urls = event.data
  try {
    send({ type: 'phase', phase: 'latency' })
    const latency = await measureLatency(urls.ping)
    send({ type: 'latency', ...latency })

    send({ type: 'phase', phase: 'download' })
    const download = await measureTransfer(makeDownload(urls.download), 4, (point, progress) => {
      send({ type: 'sample', phase: 'download', point, progress })
    })
    send({ type: 'transfer', phase: 'download', mbps: download })

    send({ type: 'phase', phase: 'upload' })
    const upload = await measureTransfer(makeUpload(urls.upload), 3, (point, progress) => {
      send({ type: 'sample', phase: 'upload', point, progress })
    })
    send({ type: 'transfer', phase: 'upload', mbps: upload })
    send({ type: 'done' })
  } catch (error) {
    const reason = error instanceof TestFailure ? error.reason
      : error instanceof DOMException && error.name === 'TimeoutError' ? 'timeout'
      : 'network'
    send({ type: 'error', reason })
  }
}
