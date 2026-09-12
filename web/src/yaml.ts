// Minimal JSON → YAML emitter for the "View YAML" row action.
//
// The bridge `get` op returns a kube object as JSON; the row action labels the
// view "YAML" (the Lens vocabulary), so this renders that object in YAML.

const SAFE = /^[A-Za-z0-9_./-]+$/
const RESERVED = /^(true|false|null|yes|no|on|off|~)$/i
const NUMERIC = /^-?\d/

function scalar(value: unknown): string {
  if (value === null || value === undefined) return 'null'
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') return Number.isFinite(value) ? String(value) : 'null'
  const text = String(value)
  if (text === '') return "''"
  if (SAFE.test(text) && !RESERVED.test(text) && !NUMERIC.test(text)) return text
  return JSON.stringify(text)
}

/** Render a JSON value as YAML text. */
export function toYaml(value: unknown): string {
  return emit(value, 0)
}

function emit(value: unknown, indent: number): string {
  const pad = ' '.repeat(indent)
  if (Array.isArray(value)) {
    if (value.length === 0) return `${pad}[]`
    return value
      .map((item) => {
        if (Array.isArray(item)) {
          return item.length === 0 ? `${pad}- []` : `${pad}-\n${emit(item, indent + 2)}`
        }
        if (item && typeof item === 'object') {
          return Object.keys(item).length === 0
            ? `${pad}- {}`
            : `${pad}-\n${emit(item, indent + 2)}`
        }
        return `${pad}- ${scalar(item)}`
      })
      .join('\n')
  }
  if (value && typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>)
    if (entries.length === 0) return `${pad}{}`
    return entries
      .map(([key, item]) => {
        if (Array.isArray(item)) {
          return item.length === 0 ? `${pad}${key}: []` : `${pad}${key}:\n${emit(item, indent + 2)}`
        }
        if (item && typeof item === 'object') {
          return Object.keys(item).length === 0
            ? `${pad}${key}: {}`
            : `${pad}${key}:\n${emit(item, indent + 2)}`
        }
        return `${pad}${key}: ${scalar(item)}`
      })
      .join('\n')
  }
  return `${pad}${scalar(value)}`
}
