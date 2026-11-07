export const messages = {
  'zh-CN': {
    title: '网络测速',
    start: '开始测速',
    cancel: '取消测速',
    restart: '重新测速',
    idle: '',
    canceled: '测速已取消',
    done: '测速完成',
    measuring: { latency: '正在测量延迟', download: '正在测量下载', upload: '正在测量上传' },
    errors: {
      network: '连接失败, 请重试',
      timeout: '测速超时, 请重试',
      unauthorized: '访问授权失败, 请重新登录',
      response: '服务响应异常, 请重试',
    },
    liveRate: '实时速率',
    transferRate: { download: '下载速率', upload: '上传速率' },
    time: '时间',
    results: '测速结果',
    latency: '延迟',
    download: '下载',
    upload: '上传',
    jitter: '抖动',
    clientIp: '客户端 IP',
    ipLocation: 'IP 归属地',
  },
  en: {
    title: 'Network speed test',
    start: 'Start test',
    cancel: 'Cancel test',
    restart: 'Test again',
    idle: '',
    canceled: 'Test canceled',
    done: 'Test complete',
    measuring: { latency: 'Measuring latency', download: 'Measuring download', upload: 'Measuring upload' },
    errors: {
      network: 'Connection failed. Please try again.',
      timeout: 'Test timed out. Please try again.',
      unauthorized: 'Access denied. Please sign in again.',
      response: 'Unexpected server response. Please try again.',
    },
    liveRate: 'Live speed',
    transferRate: { download: 'Download speed', upload: 'Upload speed' },
    time: 'Time',
    results: 'Test results',
    latency: 'Latency',
    download: 'Download',
    upload: 'Upload',
    jitter: 'Jitter',
    clientIp: 'Client IP',
    ipLocation: 'IP location',
  },
}

export type Language = keyof typeof messages
export type Messages = typeof messages[Language]

export function chooseLanguage(preferred: readonly string[]): Language {
  for (const tag of preferred) {
    const primary = tag.toLowerCase().split('-')[0]
    if (primary === 'zh') return 'zh-CN'
    if (primary === 'en') return 'en'
  }
  return 'en'
}
