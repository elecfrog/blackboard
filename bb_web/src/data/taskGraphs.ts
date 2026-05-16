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
export type PinValueType =
  | 'string'
  | 'text'
  | 'int'
  | 'float'
  | 'bool'
  | 'json'
  | 'array'
  | 'any'
  | 'markdown'
  | 'file_ref'
  | 'wiki_ref'
  | 'ticket_ref'
  | 'diff'
  | 'test_result'
  | 'review_comment'
  | 'handoff_summary'
  | 'runtime_log'
  | 'artifact_ref'

export interface NodePin {
  id: string
  label: string
  direction: PinDirection
  category: PinCategory
  value_type?: PinValueType
  required?: boolean
}

// ─── Dataflow Channel Types ─────────────────────────────────────────────────

export type TaskGraphChannelValueType = PinValueType
export type TaskGraphChannelKind = 'last_value' | 'topic' | 'aggregate' | 'barrier' | 'artifact_ref'

export interface TaskGraphChannelSpec {
  name: string
  kind: TaskGraphChannelKind
  value_type: TaskGraphChannelValueType
  reducer?: string
  barrier_nodes?: string[]
  description?: string
}

export interface TaskGraphChannelState {
  name: string
  kind: TaskGraphChannelKind
  value_type: TaskGraphChannelValueType
  version: number
  value: unknown
  updated_by_node_id?: string
  updated_at?: string
}

export interface TaskGraphChannelWrite {
  channel: string
  source_node_id: string
  value: unknown
}

// ─── Node Taxonomy Types ────────────────────────────────────────────────────

export type TaskGraphNodeCategory = 'terminal' | 'control' | 'runtime' | 'transform' | 'artifact' | 'integration' | 'approval'
export type TaskGraphNodeRole =
  | 'explorer_agent'
  | 'implementer_agent'
  | 'verifier_agent'
  | 'reviewer_agent'
  | 'handoff_writer'
  | 'opencode_session'
  | 'codex_session'
  | 'claude_session'
  | 'local_shell'
  | 'write_wiki_doc'
  | 'update_ticket'
  | 'feishu_notify'

export type TaskGraphPermissionKind =
  | 'read_project'
  | 'read_worktree'
  | 'write_scoped'
  | 'run_tests'
  | 'network'
  | 'git_operation'
  | 'external_notify'
  | 'update_ticket'
  | 'write_wiki'

export type TaskGraphRuntimeBindingKind = 'none' | 'llm' | 'agent_session' | 'shell' | 'sub_graph' | 'blackboard' | 'webhook'
export type TaskGraphSessionResumePolicy = 'none' | 'reuse_by_run' | 'reuse_by_node' | 'fork_from_previous'

export interface TaskGraphRuntimeBinding {
  kind: TaskGraphRuntimeBindingKind
  provider?: string
  profile?: string
  model?: string
  variant?: string
  session_resume_policy: TaskGraphSessionResumePolicy
}

export interface TaskGraphPermissionSpec {
  kind: TaskGraphPermissionKind
  required?: boolean
  scope?: string
}

export interface TaskGraphArtifactOutputSpec {
  name: string
  value_type: TaskGraphChannelValueType
  channel_kind: TaskGraphChannelKind
}

// ─── Node / Edge Types ───────────────────────────────────────────────────────

export interface TaskGraphNode {
  id: string
  type:
    | 'start'
    | 'end'
    | 'llm'
    | 'plan'
    | 'shell'
    | 'human_gate'
    | 'branch'
    | 'loop'
    | 'input_var'
    | 'sub_graph'
    | 'intent_extract'
    | 'kb_plan'
    | 'manifest_merge'
    | 'llm_mutation'
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

export type GraphMutationOp =
  | { type: 'add_node'; node: TaskGraphNode }
  | { type: 'remove_node'; node_id: string }
  | { type: 'add_edge'; edge: TaskGraphEdge }
  | { type: 'remove_edge'; edge_id: string }
  | { type: 'patch_node_config'; node_id: string; patch: unknown }

export interface GraphMutationRequest {
  id: string
  source_task_id: string
  source_node_id: string
  op: GraphMutationOp
  reason?: string
}

export interface GraphMutationSummary {
  added_nodes: string[]
  removed_nodes: string[]
  added_edges: string[]
  removed_edges: string[]
  patched_nodes: string[]
}

export interface GraphMutationConflict {
  code: string
  message: string
  request_ids: string[]
  node_id?: string
  edge_id?: string
}

export type GraphMutationBatchResult =
  | {
      status: 'applied'
      new_revision: number
      summary: GraphMutationSummary
    }
  | {
      status: 'rejected'
      conflicts: GraphMutationConflict[]
    }

export interface GraphMutationBatch {
  id: string
  superstep: number
  base_revision: number
  requests: GraphMutationRequest[]
  result: GraphMutationBatchResult
}

export interface TopologyMutationEventPayload {
  batch_id: string
  graph_revision_before: number
  graph_revision_after: number
  result: GraphMutationBatchResult
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

export interface TaskGraphRunPolicy {
  allow_concurrent_runs: boolean
  max_concurrent_runs: number
  queue_enabled: boolean
  max_queue_wait_ms: number
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
    run_policy?: TaskGraphRunPolicy
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

export interface TaskGraphRunResolverOptions {
  runtime?: string
  agent_profile?: string
  model?: string
  variant?: string
}

export interface TaskGraphRunLaunchOptions {
  intent?: string
  resolver?: TaskGraphRunResolverOptions
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

export type TaskGraphRunStatus = 'queued' | 'pending' | 'running' | 'paused' | 'succeeded' | 'failed' | 'cancelled'
export type TaskGraphNodeRunStatus = 'idle' | 'queued' | 'running' | 'succeeded' | 'failed' | 'skipped' | 'paused'
export type TaskGraphArtifactContentType = 'markdown' | 'json' | 'text'

export interface TaskGraphRunSummary {
  id: string
  project: string
  graph?: TaskGraphRef & { version?: number }
  graph_ref?: TaskGraphRef & { version?: number }
  status: TaskGraphRunStatus
  created_at: string
  queued_at?: string
  queue_deadline_at?: string
  started_at?: string
  updated_at: string
  completed_at?: string
  current_superstep?: number
  last_checkpoint_id?: string
  current_graph_revision?: number
  active_nodes?: string[]
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
  queued_at?: string
  queue_deadline_at?: string
  started_at?: string
  updated_at: string
  completed_at?: string
  current_superstep?: number
  last_checkpoint_id?: string
  current_graph_revision: number
  active_nodes: string[]
  paused?: TaskGraphRunPaused
  context: TaskGraphRunContext
  graph_snapshot: TaskGraphDefinition
  nodes: TaskGraphRunNode[]
  parent_run_id?: string
  /** Legacy input only. UI must not use this directly. */
  cursor?: string[]
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

export type TaskGraphSuperstepStatus = 'running' | 'succeeded' | 'paused' | 'failed' | 'cancelled'

export interface TaskGraphPendingWrite {
  source_node_id: string
  target: string
  value: unknown
}

export interface TaskGraphPendingTaskEffect {
  task_id: string
  source_node_id: string
  normal_writes: TaskGraphChannelWrite[]
  graph_mutations: GraphMutationRequest[]
}

export interface TaskGraphSuperstepCheckpoint {
  id: string
  run_id: string
  superstep: number
  status: TaskGraphSuperstepStatus
  created_at: string
  completed_at?: string
  graph_revision_before: number
  graph_revision_after: number
  mutation_batch_id?: string
  ready_nodes: string[]
  waiting_nodes: string[]
  node_statuses: Record<string, TaskGraphNodeRunStatus>
  context: TaskGraphRunContext
  pending_effects?: TaskGraphPendingTaskEffect[]
  pending_writes?: TaskGraphPendingWrite[]
  message?: string
  /** Legacy input only. */
  cursor_before?: string[]
  /** Legacy input only. */
  cursor_after?: string[]
}

export interface TaskGraphRunEvent {
  id: string
  seq: number
  run_id: string
  superstep: number
  kind: string
  node_id?: string
  message: string
  payload?: unknown
  created_at: string
}

export type TaskGraphScheduleKind = 'interval' | 'daily' | 'weekly' | 'cron'

export interface TaskGraphScheduleSpec {
  kind: TaskGraphScheduleKind
  expression: string
}

export interface TaskGraphScheduleState {
  next_run_at?: string
  last_run_at?: string
  last_run_id?: string
  last_status?: string
  last_error?: string
  last_planned_fire_at?: string
}

export interface TaskGraphSchedule {
  id: string
  name: string
  project: string
  enabled: boolean
  graph_ref: TaskGraphRef
  input: unknown
  schedule: TaskGraphScheduleSpec
  timezone: string
  concurrency_policy: 'skip'
  misfire_policy: 'run_once'
  created_at: string
  updated_at: string
  state: TaskGraphScheduleState
}

export interface TaskGraphScheduleCreateInput {
  id?: string
  name: string
  graph_ref: TaskGraphRef
  input?: unknown
  schedule: TaskGraphScheduleSpec
  timezone?: string
  enabled?: boolean
}

export type TaskGraphSchedulePatchInput = Partial<{
  name: string
  enabled: boolean
  graph_ref: TaskGraphRef
  input: unknown
  schedule: TaskGraphScheduleSpec
  timezone: string
}>

const TASK_GRAPH_ENGINE_STORAGE_KEY = 'blackboard.task-graphs.engine'
const knownLlmRuntimes = ['codex', 'opencode', 'codebuddy']
const MOCK_PREGEL_MUTATION_GRAPH_ID = 'mock-pregel-mutation'
const MOCK_PREGEL_MUTATION_RUN_ID = 'mock-pregel-mutation-run'
const MOCK_PREGEL_MUTATION_CONFLICT_RUN_ID = 'mock-pregel-mutation-conflict-run'

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

function uniqueIds(values: string[]) {
  return [...new Set(values.filter(Boolean))]
}

function activeNodesFromRunNodes(nodes: TaskGraphRunNode[]) {
  return nodes
    .filter((node) => node.status === 'running' || node.status === 'queued')
    .map((node) => node.node_id)
}

export function normalizeTaskGraphRunSummary(raw: TaskGraphRunSummary): TaskGraphRunSummary {
  const graph_ref = runGraphRef(raw) as TaskGraphRef & { version?: number }
  return {
    ...raw,
    graph: graph_ref,
    graph_ref,
    current_graph_revision: raw.current_graph_revision ?? (raw as TaskGraphRunSummary & { graph_revision?: number }).graph_revision,
    active_nodes: uniqueIds(raw.active_nodes ?? []),
  }
}

export function normalizeTaskGraphRunDetail(raw: TaskGraphRunDetail): TaskGraphRunDetail {
  const runningNodes = activeNodesFromRunNodes(raw.nodes ?? [])
  const graph_ref = runGraphRef(raw) as TaskGraphRef & { version: number }
  return {
    ...raw,
    graph_ref,
    current_graph_revision: raw.current_graph_revision ?? (raw as TaskGraphRunDetail & { graph_revision?: number }).graph_revision ?? 0,
    active_nodes: uniqueIds(raw.active_nodes ?? raw.cursor ?? runningNodes),
    context: {
      input: raw.context?.input ?? {},
      node_outputs: raw.context?.node_outputs ?? {},
      branch_decisions: raw.context?.branch_decisions ?? [],
      loop_iterations: raw.context?.loop_iterations ?? [],
    },
    nodes: raw.nodes ?? [],
  }
}

function normalizeTaskGraphCheckpoint(raw: TaskGraphSuperstepCheckpoint): TaskGraphSuperstepCheckpoint {
  const before = raw.graph_revision_before ?? 0
  const after = raw.graph_revision_after ?? before
  return {
    ...raw,
    graph_revision_before: before,
    graph_revision_after: after,
    ready_nodes: raw.ready_nodes ?? raw.cursor_after ?? [],
    waiting_nodes: raw.waiting_nodes ?? [],
    node_statuses: raw.node_statuses ?? {},
  }
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
    if (node.type === 'shell') {
      const command = configString(node.config, 'command')
      if (!command.trim()) {
        errors.push({ target: 'node', id: node.id, message: 'Shell command is required.' })
      }
      if (/\s/.test(command)) {
        errors.push({ target: 'node', id: node.id, message: 'Shell command must be an executable name/path; put parameters in args.' })
      }
      if (/[|;<>`]/.test(command) || command.includes('&&') || command.includes('||') || command.includes('$(')) {
        errors.push({ target: 'node', id: node.id, message: 'Shell command cannot contain shell metacharacters.' })
      }
      const args = node.config.args
      if (Array.isArray(args) && args.some((arg) => typeof arg === 'string' && (/[|;<>`]/.test(arg) || arg.includes('&&') || arg.includes('||') || arg.includes('$(')))) {
        errors.push({ target: 'node', id: node.id, message: 'Shell args cannot contain shell metacharacters.' })
      }
      const timeout = configNumber(node.config, 'timeout_ms')
      if (Number.isFinite(timeout) && timeout <= 0) {
        errors.push({ target: 'node', id: node.id, message: 'Shell timeout_ms must be greater than 0.' })
      }
      const expectedExitCodes = node.config.expected_exit_codes
      if (Array.isArray(expectedExitCodes) && expectedExitCodes.length === 0) {
        errors.push({ target: 'node', id: node.id, message: 'Shell expected_exit_codes must contain at least one code.' })
      }
      const capture = configRecord(node.config, 'capture')
      const maxBytes = typeof capture.max_bytes === 'number' ? capture.max_bytes : Number.NaN
      if (Number.isFinite(maxBytes) && maxBytes <= 0) {
        errors.push({ target: 'node', id: node.id, message: 'Shell capture.max_bytes must be greater than 0.' })
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

function shouldExposeTaskGraphMocks(project: string) {
  return project === 'blackboard'
}

function mockPregelMutationGraph(revision = 0): TaskGraphDefinition {
  const reviewNode: TaskGraphNode = {
    id: 'review',
    type: 'human_gate',
    label: 'Review',
    description: 'Runtime review node added by a topology mutation.',
    position: { x: 640, y: 120 },
    config: {
      actions: [
        { id: 'approve', label: 'Approve', result: 'resume' },
        { id: 'reject', label: 'Reject', result: 'reject' },
      ],
    },
  }
  const nodes: TaskGraphNode[] = [
    { id: 'start', type: 'start', label: 'Start', position: { x: 80, y: 120 }, config: {} },
    {
      id: 'planner',
      type: 'llm',
      label: 'Planner',
      description: 'Produces runtime graph mutation requests.',
      position: { x: 360, y: 120 },
      config: { runtime: 'codex', prompt: 'Plan next review step.' },
    },
    ...(revision > 0 ? [reviewNode] : []),
    { id: 'end', type: 'end', label: 'End', position: { x: revision > 0 ? 920 : 640, y: 120 }, config: { result: 'succeeded' } },
  ]
  const edges: TaskGraphEdge[] = revision > 0
    ? [
        { id: 'start__planner', from: 'start', to: 'planner', kind: 'control' },
        { id: 'planner__review', from: 'planner', to: 'review', kind: 'control' },
        { id: 'review__end', from: 'review', to: 'end', kind: 'control' },
      ]
    : [
        { id: 'start__planner', from: 'start', to: 'planner', kind: 'control' },
        { id: 'planner__end', from: 'planner', to: 'end', kind: 'control' },
      ]

  return {
    schema_version: 1,
    scope: 'project',
    id: MOCK_PREGEL_MUTATION_GRAPH_ID,
    title: 'Pregel topology mutation fixture',
    description: 'Frontend fixture for active_nodes, graph revision, and topology mutation timeline.',
    version: 1,
    readonly: true,
    metadata: { tags: ['fixture', 'pregel', 'topology-mutation'] },
    nodes,
    edges,
  }
}

function mockPregelMutationCatalogItem(): TaskGraphCatalogItem {
  return {
    scope: 'project',
    id: MOCK_PREGEL_MUTATION_GRAPH_ID,
    title: 'Pregel topology mutation fixture',
    description: 'Mock run fixture for graph revision and topology mutation UI.',
    version: 1,
    readonly: true,
    source: 'project',
    origin: null,
    node_count: 3,
    edge_count: 2,
    updated_at: '2026-05-16T00:00:00.000Z',
    last_run: {
      run_id: MOCK_PREGEL_MUTATION_RUN_ID,
      status: 'paused',
      updated_at: '2026-05-16T00:04:00.000Z',
    },
  }
}

function mockMutationSummary(): GraphMutationSummary {
  return {
    added_nodes: ['review'],
    removed_nodes: [],
    added_edges: ['planner__review', 'review__end'],
    removed_edges: ['planner__end'],
    patched_nodes: [],
  }
}

function mockTaskGraphRunSummaries(project: string): TaskGraphRunSummary[] {
  const graph_ref = { scope: 'project', id: MOCK_PREGEL_MUTATION_GRAPH_ID, version: 1 } as TaskGraphRef & { version: number }
  return [
    normalizeTaskGraphRunSummary({
      id: MOCK_PREGEL_MUTATION_RUN_ID,
      project,
      graph_ref,
      graph: graph_ref,
      status: 'paused',
      created_at: '2026-05-16T00:00:00.000Z',
      started_at: '2026-05-16T00:00:05.000Z',
      updated_at: '2026-05-16T00:04:00.000Z',
      current_superstep: 2,
      last_checkpoint_id: 'mock-checkpoint-mutation-1',
      current_graph_revision: 1,
      active_nodes: ['review'],
    }),
    normalizeTaskGraphRunSummary({
      id: MOCK_PREGEL_MUTATION_CONFLICT_RUN_ID,
      project,
      graph_ref,
      graph: graph_ref,
      status: 'failed',
      created_at: '2026-05-16T00:10:00.000Z',
      started_at: '2026-05-16T00:10:04.000Z',
      updated_at: '2026-05-16T00:11:30.000Z',
      completed_at: '2026-05-16T00:11:30.000Z',
      current_superstep: 1,
      last_checkpoint_id: 'mock-checkpoint-mutation-conflict',
      current_graph_revision: 0,
      active_nodes: [],
    }),
  ]
}

function appendMockTaskGraphCatalog(project: string, graphs: TaskGraphCatalogItem[]) {
  if (!shouldExposeTaskGraphMocks(project) || graphs.some((graph) => graph.id === MOCK_PREGEL_MUTATION_GRAPH_ID)) return graphs
  return [...graphs, mockPregelMutationCatalogItem()]
}

function appendMockTaskGraphRuns(project: string, runs: TaskGraphRunSummary[]) {
  if (!shouldExposeTaskGraphMocks(project)) return runs
  const existing = new Set(runs.map((run) => run.id))
  return [
    ...runs,
    ...mockTaskGraphRunSummaries(project).filter((run) => !existing.has(run.id)),
  ]
}

function mockTaskGraphRunDetail(project: string, runId: string): TaskGraphRunDetail | null {
  const graph_ref = { scope: 'project', id: MOCK_PREGEL_MUTATION_GRAPH_ID, version: 1 } as TaskGraphRef & { version: number }
  if (runId === MOCK_PREGEL_MUTATION_RUN_ID) {
    return normalizeTaskGraphRunDetail({
      id: MOCK_PREGEL_MUTATION_RUN_ID,
      project,
      graph_ref,
      status: 'paused',
      created_at: '2026-05-16T00:00:00.000Z',
      started_at: '2026-05-16T00:00:05.000Z',
      updated_at: '2026-05-16T00:04:00.000Z',
      current_superstep: 2,
      last_checkpoint_id: 'mock-checkpoint-mutation-1',
      current_graph_revision: 1,
      active_nodes: ['review'],
      paused: {
        node_id: 'review',
        reason: 'Review node was added by topology mutation and is waiting for a HumanGate action.',
        actions: [
          { id: 'approve', label: 'Approve', result: 'resume' },
          { id: 'reject', label: 'Reject', result: 'reject' },
        ],
      },
      context: {
        input: { topic: 'topology mutation demo' },
        node_outputs: {
          planner: {
            mutation_batch_id: 'mock-mutation-batch-1',
            summary: mockMutationSummary(),
          },
        },
        branch_decisions: [],
        loop_iterations: [],
      },
      graph_snapshot: mockPregelMutationGraph(1),
      nodes: [
        { node_id: 'start', status: 'succeeded', started_at: '2026-05-16T00:00:05.000Z', completed_at: '2026-05-16T00:00:06.000Z', duration_ms: 1000 },
        { node_id: 'planner', status: 'succeeded', started_at: '2026-05-16T00:00:07.000Z', completed_at: '2026-05-16T00:02:20.000Z', duration_ms: 133000 },
        { node_id: 'review', status: 'paused', started_at: '2026-05-16T00:04:00.000Z' },
        { node_id: 'end', status: 'idle' },
      ],
    })
  }

  if (runId === MOCK_PREGEL_MUTATION_CONFLICT_RUN_ID) {
    return normalizeTaskGraphRunDetail({
      id: MOCK_PREGEL_MUTATION_CONFLICT_RUN_ID,
      project,
      graph_ref,
      status: 'failed',
      created_at: '2026-05-16T00:10:00.000Z',
      started_at: '2026-05-16T00:10:04.000Z',
      updated_at: '2026-05-16T00:11:30.000Z',
      completed_at: '2026-05-16T00:11:30.000Z',
      current_superstep: 1,
      last_checkpoint_id: 'mock-checkpoint-mutation-conflict',
      current_graph_revision: 0,
      active_nodes: [],
      context: {
        input: { topic: 'topology mutation conflict demo' },
        node_outputs: {},
        branch_decisions: [],
        loop_iterations: [],
      },
      graph_snapshot: mockPregelMutationGraph(0),
      nodes: [
        { node_id: 'start', status: 'succeeded', started_at: '2026-05-16T00:10:04.000Z', completed_at: '2026-05-16T00:10:05.000Z', duration_ms: 1000 },
        {
          node_id: 'planner',
          status: 'failed',
          started_at: '2026-05-16T00:10:06.000Z',
          completed_at: '2026-05-16T00:11:30.000Z',
          duration_ms: 84000,
          error: {
            code: 'topology_mutation_conflict',
            message: 'Rejected add_node review because the node id already exists with a different spec.',
          },
        },
        { node_id: 'end', status: 'idle' },
      ],
    })
  }

  return null
}

function mockTopologyMutationEvents(project: string, runId: string): TaskGraphRunEvent[] {
  const _project = project
  if (runId === MOCK_PREGEL_MUTATION_RUN_ID) {
    return [
      {
        id: 'mock-event-topology-mutation-1',
        seq: 1,
        run_id: runId,
        superstep: 1,
        kind: 'topology_mutation',
        message: 'Topology mutation applied.',
        payload: {
          batch_id: 'mock-mutation-batch-1',
          graph_revision_before: 0,
          graph_revision_after: 1,
          result: {
            status: 'applied',
            new_revision: 1,
            summary: mockMutationSummary(),
          },
        } satisfies TopologyMutationEventPayload,
        created_at: '2026-05-16T00:03:30.000Z',
      },
    ]
  }

  if (runId === MOCK_PREGEL_MUTATION_CONFLICT_RUN_ID) {
    return [
      {
        id: 'mock-event-topology-mutation-conflict',
        seq: 1,
        run_id: runId,
        superstep: 1,
        kind: 'topology_mutation',
        message: 'Topology mutation rejected.',
        payload: {
          batch_id: 'mock-mutation-batch-conflict',
          graph_revision_before: 0,
          graph_revision_after: 0,
          result: {
            status: 'rejected',
            conflicts: [
              {
                code: 'topology_mutation_conflict',
                message: 'Node review already exists with a different config.',
                request_ids: ['mock-conflict-add-review'],
                node_id: 'review',
              },
            ],
          },
        } satisfies TopologyMutationEventPayload,
        created_at: '2026-05-16T00:11:20.000Z',
      },
    ]
  }

  return []
}

function mockTaskGraphRunCheckpoints(project: string, runId: string): TaskGraphSuperstepCheckpoint[] {
  const _project = project
  if (runId === MOCK_PREGEL_MUTATION_RUN_ID) {
    return [
      normalizeTaskGraphCheckpoint({
        id: 'mock-checkpoint-mutation-1',
        run_id: runId,
        superstep: 1,
        status: 'paused',
        created_at: '2026-05-16T00:03:40.000Z',
        graph_revision_before: 0,
        graph_revision_after: 1,
        mutation_batch_id: 'mock-mutation-batch-1',
        ready_nodes: ['review'],
        waiting_nodes: ['end'],
        node_statuses: { start: 'succeeded', planner: 'succeeded', review: 'queued', end: 'idle' },
        context: {
          input: { topic: 'topology mutation demo' },
          node_outputs: {},
          branch_decisions: [],
          loop_iterations: [],
        },
        pending_effects: [
          {
            task_id: 'planner-task-1',
            source_node_id: 'planner',
            normal_writes: [],
            graph_mutations: [
              {
                id: 'mock-add-review',
                source_task_id: 'planner-task-1',
                source_node_id: 'planner',
                op: { type: 'add_node', node: mockPregelMutationGraph(1).nodes.find((node) => node.id === 'review')! },
                reason: 'Planner requested a review gate.',
              },
            ],
          },
        ],
      }),
    ]
  }

  if (runId === MOCK_PREGEL_MUTATION_CONFLICT_RUN_ID) {
    return [
      normalizeTaskGraphCheckpoint({
        id: 'mock-checkpoint-mutation-conflict',
        run_id: runId,
        superstep: 1,
        status: 'failed',
        created_at: '2026-05-16T00:11:30.000Z',
        completed_at: '2026-05-16T00:11:30.000Z',
        graph_revision_before: 0,
        graph_revision_after: 0,
        mutation_batch_id: 'mock-mutation-batch-conflict',
        ready_nodes: [],
        waiting_nodes: ['end'],
        node_statuses: { start: 'succeeded', planner: 'failed', end: 'idle' },
        context: {
          input: { topic: 'topology mutation conflict demo' },
          node_outputs: {},
          branch_decisions: [],
          loop_iterations: [],
        },
        message: 'Topology mutation rejected because add_node review conflicted with an existing node id.',
      }),
    ]
  }

  return []
}

export async function loadTaskGraphCatalog(project: string): Promise<TaskGraphCatalogResult> {
  const encoded = encodeURIComponent(project)
  try {
    const payload = await fetchJson<{ graphs: TaskGraphCatalogItem[] }>(`/api/projects/${encoded}/task-graphs`)
    return { graphs: appendMockTaskGraphCatalog(project, payload.graphs ?? []), source: 'rest' }
  } catch (err) {
    if (shouldExposeTaskGraphMocks(project)) return { graphs: [mockPregelMutationCatalogItem()], source: 'mock' }
    throw err
  }
}

export async function readTaskGraph(project: string, ref: TaskGraphRef): Promise<{ graph: TaskGraphDefinition; source: 'rest' | 'mock' }> {
  if (shouldExposeTaskGraphMocks(project) && ref.scope === 'project' && ref.id === MOCK_PREGEL_MUTATION_GRAPH_ID) {
    return { graph: mockPregelMutationGraph(0), source: 'mock' }
  }
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
  try {
    const payload = await fetchJson<{ runs: TaskGraphRunSummary[] }>(`/api/projects/${encoded}/task-graph-runs`)
    return {
      source: 'rest',
      runs: appendMockTaskGraphRuns(project, payload.runs.map(normalizeTaskGraphRunSummary)),
    }
  } catch (err) {
    if (shouldExposeTaskGraphMocks(project)) return { source: 'mock', runs: mockTaskGraphRunSummaries(project) }
    throw err
  }
}

export async function startTaskGraphRun(
  project: string,
  ref: TaskGraphRef,
  input: Record<string, unknown> = {},
  launchOptions: TaskGraphRunLaunchOptions = {},
): Promise<{ run: TaskGraphRunSummary; source: 'rest' | 'mock' }> {
  if (shouldExposeTaskGraphMocks(project) && ref.scope === 'project' && ref.id === MOCK_PREGEL_MUTATION_GRAPH_ID) {
    return { run: mockTaskGraphRunSummaries(project)[0], source: 'mock' }
  }
  const encoded = encodeURIComponent(project)
  const engine = taskGraphEngineOverride()
  const intent = launchOptions.intent?.trim()
  const payload = await fetchJson<{ run: TaskGraphRunSummary }>(`/api/projects/${encoded}/task-graph-runs`, {
    method: 'POST',
    body: JSON.stringify({
      graph: ref,
      input,
      dry_run: false,
      ...(intent ? { intent } : {}),
      ...(launchOptions.resolver ? { resolver: launchOptions.resolver } : {}),
      ...(engine ? { engine } : {}),
    }),
  })
  return { run: normalizeTaskGraphRunSummary(payload.run), source: 'rest' }
}

export async function listTaskGraphSchedules(project: string): Promise<{ schedules: TaskGraphSchedule[]; source: 'rest' }> {
  const encoded = encodeURIComponent(project)
  const payload = await fetchJson<{ schedules: TaskGraphSchedule[] }>(`/api/projects/${encoded}/task-graph-schedules`)
  return { schedules: payload.schedules ?? [], source: 'rest' }
}

export async function createTaskGraphSchedule(
  project: string,
  input: TaskGraphScheduleCreateInput,
): Promise<{ schedule: TaskGraphSchedule; source: 'rest' }> {
  const encoded = encodeURIComponent(project)
  const payload = await fetchJson<{ schedule: TaskGraphSchedule }>(`/api/projects/${encoded}/task-graph-schedules`, {
    method: 'POST',
    body: JSON.stringify(input),
  })
  return { schedule: payload.schedule, source: 'rest' }
}

export async function patchTaskGraphSchedule(
  project: string,
  id: string,
  patch: TaskGraphSchedulePatchInput,
): Promise<{ schedule: TaskGraphSchedule; source: 'rest' }> {
  const encoded = encodeURIComponent(project)
  const payload = await fetchJson<{ schedule: TaskGraphSchedule }>(
    `/api/projects/${encoded}/task-graph-schedules/${encodeURIComponent(id)}`,
    {
      method: 'PATCH',
      body: JSON.stringify(patch),
    },
  )
  return { schedule: payload.schedule, source: 'rest' }
}

export async function deleteTaskGraphSchedule(project: string, id: string): Promise<void> {
  const encoded = encodeURIComponent(project)
  await fetch(`/api/projects/${encoded}/task-graph-schedules/${encodeURIComponent(id)}`, {
    method: 'DELETE',
    cache: 'no-cache',
  }).then((response) => {
    if (!response.ok) throw new Error(`HTTP ${response.status} ${response.statusText}`)
  })
}

export async function runTaskGraphScheduleNow(
  project: string,
  id: string,
): Promise<{ run: TaskGraphRunSummary; source: 'rest' }> {
  const encoded = encodeURIComponent(project)
  const payload = await fetchJson<{ run: TaskGraphRunSummary }>(
    `/api/projects/${encoded}/task-graph-schedules/${encodeURIComponent(id)}/run-now`,
    { method: 'POST' },
  )
  return { run: normalizeTaskGraphRunSummary(payload.run), source: 'rest' }
}

export async function cancelTaskGraphRun(project: string, runId: string): Promise<void> {
  const encoded = encodeURIComponent(project)
  await fetchJson(`/api/projects/${encoded}/task-graph-runs/${encodeURIComponent(runId)}/cancel`, {
    method: 'POST',
  })
}

export async function readTaskGraphRun(project: string, runId: string): Promise<{ run: TaskGraphRunDetail; source: 'rest' | 'mock' }> {
  const encoded = encodeURIComponent(project)
  try {
    const payload = await fetchJson<{ run: TaskGraphRunDetail }>(
      `/api/projects/${encoded}/task-graph-runs/${encodeURIComponent(runId)}`,
    )
    return { run: normalizeTaskGraphRunDetail(payload.run), source: 'rest' }
  } catch (err) {
    const mockRun = shouldExposeTaskGraphMocks(project) ? mockTaskGraphRunDetail(project, runId) : null
    if (mockRun) return { run: mockRun, source: 'mock' }
    throw err
  }
}

export async function readTaskGraphRunEventLog(project: string, runId: string): Promise<TaskGraphRunEvent[]> {
  const encoded = encodeURIComponent(project)
  try {
    const payload = await fetchJson<{ events: TaskGraphRunEvent[] }>(
      `/api/projects/${encoded}/task-graph-runs/${encodeURIComponent(runId)}/event-log`,
    )
    return payload.events
  } catch (err) {
    const mockEvents = shouldExposeTaskGraphMocks(project) ? mockTopologyMutationEvents(project, runId) : []
    if (mockEvents.length > 0) return mockEvents
    throw err
  }
}

export async function readTaskGraphRunCheckpoints(project: string, runId: string): Promise<TaskGraphSuperstepCheckpoint[]> {
  const encoded = encodeURIComponent(project)
  try {
    const payload = await fetchJson<{ checkpoints: TaskGraphSuperstepCheckpoint[] }>(
      `/api/projects/${encoded}/task-graph-runs/${encodeURIComponent(runId)}/checkpoints`,
    )
    return payload.checkpoints.map(normalizeTaskGraphCheckpoint)
  } catch (err) {
    const mockCheckpoints = shouldExposeTaskGraphMocks(project) ? mockTaskGraphRunCheckpoints(project, runId) : []
    if (mockCheckpoints.length > 0) return mockCheckpoints
    throw err
  }
}

export async function resumeTaskGraphGate(
  project: string,
  runId: string,
  nodeId: string,
  actionId: string,
): Promise<{ run: TaskGraphRunSummary; source: 'rest' | 'mock' }> {
  if (
    shouldExposeTaskGraphMocks(project)
    && runId === MOCK_PREGEL_MUTATION_RUN_ID
    && nodeId === 'review'
    && actionId
  ) {
    return { run: mockTaskGraphRunSummaries(project)[0], source: 'mock' }
  }
  const encoded = encodeURIComponent(project)
  const payload = await fetchJson<{ run: TaskGraphRunSummary }>(
    `/api/projects/${encoded}/task-graph-runs/${encodeURIComponent(runId)}/gates/${encodeURIComponent(nodeId)}/resume`,
    {
      method: 'POST',
      body: JSON.stringify({ action: actionId }),
    },
  )
  return { run: normalizeTaskGraphRunSummary(payload.run), source: 'rest' }
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
      const nextRun = normalizeTaskGraphRunDetail(payload.run)
      onRun(nextRun)
      if (['succeeded', 'failed', 'cancelled'].includes(nextRun.status)) {
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
