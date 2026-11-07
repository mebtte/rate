export function summarizeLatency(samples: readonly number[]) {
  if (samples.length === 0) throw new RangeError('At least one latency sample is required')

  const sorted = [...samples].sort((a, b) => a - b)
  const middle = Math.floor(sorted.length / 2)
  const latency = sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2

  // Jitter is the average absolute difference between consecutive round trips.
  const totalVariation = samples.slice(1)
    .reduce((sum, sample, index) => sum + Math.abs(sample - samples[index]), 0)
  const jitter = totalVariation / Math.max(samples.length - 1, 1)

  return { latency, jitter }
}

export function megabitsPerSecond(bytes: number, milliseconds: number) {
  return milliseconds > 0 ? bytes * 8 / milliseconds / 1000 : 0
}
