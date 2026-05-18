export type RuntimeConnectorStatus = 'connected' | 'missing' | 'check_failed'

export type PiShellPathSource = 'settings' | 'git_bash_default' | 'path' | 'missing'

export interface RuntimeConnector {
  id: string
  display_name: string
  command: string
  configured_command?: string
  fallback_command: string
  resolved_program: string
  resolved_args: string[]
  status: RuntimeConnectorStatus
  version?: string
  error?: string
  max_concurrency?: number
}

export interface PiRuntimeConnector {
  agent_dir?: string
  settings_path?: string
  settings_exists: boolean
  settings_error?: string
  configured_shell_path?: string
  effective_shell_path?: string
  shell_path_source: PiShellPathSource
  shell_path_exists: boolean
  recommended_shell_path?: string
  default_provider?: string
  default_model?: string
  default_thinking_level?: string
  workaround_required: boolean
}

export interface RuntimeConnectorList {
  config_path: string
  node_timeout_secs: number
  run_timeout_secs: number
  runtimes: RuntimeConnector[]
  pi: PiRuntimeConnector
}

export interface RuntimeConnectorPatch {
  commands?: Record<string, string>
  runtime_max_concurrency?: Record<string, number>
  node_timeout_secs?: number
  run_timeout_secs?: number
  pi?: {
    shell_path?: string
  }
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
      // Keep the HTTP status text if the backend did not return bb's error shape.
    }
    throw new ApiHttpError(response.status, message)
  }
  return (await response.json()) as T
}

export async function loadRuntimeConnectors(): Promise<RuntimeConnectorList> {
  return fetchJson<RuntimeConnectorList>('/api/agents/runtime-connectors')
}

export async function patchRuntimeConnectors(
  input: RuntimeConnectorPatch,
): Promise<RuntimeConnectorList> {
  return fetchJson<RuntimeConnectorList>('/api/agents/runtime-connectors', {
    method: 'PATCH',
    headers: {
      'content-type': 'application/json',
    },
    body: JSON.stringify(input),
  })
}
