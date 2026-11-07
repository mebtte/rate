import assert from 'node:assert/strict'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import { build } from 'vite'

test('production CSS contains usable light and dark theme colors', async () => {
  const result = await build({
    root: fileURLToPath(new URL('..', import.meta.url)),
    logLevel: 'silent',
    build: { write: false },
  })
  const bundles = Array.isArray(result) ? result : [result]
  const css = bundles
    .flatMap((bundle) => 'output' in bundle ? bundle.output : [])
    .filter((item) => item.type === 'asset' && item.fileName.endsWith('.css'))
    .map((item) => String(item.source))
    .join('\n')
  assert.ok(css, 'The production build must emit a stylesheet')

  const light = /\.rate\s*\{([^{}]+)\}/.exec(css)?.[1]
  const dark = /@media\s*\(\s*prefers-color-scheme\s*:\s*dark\s*\)\s*\{\s*\.rate\s*\{([^{}]+)\}/.exec(css)?.[1]
  assert.ok(light && dark, 'The optimized stylesheet must include both palettes')

  function hexColor(rule: string, token: string) {
    const value = rule.split(';').find((declaration) => declaration.trim().startsWith(token + ':'))
      ?.split(':')[1].trim()
    // Each token must substitute a single color after optimization.
    assert.match(value ?? '', /^#(?:[\da-f]{3,4}|[\da-f]{6}|[\da-f]{8})$/i, token)
    return value
  }

  for (const token of ['--rate-page', '--rate-signal']) {
    assert.notEqual(hexColor(light, token), hexColor(dark, token))
  }
})
