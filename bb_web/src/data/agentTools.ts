export type AgentToolStatus =
  | 'missing'
  | 'installed'
  | 'version_mismatch'
  | 'npm_missing'
  | 'check_failed'
  | 'external_install'

export interface AgentTool {
  id: string
  display_name: string
  cli_name: string
  npm_package: string
  target_version: string
  install_arg: string
  install_command: string
  status: AgentToolStatus
  cli_path?: string
  current_version?: string
  npm_version?: string
  last_error?: string
}

export interface AgentToolList {
  tools: AgentTool[]
}

export interface AgentToolInstallResult {
  tool: AgentTool
  command: string
  exit_code?: number
  stdout_tail?: string
  stderr_tail?: string
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
      // Keep the HTTP status message when the server returns non-JSON.
    }
    throw new ApiHttpError(response.status, message)
  }
  return (await response.json()) as T
}

export async function loadAgentTools(): Promise<AgentToolList> {
  return fetchJson<AgentToolList>('/api/agents/tools')
}

export async function installAgentTool(id: string): Promise<AgentToolInstallResult> {
  const safeId = encodeURIComponent(id)
  return fetchJson<AgentToolInstallResult>(`/api/agents/tools/${safeId}/install`, {
    method: 'POST',
  })
}
