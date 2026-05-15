export type AgentSessionStatus = 'pending' | 'running' | 'idle' | 'failed' | 'cancelled'
export type AgentEventType =
  | 'status'
  | 'text'
  | 'thinking'
  | 'tool_use'
  | 'tool_result'
  | 'error'
  | 'log'
  | 'usage_update'

export interface TokenUsage {
  input_tokens: number
  output_tokens: number
  cache_read_tokens: number
  cache_write_tokens: number
}

export interface AgentSessionSummary {
  id: string
  status: AgentSessionStatus
  provider_session_id?: string
  event_count: number
  tool_count: number
  usage?: Record<string, TokenUsage>
  updated_at: string
}

export interface AgentSession extends AgentSessionSummary {
  project: string
  title?: string
  runtime: string
  agent: string
  model?: string
  variant?: string
  parent?: Record<string, unknown>
  created_at: string
  completed_at?: string
}

export interface AgentEvent {
  seq: number
  timestamp: string
  type: AgentEventType
  content?: string
  tool?: string
  call_id?: string
  input?: unknown
  output?: string
  status?: string
  level?: string
  session_id?: string
  usage?: Record<string, TokenUsage>
}

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url, { cache: 'no-cache' })
  if (!response.ok) {
    let message = `HTTP ${response.status} ${response.statusText} for ${url}`
    try {
      const payload = (await response.json()) as { error?: { message?: string } }
      if (payload.error?.message) message = payload.error.message
    } catch {
      // Keep the HTTP fallback.
    }
    throw new Error(message)
  }
  return (await response.json()) as T
}

async function writeJson<T>(url: string, method: string, body?: unknown): Promise<T> {
  const response = await fetch(url, {
    method,
    headers: body === undefined ? undefined : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  if (!response.ok) {
    let message = `HTTP ${response.status} ${response.statusText} for ${url}`
    try {
      const payload = (await response.json()) as { error?: { message?: string } }
      if (payload.error?.message) message = payload.error.message
    } catch {
      // Keep the HTTP fallback.
    }
    throw new Error(message)
  }
  return (await response.json()) as T
}

export async function createAgentSession(
  project: string,
  input: {
    prompt: string
    title?: string
    runtime?: string
    agent?: string
    model?: string
    variant?: string
    provider_session_id?: string
    timeout_secs?: number
  },
): Promise<AgentSession> {
  const payload = await writeJson<{ session: AgentSession }>(
    `/api/projects/${encodeURIComponent(project)}/agent-sessions`,
    'POST',
    input,
  )
  return payload.session
}

export async function readAgentSession(
  project: string,
  sessionId: string,
): Promise<AgentSession> {
  const payload = await fetchJson<{ session: AgentSession }>(
    `/api/projects/${encodeURIComponent(project)}/agent-sessions/${encodeURIComponent(sessionId)}`,
  )
  return payload.session
}

export async function readAgentSessionEvents(
  project: string,
  sessionId: string,
  since = 0,
): Promise<AgentEvent[]> {
  const payload = await fetchJson<{ events: AgentEvent[] }>(
    `/api/projects/${encodeURIComponent(project)}/agent-sessions/${encodeURIComponent(sessionId)}/events?since=${since}`,
  )
  return payload.events
}

export function watchAgentSessionEvents(
  project: string,
  sessionId: string,
  onEvents: (events: AgentEvent[]) => void,
  onError?: (message: string) => void,
  since = 0,
): () => void {
  if (typeof window === 'undefined' || typeof EventSource === 'undefined') return () => {}

  const source = new EventSource(
    `/api/projects/${encodeURIComponent(project)}/agent-sessions/${encodeURIComponent(sessionId)}/stream?since=${since}`,
  )
  let closed = false

  source.addEventListener('agent_session_events', (event) => {
    try {
      const payload = JSON.parse((event as MessageEvent).data) as { events?: AgentEvent[] }
      if (payload.events?.length) onEvents(payload.events)
    } catch (err) {
      onError?.(err instanceof Error ? err.message : String(err))
    }
  })

  source.addEventListener('agent_session_error', (event) => {
    try {
      const payload = JSON.parse((event as MessageEvent).data) as { error?: { message?: string } }
      onError?.(payload.error?.message ?? 'AgentSession stream failed.')
    } catch {
      onError?.('AgentSession stream failed.')
    }
  })

  source.onerror = () => {
    if (!closed) onError?.('AgentSession stream disconnected.')
  }

  return () => {
    closed = true
    source.close()
  }
}
