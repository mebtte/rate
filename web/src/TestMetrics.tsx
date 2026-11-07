import type { CSSProperties } from 'react'
import { Icon } from './Icon.tsx'
import type { Messages } from './messages.ts'
import type { Phase, Results, TracePoint, TransferPhase } from './protocol.ts'

const metrics = ['download', 'upload', 'latency', 'jitter'] as const

interface TestMetricsProps {
  copy: Messages
  phase?: Phase
  running: boolean
  progress: number
  results: Results
  trace?: { phase: TransferPhase; points: readonly TracePoint[] }
}

export function TestMetrics({ copy, phase, running, progress, results, trace }: TestMetricsProps) {
  // The chart keeps the previous trace until the next transfer has a sample.
  const liveTrace = running && trace?.phase === phase ? trace : undefined

  return (
    <dl className="rate-results" aria-label={copy.results}>
      {metrics.map((metric) => {
        // Jitter and latency come from the same samples.
        const metricPhase = metric === 'jitter' ? 'latency' : metric
        const result = results[metric]
        const reading = result ?? (liveTrace?.phase === metric ? liveTrace.points.at(-1)?.mbps : undefined)
        const completion = result !== undefined ? 1 : phase === metricPhase ? progress : 0
        const unit = metric === 'download' || metric === 'upload' ? 'Mbps' : 'ms'

        return (
          <div key={metric} data-active={running && phase === metricPhase ? true : undefined}
            aria-current={running && phase === metric ? 'step' : undefined}>
            <dt><Icon name={metric === 'jitter' ? 'activity' : metric} />{copy[metric]}</dt>
            <dd>
              <strong>{reading?.toFixed(1) ?? '—'}</strong><span>{unit}</span>
              <span className="rate-metric-progress" aria-hidden="true"
                style={{ '--rate-progress': `${completion * 100}%` } as CSSProperties} />
            </dd>
          </div>
        )
      })}
    </dl>
  )
}
