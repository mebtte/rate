export interface ClientInfo {
  ip: string
  countryCode?: string | null
  asn?: string | null
  networkName?: string | null
}

export function testEndpoints(page: string) {
  // The server redirects the mount root to a trailing slash before serving the page.
  const api = new URL('api/v1/', page)
  const download = new URL('download', api)
  download.searchParams.set('bytes', '8388608')

  return {
    client: new URL('client', api).href,
    ping: new URL('ping', api).href,
    download: download.href,
    upload: new URL('upload', api).href,
  }
}

export type TestEndpoints = ReturnType<typeof testEndpoints>

export async function fetchClient(url: string, signal: AbortSignal): Promise<ClientInfo> {
  const response = await fetch(url, { credentials: 'same-origin', cache: 'no-store', signal })
  if (!response.ok) throw new Error('Client lookup failed')
  return response.json()
}
