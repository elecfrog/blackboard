// Agent connector data layer. Talks to the bb-server HTTP routes added in
// `000002` (`GET/POST/DELETE /api/agents/connectors`). Falls back to a clear
// "offline" signal when bb-server is unreachable so the Settings UI can grey
// itself out instead of looking broken.

export type AgentConnectorState =
  | 'synced'
  | 'drift'
  | 'missing'
  | 'unreachable'
  | 'source_missing'

export type AgentSourceState = 'present' | 'missing'

export type AgentConnectorType = 'agents_md' | 'rules' | 'mcp_server'

export interface AgentConnectorTarget {
  label: string
  target_template: string
  target_path: string
  source_path?: string
  connector_type: AgentConnectorType
  state: AgentConnectorState
  source_sha256_short?: string
  target_sha256_short?: string
  target_mtime?: string
  is_symlink: boolean
  error?: string
}

export interface AgentConnector {
  id: string
  display_name: string
  /** Backward-compatible primary target fields; prefer `targets` for display. */
  target_template: string
  target_path: string
  connector_type: AgentConnectorType
  state: AgentConnectorState
  source_sha256_short?: string
  target_sha256_short?: string
  target_mtime?: string
  is_symlink: boolean
  targets: AgentConnectorTarget[]
}

export interface AgentConnectorList {
  source_state: AgentSourceState
  source_path: string
  source_sha256_short?: string
  connectors: AgentConnector[]
}

export interface AgentConnectorListLoaded {
  data: AgentConnectorList | null
  /** 'rest' when the live backend answered, 'offline' when it was unreachable, 'error' for backend JSON errors. */
  source: 'rest' | 'offline' | 'error'
  error?: string
}

class ApiHttpError extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message)
  }
}

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, { cache: 'no-cache', ...init })
  if (!response.ok) {
    let message = `HTTP ${response.status} ${response.statusText} for ${url}`
    try {
      const payload = (await response.json()) as { error?: { message?: string } }
      if (payload.error?.message) message = payload.error.message
    } catch {
      // Use the HTTP status message when the server did not return the
      // standard bb-server JSON error shape.
    }
    throw new ApiHttpError(response.status, message)
  }
  return (await response.json()) as T
}

/// Load the live connector list. Returns `{ data: null, source: 'offline' }`
/// when bb-server is unreachable, so the UI can render its degraded state
/// without throwing.
export async function loadAgentConnectors(): Promise<AgentConnectorListLoaded> {
  try {
    const data = await fetchJson<AgentConnectorList>('/api/agents/connectors')
    return { data, source: 'rest' }
  } catch (err) {
    if (err instanceof ApiHttpError) {
      console.warn('[bb] agents/connectors REST error', err)
      return { data: null, source: 'error', error: err.message }
    }
    console.warn('[bb] agents/connectors REST unreachable', err)
    return {
      data: null,
      source: 'offline',
      error: err instanceof Error ? err.message : String(err),
    }
  }
}

/// Sync a single connector. Returns the updated connector entry.
export async function syncAgentConnector(id: string): Promise<AgentConnector> {
  const safeId = encodeURIComponent(id)
  return fetchJson<AgentConnector>(`/api/agents/connectors/${safeId}/sync`, {
    method: 'POST',
  })
}

/// Disconnect (delete the target file for) a single connector. Returns the
/// updated connector entry.
export async function disconnectAgentConnector(id: string): Promise<AgentConnector> {
  const safeId = encodeURIComponent(id)
  return fetchJson<AgentConnector>(`/api/agents/connectors/${safeId}`, {
    method: 'DELETE',
  })
}
