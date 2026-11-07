import type { Messages } from './messages.ts'
import { measurementMs } from './protocol.ts'
import type { TracePoint, TransferPhase } from './protocol.ts'

const plotWidth = 360
const plotHeight = 84

interface ChartProps {
  copy: Messages
  trace?: { phase: TransferPhase; points: TracePoint[] }
}

function tracePath(points: readonly TracePoint[], max: number, duration: number) {
  if (points.length === 0) return {}
  const coordinates = points.map((point) => [
    Math.min(point.elapsed / duration, 1) * plotWidth,
    plotHeight * (1 - point.mbps / max),
  ])
  const line = `M${coordinates.map(([x, y]) => `${x} ${y}`).join(' L')}`
  return {
    line,
    fill: `${line} L${coordinates.at(-1)![0]} ${plotHeight} H${coordinates[0][0]} Z`,
  }
}

export function SpeedChart({ copy, trace }: ChartProps) {
  const title = trace ? copy.transferRate[trace.phase] : copy.liveRate
  const peak = trace ? Math.max(...trace.points.map((point) => point.mbps), 1) : 1
  const magnitude = 10 ** Math.floor(Math.log10(peak))
  const max = Math.ceil(peak / magnitude) * magnitude
  const last = trace?.points.at(-1)
  const duration = Math.max(measurementMs / 1000, last?.elapsed ?? 0)
  const paths = tracePath(trace?.points ?? [], max, duration)

  return (
    <section className="rate-chart" aria-label={title}>
      <div className="rate-chart-header">
        <h2>{title}</h2>
        <span className="rate-axis-unit">Mbps</span>
      </div>
      <div className="rate-plot" aria-hidden="true">
        <div className="rate-y">
          <span>{trace ? max : undefined}</span>
          <span>{trace ? max / 2 : undefined}</span>
          <span>0</span>
        </div>
        <div className="rate-trace">
          <svg viewBox={`0 0 ${plotWidth} ${plotHeight}`} preserveAspectRatio="none">
            <path className="rate-gridlines" d={`M0 0H${plotWidth} M0 ${plotHeight / 2}H${plotWidth}`} fill="none" />
            <path className="rate-gridlines rate-grid-baseline" d={`M0 ${plotHeight}H${plotWidth}`} fill="none" />
            {paths.fill && <path className="rate-fill" d={paths.fill} />}
            {paths.line && <path className="rate-line" d={paths.line} fill="none" />}
          </svg>
        </div>
        <div className="rate-x"><span>0 s</span><span>{trace ? `${duration.toFixed(1)} s` : copy.time}</span></div>
      </div>
    </section>
  )
}
