import { useEffect, useRef, useState } from 'react'
import type { TestEndpoints } from './api.ts'
import type { Failure, Phase, Results, TestMessage, TracePoint, TransferPhase } from './protocol.ts'

interface Run {
  status: 'idle' | 'running' | 'canceled' | 'done' | 'error'
  phase?: Phase
  progress: number
  results: Results
  trace?: { phase: TransferPhase; points: TracePoint[] }
  error?: Failure
}

const initial: Run = { status: 'idle', progress: 0, results: {} }

function applyMessage(run: Run, message: TestMessage): Run {
  switch (message.type) {
    case 'phase':
      return { ...run, phase: message.phase, progress: 0 }
    case 'progress':
      return { ...run, progress: message.progress }
    case 'sample': {
      const points = run.trace?.phase === message.phase ? run.trace.points : []
      return {
        ...run,
        progress: message.progress,
        trace: { phase: message.phase, points: [...points, message.point] },
      }
    }
    case 'latency':
      return { ...run, results: { ...run.results, latency: message.latency, jitter: message.jitter } }
    case 'transfer':
      return { ...run, results: { ...run.results, [message.phase]: message.mbps } }
    case 'done':
      return { ...run, status: 'done' }
    case 'error':
      return { ...run, status: 'error', error: message.reason }
  }
}

export function useSpeedTest(endpoints: TestEndpoints) {
  const active = useRef<Worker | null>(null)
  const [run, setRun] = useState(initial)

  useEffect(() => () => {
    active.current?.terminate()
    active.current = null
  }, [])

  function toggle() {
    if (active.current) {
      // Each run owns its Worker. Canceling also stops every in-flight request.
      active.current.terminate()
      active.current = null
      setRun((current) => ({ ...current, status: 'canceled' }))
      return
    }

    setRun({ ...initial, status: 'running', phase: 'latency' })
    try {
      const worker = new Worker(new URL('./speed.worker.ts', import.meta.url), { type: 'module' })
      active.current = worker

      function receive(message: TestMessage) {
        if (active.current !== worker) return
        if (message.type === 'done' || message.type === 'error') {
          active.current = null
          worker.terminate()
        }
        setRun((current) => applyMessage(current, message))
      }

      worker.onmessage = (event: MessageEvent<TestMessage>) => receive(event.data)
      worker.onerror = () => receive({ type: 'error', reason: 'network' })
      worker.postMessage(endpoints)
    } catch {
      active.current?.terminate()
      active.current = null
      setRun((current) => applyMessage(current, { type: 'error', reason: 'network' }))
    }
  }

  return { run, toggle }
}
