export type TaskGraphScope = 'system' | 'project'
export type TaskGraphSource = 'builtin' | 'project'

export interface TaskGraphRef {
  scope: TaskGraphScope
  id: string
}

export interface TaskGraphOrigin extends TaskGraphRef {
  version: number
  forked_at: string
}

export interface TaskGraphLastRun {
  run_id: string
  status: string
  updated_at: string
}

export interface TaskGraphCatalogItem extends TaskGraphRef {
  title: string
  description?: string
  version: number
  readonly: boolean
  source: TaskGraphSource
  origin: TaskGraphOrigin | null
  node_count: number
  edge_count: number
  updated_at: string
  last_run: TaskGraphLastRun | null
  /** When the graph JSON failed to parse on the backend, this carries the error message. */
  compile_error?: string | null
}

// ─── Pin Types ───────────────────────────────────────────────────────────────

export type PinCategory = 'exec' | 'data'
export type PinDirection = 'in' | 'out'
export type PinValueType = 'string' | 'int' | 'float' | 'bool' | 'json' | 'array' | 'any' | 'markdown'

export interface NodePin {
  id: string
  label: string
  direction: PinDirection
  category: PinCategory
  value_type?: PinValueType
  required?: boolean
}

// ─── Node / Edge Types ───────────────────────────────────────────────────────

export interface TaskGraphNode {
  id: string
  type: 'start' | 'end' | 'llm' | 'human_gate' | 'branch' | 'loop' | 'input_var' | 'sub_graph'
  label: string
  description?: string
  position?: { x: number; y: number }
  config: Record<string, unknown>
  pins?: NodePin[]
}

export interface TaskGraphEdge {
  id: string
  from: string
  to: string
  kind: 'control' | 'exec' | 'data'
  label?: string
  from_pin?: string
  to_pin?: string
  source_handle?: string
  target_handle?: string
}

/**
 * 基础类型：string | number | boolean | json | ticket_ref
 * 泛型容器：array<T>（T 为任意基础类型）
 * 示例："string", "number", "ticket_ref", "array<ticket_ref>", "array<string>"
 */
export type InputParamType =
  | 'string'
  | 'number'
  | 'boolean'
  | 'json'
  | 'ticket_ref'
  | `array<${string}>`

export interface TaskGraphInputParam {
  id: string
  label?: string
  type: InputParamType
  default?: unknown
  description?: string
  min?: number
  max?: number
}

export interface TaskGraphDefinition extends TaskGraphRef {
  schema_version: 1
  title: string
  description?: string
  version: number
  readonly: boolean
  origin?: TaskGraphOrigin
  metadata?: {
    tags?: string[]
    related_tickets?: string[]
    owner?: string
    created_by?: string
    updated_by?: string
  }
  inputs?: TaskGraphInputParam[]
  nodes: TaskGraphNode[]
  edges: TaskGraphEdge[]
  layout?: {
    viewport?: { x: number; y: number; scale: number }
  }
}

export interface TaskGraphCatalogResult {
  graphs: TaskGraphCatalogItem[]
  source: 'rest' | 'mock'
}

export {
  PIN_EXEC_COLOR,
  PIN_VALUE_TYPE_COLORS,
  getDefaultPins,
  getNodePins,
  canConnect,
  NODE_HEADER_HEIGHT,
  PIN_ROW_HEIGHT,
  PIN_AREA_PADDING,
  NODE_MIN_HEIGHT,
  nodeToCanvasPins,
  nodeHeightForPins,
  type CanvasPinDescriptor,
} from './taskGraphPins'

export type TaskGraphRunStatus = 'pending' | 'running' | 'paused' | 'succeeded' | 'failed' | 'cancelled'
export type TaskGraphNodeRunStatus = 'idle' | 'queued' | 'running' | 'succeeded' | 'failed' | 'skipped' | 'paused'
export type TaskGraphArtifactContentType = 'markdown' | 'json' | 'text'

export interface TaskGraphRunSummary {
  id: string
  project: string
  graph?: TaskGraphRef & { version?: number }
  graph_ref?: TaskGraphRef & { version?: number }
  status: TaskGraphRunStatus
  created_at: string
  started_at?: string
  updated_at: string
  completed_at?: string
}

export interface TaskGraphPausedAction {
  id: string
  label: string
  result: string
}

export interface TaskGraphRunPaused {
  node_id: string
  reason: string
  actions: TaskGraphPausedAction[]
}

export interface TaskGraphNodeError {
  code: string
  message: string
}

export interface TaskGraphOutputArtifact {
  id: string
  path: string
  content_type: TaskGraphArtifactContentType
}

export interface TaskGraphRunNode {
  node_id: string
  status: TaskGraphNodeRunStatus
  started_at?: string
  completed_at?: string
  duration_ms?: number
  iteration?: number
  exit_code?: number
  error?: TaskGraphNodeError
  output_artifact?: TaskGraphOutputArtifact
  log_tail?: string
  child_run_id?: string
  runtime?: string
  agent?: string
  model?: string
  agent_session_id?: string
  agent_session?: import('./agentSessions').AgentSessionSummary
}

export interface TaskGraphBranchDecision {
  node_id: string
  selected_rule_id: string
  selected_edge_id: string
  evaluated_at: string
}

export interface TaskGraphLoopIterationEntry {
  iteration: number
  started_at: string
  completed_at?: string
  result: 'continued' | 'exited' | 'failed'
}

export interface TaskGraphLoopIteration {
  loop_node_id: string
  current_iteration: number
  max_iterations: number
  exit_reason?: string
  history: TaskGraphLoopIterationEntry[]
}

export interface TaskGraphRunContext {
  input: Record<string, unknown>
  node_outputs: Record<string, unknown>
  branch_decisions: TaskGraphBranchDecision[]
  loop_iterations: TaskGraphLoopIteration[]
}

export interface TaskGraphRunDetail {
  id: string
  project: string
  graph_ref: TaskGraphRef & { version: number }
  status: TaskGraphRunStatus
  created_at: string
  started_at?: string
  updated_at: string
  completed_at?: string
  paused?: TaskGraphRunPaused
  cursor: string[]
  context: TaskGraphRunContext
  graph_snapshot: TaskGraphDefinition
  nodes: TaskGraphRunNode[]
  parent_run_id?: string
}

export interface TaskGraphValidationError {
  target: 'graph' | 'node' | 'edge'
  id?: string
  message: string
}

export interface TaskGraphValidationResult {
  status: 'passed' | 'failed'
  errors: TaskGraphValidationError[]
}

const TASK_GRAPH_ENGINE_STORAGE_KEY = 'blackboard.task-graphs.engine'
const knownLlmRuntimes = ['codex', 'opencode', 'codebuddy']

function taskGraphEngineOverride(): 'csharp_v2' | null {
  if (typeof window === 'undefined') return null
  try {
    return window.localStorage.getItem(TASK_GRAPH_ENGINE_STORAGE_KEY) === 'csharp_v2' ? 'csharp_v2' : null
  } catch {
    return null
  }
}

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, {
    cache: 'no-cache',
    ...init,
    headers: {
      ...(init?.body ? { 'content-type': 'application/json' } : {}),
      ...(init?.headers ?? {}),
    },
  })
  if (!response.ok) throw new Error(`HTTP ${response.status} ${response.statusText} for ${url}`)
  return (await response.json()) as T
}

export function taskGraphDefaultInput(graph: TaskGraphDefinition, overrides: Record<string, unknown> = {}): Record<string, unknown> {
  const values: Record<string, unknown> = {}
  for (const input of graph.inputs ?? []) {
    values[input.id] = input.default ?? defaultValueForInputType(input.type)
  }
  return { ...values, ...overrides }
}

function defaultValueForInputType(type: InputParamType) {
  if (type === 'number') return 0
  if (type === 'boolean') return false
  if (type === 'json') return {}
  if (type.startsWith('array<')) return []
  return ''
}

function kebabId(value: string, fallback: string) {
  const id = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return /^[a-z][a-z0-9-]{1,63}$/.test(id) ? id : fallback
}

function uniqueProjectGraphId(_project: string, base: string) {
  // ID uniqueness is enforced by the backend (DuplicateGraphId error).
  // Append a short timestamp suffix to reduce collisions.
  return `${base}-${Date.now().toString(36)}`
}

function runGraphRef(run: TaskGraphRunSummary | TaskGraphRunDetail): TaskGraphRef & { version?: number } {
  if (run.graph_ref) return run.graph_ref
  if ('graph' in run && run.graph) return run.graph
  return { scope: 'project', id: 'unknown' }
}

function configString(config: Record<string, unknown>, key: string) {
  const value = config[key]
  return typeof value === 'string' ? value : ''
}

function configNumber(config: Record<string, unknown>, key: string) {
  const value = config[key]
  return typeof value === 'number' ? value : Number.NaN
}

function configRecord(config: Record<string, unknown>, key: string) {
  const value = config[key]
  return value && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}
}

function branchRules(node: TaskGraphNode) {
  const rules = node.config.rules
  if (!Array.isArray(rules)) return []
  return rules
    .map((rule) => {
      if (!rule || typeof rule !== 'object') return null
      const value = rule as Record<string, unknown>
      const id = typeof value.id === 'string' ? value.id : ''
      const label = typeof value.label === 'string' ? value.label : id
      return id ? { id, label } : null
    })
    .filter((rule): rule is { id: string; label: string } => rule !== null)
}

function cycleForEdge(nodes: Set<string>, edges: TaskGraphEdge[], edge: TaskGraphEdge) {
  if (!nodes.has(edge.from) || !nodes.has(edge.to)) return null
  const adjacency = new Map<string, string[]>()
  const edgeByPath = new Map<string, TaskGraphEdge>()
  for (const id of nodes) adjacency.set(id, [])
  for (const item of edges) {
    if (item.id === edge.id || !nodes.has(item.from) || !nodes.has(item.to)) continue
    adjacency.get(item.from)?.push(item.to)
    edgeByPath.set(`${item.from}->${item.to}`, item)
  }
  const stack: Array<{ id: string; path: TaskGraphEdge[] }> = [{ id: edge.to, path: [] }]
  const seen = new Set<string>()
  while (stack.length > 0) {
    const current = stack.pop()
    const id = current?.id
    if (!id || seen.has(id)) continue
    if (id === edge.from) return [...(current?.path ?? []), edge]
    seen.add(id)
    for (const next of adjacency.get(id) ?? []) {
      const nextEdge = edgeByPath.get(`${id}->${next}`)
      stack.push({ id: next, path: nextEdge ? [...(current?.path ?? []), nextEdge] : current?.path ?? [] })
    }
  }
  return null
}

export function validateTaskGraph(
  graph: TaskGraphDefinition,
  options?: { project?: string },
): TaskGraphValidationResult {
  const errors: TaskGraphValidationError[] = []
  const nodeIds = new Set(graph.nodes.map((node) => node.id))
  const startNodes = graph.nodes.filter((node) => node.type === 'start')
  const endNodes = graph.nodes.filter((node) => node.type === 'end')

  const isBlackboard = options?.project === 'blackboard'
  if (!isBlackboard && (graph.scope !== 'project' || graph.readonly)) {
    errors.push({ target: 'graph', message: 'Only project graphs can be saved.' })
  }
  if (startNodes.length !== 1) {
    errors.push({ target: 'graph', message: 'Graph must have exactly one start node.' })
  }
  if (endNodes.length < 1) {
    errors.push({ target: 'graph', message: 'Graph must have at least one end node.' })
  }

  for (const edge of graph.edges) {
    if (!nodeIds.has(edge.from)) errors.push({ target: 'edge', id: edge.id, message: `Edge ${edge.id} references missing source node ${edge.from}.` })
    if (!nodeIds.has(edge.to)) errors.push({ target: 'edge', id: edge.id, message: `Edge ${edge.id} references missing target node ${edge.to}.` })
  }

  for (const node of graph.nodes) {
    const incoming = graph.edges.filter((edge) => edge.to === node.id)
    const outgoing = graph.edges.filter((edge) => edge.from === node.id)

    if (node.type === 'start' && incoming.length > 0) {
      errors.push({ target: 'node', id: node.id, message: 'Start node cannot have incoming edges.' })
    }
    if (node.type === 'end' && outgoing.length > 0) {
      errors.push({ target: 'node', id: node.id, message: 'End node cannot have outgoing edges.' })
    }
    if (node.type === 'llm') {
      const runAs = configString(node.config, 'run_as') === 'agent' ? 'agent' : 'llm'
      if (runAs === 'agent') {
        if (!configString(node.config, 'agent_profile')) {
          errors.push({ target: 'node', id: node.id, message: 'Agent mode must select an agent profile.' })
        }
      } else {
        const runtime = configString(node.config, 'runtime')
        if (!knownLlmRuntimes.includes(runtime)) {
          errors.push({ target: 'node', id: node.id, message: `LLM runtime must be one of ${knownLlmRuntimes.join(', ')}.` })
        }
      }
    }
    if (node.type === 'branch') {
      const rules = branchRules(node)
      const defaultRuleId = configString(node.config, 'default_rule_id')
      if (rules.length === 0) {
        errors.push({ target: 'node', id: node.id, message: 'Branch must have at least one rule.' })
      }
      if (!rules.some((rule) => rule.id === defaultRuleId)) {
        errors.push({ target: 'node', id: node.id, message: 'Branch default_rule_id must match a rule.' })
      }
      for (const rule of rules) {
        const matchedEdges = outgoing.filter((edge) => (edge.from_pin ?? edge.source_handle) === `rule:${rule.id}`)
        if (matchedEdges.length !== 1) {
          errors.push({ target: 'node', id: node.id, message: `Branch rule ${rule.id} must have exactly one outgoing edge.` })
        }
      }
    }
    if (node.type === 'loop') {
      const maxIterations = configNumber(node.config, 'max_iterations')
      if (!Number.isFinite(maxIterations) || maxIterations < 1) {
        errors.push({ target: 'node', id: node.id, message: 'Loop must have max_iterations >= 1.' })
      }
      if (!outgoing.some((edge) => (edge.from_pin ?? edge.source_handle) === 'body')) {
        errors.push({ target: 'node', id: node.id, message: 'Loop must have a body edge.' })
      }
      if (!outgoing.some((edge) => (edge.from_pin ?? edge.source_handle) === 'exit')) {
        errors.push({ target: 'node', id: node.id, message: 'Loop must have an exit edge.' })
      }
    }
    if (node.type === 'sub_graph') {
      const bindings = configRecord(node.config, 'input_bindings')
      for (const [key, value] of Object.entries(bindings)) {
        if (typeof value === 'string' && value.trim() === '') {
          errors.push({ target: 'node', id: node.id, message: `Sub-graph input binding ${key} cannot be empty.` })
        }
      }
    }
  }

  for (const edge of graph.edges) {
    const cycle = cycleForEdge(nodeIds, graph.edges, edge)
    if (!cycle) continue
    const hasLoopController = cycle.some((item) => {
      const target = graph.nodes.find((node) => node.id === item.to)
      return target?.type === 'loop'
    })
    if (!hasLoopController) {
      errors.push({ target: 'edge', id: edge.id, message: `Cycle edge ${edge.id} must pass through a loop node.` })
    }
  }

  return { status: errors.length === 0 ? 'passed' : 'failed', errors }
}

export async function loadTaskGraphCatalog(project: string): Promise<TaskGraphCatalogResult> {
  const encoded = encodeURIComponent(project)
  const payload = await fetchJson<{ graphs: TaskGraphCatalogItem[] }>(`/api/projects/${encoded}/task-graphs`)
  return { graphs: payload.graphs ?? [], source: 'rest' }
}

export async function readTaskGraph(project: string, ref: TaskGraphRef): Promise<{ graph: TaskGraphDefinition; source: 'rest' | 'mock' }> {
  const encoded = encodeURIComponent(project)
  const payload = await fetchJson<{ graph: TaskGraphDefinition }>(
    `/api/projects/${encoded}/task-graphs/${encodeURIComponent(ref.scope)}/${encodeURIComponent(ref.id)}`,
  )
  return { graph: payload.graph, source: 'rest' }
}

export async function createProjectTaskGraph(project: string, title: string): Promise<{ graph: TaskGraphDefinition; source: 'rest' | 'mock' }> {
  const trimmedTitle = title.trim() || 'New Task Graph'
  const id = uniqueProjectGraphId(project, kebabId(trimmedTitle, 'new-task-graph'))
  const graph: TaskGraphDefinition = {
    schema_version: 1,
    id,
    scope: 'project',
    title: trimmedTitle,
    description: '项目内自定义 Task Graph。',
    version: 1,
    readonly: false,
    metadata: { created_by: 'web' },
    nodes: [
      { id: 'start', type: 'start', label: 'Start', position: { x: 80, y: 120 }, config: {} },
      { id: 'end-success', type: 'end', label: 'Success', position: { x: 360, y: 120 }, config: { result: 'succeeded' } },
    ],
    edges: [
      { id: 'start__end-success', from: 'start', to: 'end-success', kind: 'control' },
    ],
  }

  const encoded = encodeURIComponent(project)
  const payload = await fetchJson<{ graph: TaskGraphDefinition }>(`/api/projects/${encoded}/task-graphs`, {
    method: 'POST',
    body: JSON.stringify({ graph }),
  })
  return { graph: payload.graph, source: 'rest' }
}

export async function forkSystemTaskGraph(
  project: string,
  systemId: string,
  title?: string,
): Promise<{ graph: TaskGraphDefinition; source: 'rest' | 'mock' }> {
  const targetTitle = title?.trim() || `${systemId}（项目自定义）`
  const targetId = uniqueProjectGraphId(project, kebabId(`${systemId}-custom`, 'custom-task-graph'))
  const encoded = encodeURIComponent(project)

  const payload = await fetchJson<{ graph: TaskGraphDefinition }>(
    `/api/projects/${encoded}/task-graphs/system/${encodeURIComponent(systemId)}/fork`,
    {
      method: 'POST',
      body: JSON.stringify({ target_id: targetId, title: targetTitle }),
    },
  )
  return { graph: payload.graph, source: 'rest' }
}

export async function saveProjectTaskGraph(
  project: string,
  graph: TaskGraphDefinition,
  expectedVersion = graph.version,
): Promise<{ graph: TaskGraphDefinition; validation: TaskGraphValidationResult; source: 'rest' | 'mock' }> {
  const encoded = encodeURIComponent(project)
  const scopeSegment = graph.scope === 'system' ? 'system' : 'project'
  try {
    const payload = await fetchJson<{ graph: TaskGraphDefinition; validation: TaskGraphValidationResult }>(
      `/api/projects/${encoded}/task-graphs/${scopeSegment}/${encodeURIComponent(graph.id)}`,
      {
        method: 'PATCH',
        body: JSON.stringify({ graph, expected_version: expectedVersion }),
      },
    )
    return { graph: payload.graph, validation: payload.validation, source: 'rest' }
  } catch {
    const next = {
      ...graph,
      version: graph.version + 1,
      readonly: false,
      scope: graph.scope,
    }
    return {
      graph: next,
      validation: validateTaskGraph(next, { project }),
      source: 'mock',
    }
  }
}

export async function listTaskGraphRuns(project: string): Promise<{ runs: TaskGraphRunSummary[]; source: 'rest' | 'mock' }> {
  const encoded = encodeURIComponent(project)
  const payload = await fetchJson<{ runs: TaskGraphRunSummary[] }>(`/api/projects/${encoded}/task-graph-runs`)
  return {
    source: 'rest',
    runs: payload.runs.map((run) => ({
      ...run,
      graph: runGraphRef(run),
      graph_ref: runGraphRef(run) as TaskGraphRef & { version: number },
    })),
  }
}

export async function startTaskGraphRun(
  project: string,
  ref: TaskGraphRef,
  input: Record<string, unknown> = {},
): Promise<{ run: TaskGraphRunSummary; source: 'rest' | 'mock' }> {
  const encoded = encodeURIComponent(project)
  const engine = taskGraphEngineOverride()
  const payload = await fetchJson<{ run: TaskGraphRunSummary }>(`/api/projects/${encoded}/task-graph-runs`, {
    method: 'POST',
    body: JSON.stringify({ graph: ref, input, dry_run: false, ...(engine ? { engine } : {}) }),
  })
  return { run: { ...payload.run, graph: runGraphRef(payload.run), graph_ref: runGraphRef(payload.run) as TaskGraphRef & { version: number } }, source: 'rest' }
}

export async function cancelTaskGraphRun(project: string, runId: string): Promise<void> {
  const encoded = encodeURIComponent(project)
  await fetchJson(`/api/projects/${encoded}/task-graph-runs/${encodeURIComponent(runId)}/cancel`, {
    method: 'POST',
  })
}

export async function readTaskGraphRun(project: string, runId: string): Promise<{ run: TaskGraphRunDetail; source: 'rest' | 'mock' }> {
  const encoded = encodeURIComponent(project)
  const payload = await fetchJson<{ run: TaskGraphRunDetail }>(
    `/api/projects/${encoded}/task-graph-runs/${encodeURIComponent(runId)}`,
  )
  return { run: payload.run, source: 'rest' }
}

export function watchTaskGraphRun(
  project: string,
  runId: string,
  onRun: (run: TaskGraphRunDetail) => void,
  onError?: (message: string) => void,
): () => void {
  if (typeof window === 'undefined' || typeof EventSource === 'undefined') return () => {}

  const encodedProject = encodeURIComponent(project)
  const encodedRun = encodeURIComponent(runId)
  const source = new EventSource(`/api/projects/${encodedProject}/task-graph-runs/${encodedRun}/events`)
  let closed = false

  source.addEventListener('run', (event) => {
    try {
      const payload = JSON.parse((event as MessageEvent).data) as { run?: TaskGraphRunDetail }
      if (!payload.run) return
      onRun(payload.run)
      if (['succeeded', 'failed', 'cancelled'].includes(payload.run.status)) {
        closed = true
        source.close()
      }
    } catch (err) {
      onError?.(err instanceof Error ? err.message : String(err))
    }
  })

  source.addEventListener('run_error', (event) => {
    try {
      const payload = JSON.parse((event as MessageEvent).data) as { error?: { message?: string } }
      onError?.(payload.error?.message ?? 'Task Graph run stream failed.')
    } catch {
      onError?.('Task Graph run stream failed.')
    }
  })

  source.onerror = () => {
    if (!closed) onError?.('Task Graph run stream disconnected.')
  }

  return () => {
    closed = true
    source.close()
  }
}
