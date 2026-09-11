export interface LogLine {
  time: string
  level: string
  method?: string
  message: string
}

const SAMPLE: LogLine[] = [
  { time: '10:42:07.114', level: 'INFO', method: 'GET', message: '/api/v1/orders 200 18.4ms' },
  { time: '10:42:07.182', level: 'INFO', method: 'POST', message: '/api/v1/checkout 201 42.1ms' },
  { time: '10:42:08.001', level: 'INFO', method: 'GET', message: '/healthz 200 1.2ms' },
  { time: '10:42:08.647', level: 'WARN', message: 'upstream 10.0.4.22:8080 slow response 312ms' },
  {
    time: '10:42:09.204',
    level: 'ERROR',
    method: 'GET',
    message: '/internal/queue depth exceeded threshold (queue=1024)',
  },
  { time: '10:42:09.531', level: 'INFO', method: 'GET', message: '/metrics 200 3.9ms' },
]

/** Fixture logs served when the host has no live stream (static/staging). */
export function fixtureLogLines(pod: string): LogLine[] {
  return SAMPLE.map((line) => ({
    ...line,
    message: `${line.message}  [${pod.slice(0, 24)}]`,
  }))
}

const LOG_PATTERN = /^(\d{2}:\d{2}:\d{2}\.\d{3})\s+(INFO|WARN|ERROR|DEBUG)\s+(.*)$/i
const METHOD_PATTERN = /^(GET|POST|PUT|PATCH|DELETE)\s+/

/**
 * Normalise a host `logs` reply. The bridge returns the raw log text (a
 * newline-joined string); fixture providers return structured lines directly.
 */
export function parseLogLines(result: unknown): LogLine[] {
  if (Array.isArray(result)) {
    return result
      .map((entry) => normaliseEntry(entry))
      .filter((line): line is LogLine => line !== null)
  }
  if (typeof result === 'string') {
    return result
      .split('\n')
      .map((line) => line.trimEnd())
      .filter(Boolean)
      .map(parseTextLine)
  }
  return []
}

function normaliseEntry(entry: unknown): LogLine | null {
  if (!entry || typeof entry !== 'object') return null
  const candidate = entry as Partial<LogLine>
  if (typeof candidate.message !== 'string') return null
  return {
    time: candidate.time ?? '',
    level: candidate.level ?? 'INFO',
    method: candidate.method,
    message: candidate.message,
  }
}

function parseTextLine(line: string): LogLine {
  const match = LOG_PATTERN.exec(line)
  if (!match) return { time: '', level: 'INFO', message: line }
  const [, time, level, rest] = match
  const methodMatch = METHOD_PATTERN.exec(rest)
  return {
    time,
    level: level.toUpperCase(),
    method: methodMatch ? methodMatch[1] : undefined,
    message: methodMatch ? rest.slice(methodMatch[0].length) : rest,
  }
}
