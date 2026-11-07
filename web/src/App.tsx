import { useEffect, useMemo, useState } from 'react'
import { fetchClient, testEndpoints } from './api.ts'
import type { ClientInfo } from './api.ts'
import { Icon } from './Icon.tsx'
import { chooseLanguage, messages } from './messages.ts'
import type { Language, Messages } from './messages.ts'
import { SpeedChart } from './SpeedChart.tsx'
import { TestMetrics } from './TestMetrics.tsx'
import { useSpeedTest } from './useSpeedTest.ts'

function ClientDetails({ client, copy, language }: { client?: ClientInfo; copy: Messages; language: Language }) {
  const country = client?.countryCode && /^[A-Z]{2}$/i.test(client.countryCode)
    ? new Intl.DisplayNames([language], { type: 'region' }).of(client.countryCode.toUpperCase())
    : undefined
  const location = [country, client?.networkName ?? client?.asn].filter(Boolean).join(' · ')
  return (
    <dl className="rate-details">
      <div><dt>{copy.clientIp}</dt><dd>{client?.ip ?? '—'}</dd></div>
      {location && (
        <div>
          <dt>{copy.ipLocation}</dt>
          <dd>{location} <a className="rate-attribution" href="https://ipinfo.io">IPinfo</a></dd>
        </div>
      )}
    </dl>
  )
}

export function App() {
  const endpoints = useMemo(() => testEndpoints(window.location.href), [])
  const [client, setClient] = useState<ClientInfo>()
  const { run, toggle } = useSpeedTest(endpoints)
  const language = chooseLanguage(navigator.languages)
  const copy = messages[language]
  const running = run.status === 'running'
  const status = run.status === 'running' ? copy.measuring[run.phase ?? 'latency']
    : run.status === 'error' ? copy.errors[run.error ?? 'network']
    : copy[run.status]
  const action = running ? copy.cancel : run.status === 'done' ? copy.restart : copy.start

  useEffect(() => {
    document.documentElement.lang = language
  }, [language])

  useEffect(() => {
    const controller = new AbortController()
    const signal = AbortSignal.any([controller.signal, AbortSignal.timeout(5000)])
    fetchClient(endpoints.client, signal).then(setClient).catch(() => {})
    return () => controller.abort()
  }, [endpoints])

  return (
    <main className="rate" data-state={run.status}>
      <div className="rate-body">
        <section className="rate-main" aria-label={copy.title}>
          <div className="rate-intro">
            <div className="rate-controls">
              <a className="rate-brand" href="https://github.com/mebtte/rate" aria-label="rate (GitHub)">
                <span className="rate-mark"><Icon name="activity" /></span>rate
              </a>
              <div className="rate-test-action">
                <button className="rate-action" type="button" onClick={toggle}>{action}</button>
                <ClientDetails client={client} copy={copy} language={language} />
                <p className={run.status === 'error' ? 'rate-status' : 'rate-announcement'} role="status">{status}</p>
              </div>
            </div>
          </div>

          <SpeedChart copy={copy} trace={run.trace} />
        </section>

        <TestMetrics copy={copy} phase={run.phase} running={running} progress={run.progress}
          results={run.results} trace={run.trace} />
      </div>
    </main>
  )
}
