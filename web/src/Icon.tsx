const paths = {
  activity: 'M2 12h4l3-7 4 14 3-7h6',
  download: 'M12 5v14m-7-7 7 7 7-7',
  upload: 'M12 19V5m-7 7 7-7 7 7',
  latency: 'M10 2h4m-2 12 3-3m4-3 1-1M12 6a8 8 0 1 0 0 16 8 8 0 0 0 0-16Z',
}

export function Icon({ name }: { name: keyof typeof paths }) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
      strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={paths[name]} />
    </svg>
  )
}
