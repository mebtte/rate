import type { TestEndpoints } from './api.ts'

export const measurementMs = 7000

export type Phase = 'latency' | 'download' | 'upload'
export type TransferPhase = Exclude<Phase, 'latency'>
export type Failure = 'network' | 'timeout' | 'unauthorized' | 'response'
export type Results = Partial<Record<Phase | 'jitter', number>>

export interface TracePoint {
  elapsed: number
  mbps: number
}

export type TestMessage =
  | { type: 'phase'; phase: Phase }
  | { type: 'progress'; progress: number }
  | { type: 'sample'; phase: TransferPhase; progress: number; point: TracePoint }
  | { type: 'latency'; latency: number; jitter: number }
  | { type: 'transfer'; phase: TransferPhase; mbps: number }
  | { type: 'done' }
  | { type: 'error'; reason: Failure }

export type TestCommand = TestEndpoints
