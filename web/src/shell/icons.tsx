import type { ReactNode } from 'react'

interface IconDef {
  viewBox?: string
  content: ReactNode
}

const ICONS = {
  kite: {
    viewBox: '0 0 32 32',
    content: (
      <>
        <path d="M16 3l11.5 10L16 28 4.5 13 16 3z" fill="var(--brand)" stroke="none" />
        <path
          d="M12.2 15 16 13l6-4.8M12.9 20 16 13l6.3 5.2"
          fill="none"
          stroke="white"
          strokeWidth={1.8}
        />
        <path
          d="M16 28v4M11.5 29.2 16 33l4.5-3.8"
          fill="none"
          stroke="var(--brand)"
          strokeWidth={1.8}
        />
      </>
    ),
  },
  cluster: {
    content: (
      <>
        <circle cx="12" cy="5" r="2.6" />
        <circle cx="5" cy="18" r="2.6" />
        <circle cx="19" cy="18" r="2.6" />
        <path d="M12 7.6v4.2M12 11.8 6.6 15.2M12 11.8l5.4 3.4" />
      </>
    ),
  },
  node: {
    content: (
      <>
        <path d="m12 3 7 4v9l-7 4-7-4V7l7-4z" />
        <path d="m5 7.5 7 4 7-4M12 11.5V20" />
      </>
    ),
  },
  pods: {
    content: (
      <>
        <rect x="5" y="4" width="14" height="16" rx="2" />
        <path d="M9 8h6M9 12h6M9 16h4" />
      </>
    ),
  },
  deploy: {
    content: (
      <>
        <rect x="4" y="4" width="16" height="16" rx="2" />
        <path d="M8 9h8M12 9v6M9 13l3-3 3 3" />
      </>
    ),
  },
  services: {
    content: (
      <>
        <circle cx="6" cy="6" r="2" />
        <circle cx="18" cy="6" r="2" />
        <circle cx="12" cy="18" r="2" />
        <path d="M8 6h8M6 8v8M18 8v8M10 18h4" />
      </>
    ),
  },
  config: {
    content: (
      <>
        <path d="M5 6h14M5 12h14M5 18h14" />
        <circle cx="9" cy="6" r="2.4" />
        <circle cx="15" cy="12" r="2.4" />
        <circle cx="8" cy="18" r="2.4" />
      </>
    ),
  },
  storage: {
    content: (
      <>
        <ellipse cx="12" cy="5" rx="7" ry="2.6" />
        <path d="M5 5v7c0 1.4 3.1 2.6 7 2.6s7-1.2 7-2.6V5M5 12v7c0 1.4 3.1 2.6 7 2.6s7-1.2 7-2.6v-7" />
      </>
    ),
  },
  network: {
    content: (
      <>
        <circle cx="12" cy="12" r="8.5" />
        <path d="M3.5 12h17M12 3.5c2.3 2.2 3.5 5.1 3.5 8.5s-1.2 6.3-3.5 8.5c-2.3-2.2-3.5-5.1-3.5-8.5S9.7 5.7 12 3.5z" />
      </>
    ),
  },
  argo: {
    content: (
      <>
        <path d="M12 3 21 8v8l-9 5-9-5V8l9-5z" />
        <path d="M8 9h8M12 9v6M9.5 12l2.5-2.5 2.5 2.5" />
      </>
    ),
  },
  projects: {
    content: <path d="M4 6.5h6l2 2h8v9H4v-11z" />,
  },
  repo: {
    content: (
      <>
        <path d="M6 3v12" />
        <circle cx="6" cy="18" r="2.2" />
        <path d="M6 15c3.5 0 5-2.4 12-2.4" />
        <circle cx="18" cy="15.6" r="2.2" />
      </>
    ),
  },
  search: {
    content: (
      <>
        <circle cx="11" cy="11" r="6.5" />
        <path d="m16 16 4 4" />
      </>
    ),
  },
  refresh: {
    content: (
      <>
        <path d="M19 8a7.5 7.5 0 1 0 2 6" />
        <path d="M19 3v5h-5" />
      </>
    ),
  },
  menu: {
    content: <path d="M4 7h16M4 12h16M4 17h16" />,
  },
  chevron: {
    content: <path d="m9 6 6 6-6 6" />,
  },
  settings: {
    content: (
      <>
        <circle cx="12" cy="12" r="3" />
        <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1a1.7 1.7 0 0 0 1.9.3A1.7 1.7 0 0 0 10 3V2.8h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1z" />
      </>
    ),
  },
} satisfies Record<string, IconDef>

export type IconName = keyof typeof ICONS

export function Icon({
  name,
  size,
  className,
}: {
  name: IconName
  size?: number
  className?: string
}) {
  const def: IconDef = ICONS[name]
  return (
    <svg
      className={className ? `icon ${className}` : 'icon'}
      style={size ? { width: size, height: size } : undefined}
      viewBox={def.viewBox ?? '0 0 24 24'}
      aria-hidden="true"
    >
      {def.content}
    </svg>
  )
}
