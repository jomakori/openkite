import { useEffect, useState } from 'react'
import { resolveBridge } from '../bridge'
import { fixtureLogLines, parseLogLines, type LogLine } from '../logs'

export interface LogTarget {
  name: string
  namespace: string
}

/**
 * Stream a pod's logs through the bridge, falling back to fixture lines when
 * the host stream is absent (static/browser target) or errors.
 */
export function useLogLines(target: LogTarget | null, nonce: number): LogLine[] {
  const [lines, setLines] = useState<LogLine[]>([])

  useEffect(() => {
    if (!target) {
      setLines([])
      return
    }
    let cancelled = false
    void (async () => {
      try {
        const bridge = resolveBridge().bridge
        const result = await bridge.api.logs(target.name, target.namespace || 'default')
        const parsed = parseLogLines(result)
        if (!cancelled) setLines(parsed.length ? parsed : fixtureLogLines(target.name))
      } catch {
        if (!cancelled) setLines(fixtureLogLines(target.name))
      }
    })()
    return () => {
      cancelled = true
    }
  }, [target?.name, target?.namespace, nonce])

  return lines
}
