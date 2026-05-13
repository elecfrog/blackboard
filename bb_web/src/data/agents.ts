export type AgentRegistrySourceState = 'present' | 'missing'

export interface McpServerConfig {
  name: string
  transport: 'stdio' | 'sse'
  command?: string
  args?: string[]
  url?: string
  env?: Record<string, string>
}

export interface AgentProfile {
  id: string
  display_name: string
  kind: string
  runtime?: string
  scope: string
  status: string
  assignable: boolean
  distribute: boolean
  source_path?: string
  roles: string[]
  description?: string
  // ── Runtime binding (Ticket #000048) ──
  model?: string
  variant?: string
  instructions?: string
  instructions_path?: string
  // ── Execution config ──
  custom_env?: Record<string, string>
  custom_args?: string[]
  max_concurrent_tasks?: number
  // ── Org relations ──
  org_role?: 'coordinator' | 'worker'
  coordinator?: string
  workers?: string[]
  // ── MCP servers (Ticket #000049) ──
  mcp_servers?: McpServerConfig[]
  // ── Skills (Ticket #000050) ──
  skills?: string[]
}

export interface ProjectAgentProfile extends AgentProfile {
  origin: 'global' | 'project' | string
  project_role?: string
  project_lanes: string[]
}

export interface ProjectAgentList {
  source_state: AgentRegistrySourceState
  source_path: string
  project: string
  agents: ProjectAgentProfile[]
}

export interface AgentRegistryList {
  source_state: AgentRegistrySourceState
  source_path: string
  agents: AgentProfile[]
  project_agents: Array<{
    project: string
    agent: string
    role?: string
    lanes: string[]
  }>
}

export interface ProjectAgentRegistration {
  project: string
  agent: string
  role?: string
  lanes: string[]
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

export async function loadAgentRegistry(): Promise<AgentRegistryList> {
  return fetchJson<AgentRegistryList>('/api/agents')
}

export async function loadProjectAgents(project: string): Promise<ProjectAgentList> {
  return fetchJson<ProjectAgentList>(`/api/projects/${encodeURIComponent(project)}/agents`)
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

export async function upsertAgent(agent: AgentProfile): Promise<AgentProfile> {
  return writeJson<AgentProfile>('/api/agents', 'POST', agent)
}

export async function upsertProjectAgent(
  project: string,
  input: { agent: string; role?: string; lanes?: string[] },
): Promise<ProjectAgentRegistration> {
  return writeJson<ProjectAgentRegistration>(
    `/api/projects/${encodeURIComponent(project)}/agents`,
    'POST',
    { ...input, lanes: input.lanes ?? [] },
  )
}

export async function removeProjectAgent(
  project: string,
  agent: string,
): Promise<{ project: string; agent: string; removed: boolean }> {
  return writeJson<{ project: string; agent: string; removed: boolean }>(
    `/api/projects/${encodeURIComponent(project)}/agents/${encodeURIComponent(agent)}`,
    'DELETE',
  )
}
