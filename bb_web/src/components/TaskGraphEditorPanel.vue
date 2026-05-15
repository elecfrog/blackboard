<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { NotifyPlugin, type NotificationInstance } from 'tdesign-vue-next/es/notification'
import {
  AlertTriangle,
  CheckCircle2,
  ListChecks,
  Play,
  Save,
  Settings2,
  X,
} from 'lucide-vue-next'
import GraphCanvas, {
  type GraphCanvasConnection,
  type GraphCanvasEdge,
  type GraphCanvasNode,
  type GraphCanvasNodeMove,
  type GraphCanvasViewport,
} from '@/components/GraphCanvas.vue'
import TaskGraphNodeInspector from '@/components/task-graph/TaskGraphNodeInspector.vue'
import TaskGraphNodePalette from '@/components/task-graph/TaskGraphNodePalette.vue'
import TaskGraphNodeShape from '@/components/task-graph/TaskGraphNodeShape.vue'
import TaskGraphInputsPanel from '@/components/task-graph/TaskGraphInputsPanel.vue'
import {
  taskGraphNodeMetaLabel,
  taskGraphNodeTypes,
  taskGraphNodeVisualForNode,
  type TaskGraphNodeVisual,
} from '@/components/task-graph/taskGraphNodeVisuals'
import {
  saveProjectTaskGraph,
  validateTaskGraph,
  getNodePins,
  nodeToCanvasPins,
  nodeHeightForPins,
  type TaskGraphDefinition,
  type TaskGraphEdge,
  type TaskGraphInputParam,
  type TaskGraphNode,
  type TaskGraphValidationError,
} from '@/data/taskGraphs'
import { loadProjectAgents, type ProjectAgentProfile, type McpServerConfig } from '@/data/agents'
import { t } from '@/i18n'

type NodeType = TaskGraphNode['type']
type EditorConfigPanel = 'inputs' | 'settings' | ''
type PaletteNodeType = TaskGraphNodeVisual

const props = defineProps<{
  project: string
  graph: TaskGraphDefinition
}>()

const emit = defineEmits<{
  saved: [graph: TaskGraphDefinition]
  run: []
  close: []
  'navigate-graph': [graphId: string]
}>()

const nodeTypes = taskGraphNodeTypes()

interface BranchRule {
  id: string
  label: string
  when: {
    path?: string
    op: string
    value?: string | number | boolean
  }
}

type BranchRulePatch = Omit<Partial<BranchRule>, 'when'> & {
  when?: Partial<BranchRule['when']>
}

type GraphInputPatch = Partial<Omit<TaskGraphInputParam, 'default'>> & {
  default?: unknown
}

const promptFileContent = ref('')
const llmRuntimes = ['codex', 'opencode', 'codebuddy']
const branchOps = ['always', 'exists', 'equals', 'not_equals', '>', '>=', '<', '<=', 'contains', 'is_empty', 'not_empty', 'truthy', 'falsy']

const localGraph = ref<TaskGraphDefinition>(cloneGraph(props.graph))
const originalVersion = ref(props.graph.version)
const selectedNodeId = ref('')
const selectedEdgeId = ref('')
const activeConfigPanel = ref<EditorConfigPanel>('')
const saving = ref(false)
let saveNotification: Promise<NotificationInstance> | null = null

function closeSaveNotification() {
  if (!saveNotification) return
  NotifyPlugin.close(saveNotification)
  saveNotification = null
}

function showSaveNotification(theme: 'success' | 'error', title: string, content?: string) {
  closeSaveNotification()
  saveNotification = NotifyPlugin[theme]({
    title,
    content,
    closeBtn: true,
    duration: theme === 'success' ? 4000 : 0,
    placement: 'top-right',
    offset: ['24px', '24px'],
    zIndex: 7000,
    className: 'bb-task-graph-notification',
    onClose: () => {
      saveNotification = null
    },
  })
}

function saveFailureContent(reason: string, action: string, details?: string) {
  return [reason, action, details].filter(Boolean).join('\n\n')
}
const closeConfirmVisible = ref(false)
const availableGraphs = ref<Array<{ id: string; scope: string; title: string }>>([])
const projectAgents = ref<ProjectAgentProfile[]>([])
const projectAgentsError = ref('')
const canvasViewport = ref<GraphCanvasViewport>({ x: 40, y: 36, scale: 1 })
const canvasWrap = ref<HTMLElement | null>(null)
const editorGridSize = 32

const isReadonly = computed(() => {
  // blackboard 项目本身可以编辑 system graph
  if (props.project === 'blackboard') return false
  return localGraph.value.scope !== 'project' || localGraph.value.readonly
})

const canvasNodes = computed<GraphCanvasNode[]>(() =>
  localGraph.value.nodes.map((node) => ({
    id: node.id,
    x: node.position?.x ?? 80,
    y: node.position?.y ?? 120,
    width: 230,
    height: Math.max(nodeHeightForPins(node), 64),
    status: node.type,
    kind: node.type,
    color: taskGraphNodeVisualForNode(node).color,
    label: node.label,
    meta: taskGraphNodeMetaLabel(node, graphInputs.value),
    title: node.label,
    connectable: !isReadonly.value,
    pins: nodeToCanvasPins(node),
    classes: [
      `task-node-${node.type}`,
      ...(selectedNodeId.value === node.id ? ['editor-selected'] : []),
    ],
  })),
)

const canvasEdges = computed<GraphCanvasEdge[]>(() =>
  localGraph.value.edges.map((edge) => ({
    id: edge.id,
    from: edge.from,
    to: edge.to,
    label: edgeDisplayLabel(edge),
    sourceHandle: edge.from_pin ?? edge.source_handle,
    targetHandle: edge.to_pin ?? edge.target_handle,
    removable: !isReadonly.value,
    dashed: edge.kind === 'data',
    edgeColor: edge.kind === 'data' ? '#f97316' : undefined,
    classes: [
      edge.from_pin ? `task-edge-source-${edge.from_pin.replace(/[^a-z0-9-]/gi, '-')}` : '',
      edge.to_pin ? `task-edge-target-${edge.to_pin.replace(/[^a-z0-9-]/gi, '-')}` : '',
      edge.kind === 'data' ? 'edge-data' : 'edge-exec',
      ...(selectedEdgeId.value === edge.id ? ['editor-selected'] : []),
      ...(edgeErrors(edge.id).length > 0 ? ['invalid'] : []),
    ].filter((item): item is string => Boolean(item)),
  })),
)

const canvasSize = computed(() => {
  const nodes = canvasNodes.value
  if (nodes.length === 0) return { width: 1200, height: 720 }
  return {
    width: Math.max(1200, Math.max(...nodes.map((node) => node.x + (node.width ?? 230))) + 160),
    height: Math.max(720, Math.max(...nodes.map((node) => node.y + (node.height ?? 48))) + 160),
  }
})

const selectedNode = computed(() =>
  localGraph.value.nodes.find((node) => node.id === selectedNodeId.value) ?? null,
)

const graphInputs = computed(() => localGraph.value.inputs ?? [])
const graphInputIds = computed(() => graphInputs.value.map((input) => input.id))
const graphInputSummary = computed(() =>
  graphInputs.value.length === 0
    ? t('taskGraphEditorNoInputs')
    : t('taskGraphEditorInputsCount', { count: graphInputs.value.length }),
)
const graphSettingsSummary = computed(() =>
  (localGraph.value.title || localGraph.value.description)
    ? t('taskGraphEditorSettingsConfigured')
    : t('taskGraphEditorSettingsMissing'),
)

const selectedEdge = computed(() =>
  localGraph.value.edges.find((edge) => edge.id === selectedEdgeId.value) ?? null,
)

const validation = computed(() => validateTaskGraph(localGraph.value, { project: props.project }))
const graphErrors = computed(() => validation.value.errors.filter((err) => err.target === 'graph'))
const selectedNodeErrors = computed(() => selectedNodeId.value ? nodeErrors(selectedNodeId.value) : [])
const selectedEdgeErrors = computed(() => selectedEdgeId.value ? edgeErrors(selectedEdgeId.value) : [])
const canSave = computed(() => !isReadonly.value && validation.value.status === 'passed' && !saving.value)

/** 当前不可保存的原因（空字符串表示可以保存） */
const saveBlockReason = computed(() => {
  if (saving.value) return ''
  if (isReadonly.value) return t('taskGraphEditorReadonly')
  if (validation.value.status === 'failed') {
    const count = validation.value.errors.length
    return t('taskGraphEditorErrorCount', { count })
  }
  return ''
})

watch(
  () => props.graph,
  (graph, previousGraph) => {
    localGraph.value = cloneGraph(graph)
    originalVersion.value = graph.version
    selectedNodeId.value = ''
    selectedEdgeId.value = ''
    activeConfigPanel.value = ''
    if (!previousGraph || graph.scope !== previousGraph.scope || graph.id !== previousGraph.id) {
      closeSaveNotification()
    }
  },
)

watch(
  () => props.project,
  () => {
    void loadEditorAgents()
  },
)

onMounted(async () => {
  // 加载可用流水线列表（用于子流水线节点选择）
  try {
    const resp = await fetch(`/api/projects/${props.project}/task-graphs`)
    if (resp.ok) {
      const data = await resp.json()
      availableGraphs.value = (data.graphs ?? []).map((g: { id: string; scope: string; title: string }) => ({
        id: g.id,
        scope: g.scope,
        title: g.title,
      }))
    }
  } catch { /* ignore */ }
  await loadEditorAgents()
})

onUnmounted(() => {
  closeSaveNotification()
})

async function loadEditorAgents() {
  projectAgentsError.value = ''
  try {
    const result = await loadProjectAgents(props.project)
    projectAgents.value = result.agents
  } catch (error) {
    projectAgents.value = []
    projectAgentsError.value = error instanceof Error ? error.message : String(error)
  }
}

function cloneGraph(graph: TaskGraphDefinition): TaskGraphDefinition {
  return JSON.parse(JSON.stringify(graph)) as TaskGraphDefinition
}

function semanticHandleLabel(handle?: string) {
  if (!handle || handle.startsWith('pin:')) return ''
  if (handle === 'body') return t('taskGraphLoopBody')
  if (handle === 'exit') return t('taskGraphLoopCompleted')
  if (handle === 'return') return 'return'
  if (handle.startsWith('rule:')) return handle.slice(5)
  return handle
}

function edgeDisplayLabel(edge: TaskGraphEdge) {
  return edge.label || semanticHandleLabel(edge.source_handle) || semanticHandleLabel(edge.target_handle)
}

function toggleConfigPanel(panel: EditorConfigPanel) {
  const nextPanel = activeConfigPanel.value === panel ? '' : panel
  activeConfigPanel.value = nextPanel
  if (nextPanel) {
    selectedNodeId.value = ''
    selectedEdgeId.value = ''
  }
}

function inputValue(event: Event) {
  return event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLSelectElement
    ? event.target.value
    : ''
}

function numberValue(event: Event) {
  return Number(inputValue(event))
}

function kebab(value: string, fallback: string) {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return normalized || fallback
}

function uniqueNodeId(_type: NodeType) {
  return crypto.randomUUID()
}

function edgeId(_from: string, _to: string, _suffix = '') {
  return crypto.randomUUID()
}

function snapEditorCoordinate(value: number) {
  return Math.round(value / editorGridSize) * editorGridSize
}

function snapEditorPosition(position: { x: number; y: number }) {
  return {
    x: Math.max(editorGridSize, snapEditorCoordinate(position.x)),
    y: Math.max(editorGridSize, snapEditorCoordinate(position.y)),
  }
}

function withPalettePreset(node: TaskGraphNode, item?: PaletteNodeType): TaskGraphNode {
  if (!item) return node
  return {
    ...node,
    label: item.label,
    config: {
      ...node.config,
      ...(item.defaultConfig ?? {}),
    },
  }
}

function defaultNode(type: NodeType, position?: { x: number; y: number }, item?: PaletteNodeType): TaskGraphNode {
  const id = uniqueNodeId(type)
  const visual = item ?? nodeTypes.find((nodeType) => nodeType.type === type)
  const base = {
    id,
    type,
    label: visual?.label ?? type,
    position: snapEditorPosition(position ?? { x: 120 + localGraph.value.nodes.length * 42, y: 130 + localGraph.value.nodes.length * 26 }),
  }
  if (type === 'start') return withPalettePreset({ ...base, config: {} }, item)
  if (type === 'end') return withPalettePreset({ ...base, config: { result: 'succeeded' } }, item)
  if (type === 'llm') {
    return withPalettePreset({
      ...base,
      config: {
        run_as: 'llm',
        runtime: 'codex',
        agent: 'native',
        model: '',
        prompt: { mode: 'inline', template: '' },
        skills: [],
        mcp_servers: [],
        custom_env: {},
        custom_args: [],
        output: { artifact_type: 'markdown', required: true },
      },
    }, item)
  }
  if (type === 'shell') {
    return withPalettePreset({
      ...base,
      label: item?.label ?? t('taskGraphNodeTypeShell'),
      config: {
        cwd: '.',
        command: 'git',
        args: ['status', '--short'],
        env: {},
        timeout_ms: 600000,
        permission: 'read_only',
        expected_exit_codes: [0],
        capture: { max_bytes: 1048576, strip_ansi: true },
      },
    }, item)
  }
  if (type === 'sub_graph') {
    return withPalettePreset({
      ...base,
      label: item?.label ?? t('taskGraphNodeTypeSubPipeline'),
      config: { graph_id: '', graph_scope: 'project', input_bindings: {} },
    }, item)
  }
  if (type === 'human_gate') {
    return withPalettePreset({
      ...base,
      label: item?.label ?? t('taskGraphNodeTypeHumanGate'),
      config: {
        title: t('taskGraphNodeTypeHumanGate'),
        instructions: '',
        actions: [
          { id: 'resume', label: t('taskGraphHumanGateContinue'), result: 'resume' },
          { id: 'reject', label: t('taskGraphHumanGateReject'), result: 'cancel' },
        ],
      },
    }, item)
  }
  if (type === 'branch') {
    return withPalettePreset({
      ...base,
      label: item?.label ?? t('taskGraphNodeTypeBranch'),
      config: {
        mode: 'first_match',
        input_ref: '$.nodes.previous.output',
        rules: [
          { id: 'matched', label: t('taskGraphBranchMatched'), when: { path: '$.ok', op: 'truthy' } },
          { id: 'fallback', label: t('taskGraphBranchDefault'), when: { op: 'always' } },
        ],
        default_rule_id: 'fallback',
      },
    }, item)
  }
  return withPalettePreset({
    ...base,
    label: item?.label ?? t('taskGraphNodeTypeLoop'),
    config: {
      max_iterations: 3,
      max_iterations_ref: '',
      condition: { input_ref: '$.nodes.previous.output', path: '$.failures.length', op: '>', value: 0 },
      body_entry: '',
      body_exit: '',
      on_max_iterations: 'fail',
    },
  }, item)
}

function addNode(item: PaletteNodeType, position?: { x: number; y: number }) {
  if (isReadonly.value) return
  const node = defaultNode(item.type, position, item)
  localGraph.value = {
    ...localGraph.value,
    nodes: [...localGraph.value.nodes, node],
  }
  selectedNodeId.value = node.id
  selectedEdgeId.value = ''
}

function beginPaletteDrag(item: PaletteNodeType, event: DragEvent) {
  event.dataTransfer?.setData('application/x-task-graph-node', JSON.stringify(item))
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'copy'
}

function dropPaletteNode(event: DragEvent) {
  const raw = event.dataTransfer?.getData('application/x-task-graph-node')
  if (!raw) return
  let item: PaletteNodeType | undefined
  try {
    item = JSON.parse(raw) as PaletteNodeType
  } catch {
    item = nodeTypes.find((entry) => entry.type === raw)
  }
  if (!item || !nodeTypes.some((entry) => entry.key === item?.key || (entry.type === item?.type && entry.role === item?.role))) return
  event.preventDefault()
  const rect = canvasWrap.value?.getBoundingClientRect()
  const size = canvasSize.value
  const point = rect
    ? {
        x: (event.clientX - rect.left) * (size.width / rect.width),
        y: (event.clientY - rect.top) * (size.height / rect.height),
      }
    : { x: 140, y: 140 }
  const position = {
    x: Math.max(20, (point.x - canvasViewport.value.x) / canvasViewport.value.scale - 115),
    y: Math.max(20, (point.y - canvasViewport.value.y) / canvasViewport.value.scale - 41),
  }
  addNode(item, snapEditorPosition(position))
}

function updateNode(id: string, updater: (node: TaskGraphNode) => TaskGraphNode) {
  localGraph.value = {
    ...localGraph.value,
    nodes: localGraph.value.nodes.map((node) => (node.id === id ? updater(node) : node)),
  }
}

function updateSelectedLabel(value: string) {
  if (!selectedNode.value) return
  updateNode(selectedNode.value.id, (node) => ({ ...node, label: value }))
}

function updateSelectedConfig(patch: Record<string, unknown>) {
  if (!selectedNode.value) return
  updateNode(selectedNode.value.id, (node) => ({ ...node, config: { ...node.config, ...patch } }))
}

function replaceSelectedConfig(config: Record<string, unknown>) {
  if (!selectedNode.value) return
  updateNode(selectedNode.value.id, (node) => ({ ...node, config }))
}

function updatePromptTemplate(value: string) {
  if (!selectedNode.value) return
  updateNode(selectedNode.value.id, (node) => ({
    ...node,
    config: {
      ...node.config,
      prompt: {
        mode: 'inline',
        ...((node.config.prompt && typeof node.config.prompt === 'object') ? node.config.prompt as Record<string, unknown> : {}),
        template: value,
      },
    },
  }))
}

function promptTemplate(node: TaskGraphNode) {
  const prompt = node.config.prompt
  return prompt && typeof prompt === 'object' && 'template' in prompt
    ? String((prompt as Record<string, unknown>).template ?? '')
    : ''
}

function promptMode(node: TaskGraphNode) {
  const prompt = node.config.prompt
  return prompt && typeof prompt === 'object' && 'mode' in prompt
    ? String((prompt as Record<string, unknown>).mode ?? 'inline')
    : 'inline'
}

function updatePromptMode(mode: string) {
  if (!selectedNode.value) return
  const prompt = selectedNode.value.config.prompt as Record<string, unknown> ?? { mode: 'inline', template: '' }
  updateSelectedConfig({ prompt: { ...prompt, mode } })
}

async function loadPromptFile() {
  if (!selectedNode.value) return
  const filePath = promptTemplate(selectedNode.value)
  if (!filePath) return
  const scope = localGraph.value.scope === 'system' ? 'system' : 'project'
  const graphId = localGraph.value.id
  try {
    const resp = await fetch(`/api/projects/${props.project}/task-graphs/${scope}/${graphId}/prompt-file?path=${encodeURIComponent(filePath)}`)
    if (resp.ok) {
      const data = await resp.json()
      promptFileContent.value = data.content ?? ''
    }
  } catch { /* ignore */ }
}

async function savePromptFile() {
  if (!selectedNode.value || !promptFileContent.value) return
  const filePath = promptTemplate(selectedNode.value)
  if (!filePath) return
  const scope = localGraph.value.scope === 'system' ? 'system' : 'project'
  const graphId = localGraph.value.id
  try {
    await fetch(`/api/projects/${props.project}/task-graphs/${scope}/${graphId}/prompt-file`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ path: filePath, content: promptFileContent.value }),
    })
  } catch { /* ignore */ }
}

function configString(node: TaskGraphNode, key: string) {
  const value = node.config[key]
  return typeof value === 'string' ? value : ''
}

function llmRunAs(node: TaskGraphNode): 'llm' | 'agent' {
  return node.config.run_as === 'agent' ? 'agent' : 'llm'
}

const selectedAgentProfile = computed(() => {
  const node = selectedNode.value
  if (!node || node.type !== 'llm' || llmRunAs(node) !== 'agent') return null
  const profileId = configString(node, 'agent_profile')
  return projectAgents.value.find((agent) => agent.id === profileId) ?? null
})

function switchLlmRunAs(mode: 'llm' | 'agent') {
  if (!selectedNode.value) return
  const node = selectedNode.value
  const common = {
    inputs: node.config.inputs ?? {},
    output: node.config.output ?? { artifact_type: 'markdown', required: true },
  }
  if (mode === 'agent') {
    replaceSelectedConfig({
      run_as: 'agent',
      agent_profile: configString(node, 'agent_profile') || projectAgents.value[0]?.id || '',
      prompt: promptMode(node) === 'file'
        ? { mode: 'file', template: promptTemplate(node) }
        : { mode: 'inline', template: promptTemplate(node) },
      ...common,
    })
    return
  }
  replaceSelectedConfig({
    run_as: 'llm',
    runtime: configString(node, 'runtime') || 'codex',
    agent: configString(node, 'agent') || 'native',
    model: configString(node, 'model'),
    prompt: promptMode(node) === 'inline'
      ? { mode: 'inline', template: promptTemplate(node) }
      : { mode: 'inline', template: '' },
    skills: llmSkills(node),
    mcp_servers: llmMcpServers(node),
    custom_env: llmCustomEnv(node),
    custom_args: llmCustomArgs(node),
    ...common,
  })
}

function agentLabel(agent: ProjectAgentProfile) {
  return agent.display_name ? `${agent.display_name} (${agent.id})` : agent.id
}

function profileRuntimeSummary(profile: ProjectAgentProfile | null) {
  if (!profile) return ''
  return [
    profile.runtime ? `runtime ${profile.runtime}` : 'runtime unset',
    profile.model ? `model ${profile.model}` : 'model unset',
    `${profile.skills?.length ?? 0} skills`,
    `${profile.mcp_servers?.length ?? 0} MCP`,
  ].join(' · ')
}

function agentTaskPromptHint(profile: ProjectAgentProfile | null) {
  return profile?.instructions_path
    ? `Empty prompt uses profile default: ${profile.instructions_path}`
    : 'Task prompt is required because this profile has no default prompt.'
}

function llmSkills(node: TaskGraphNode) {
  const value = node.config.skills
  return Array.isArray(value) ? value.map(String) : []
}

function addLlmSkill() {
  if (!selectedNode.value) return
  updateSelectedConfig({ skills: [...llmSkills(selectedNode.value), ''] })
}

function updateLlmSkill(index: number, value: string) {
  if (!selectedNode.value) return
  const skills = [...llmSkills(selectedNode.value)]
  skills[index] = value
  updateSelectedConfig({ skills })
}

function removeLlmSkill(index: number) {
  if (!selectedNode.value) return
  updateSelectedConfig({ skills: llmSkills(selectedNode.value).filter((_, itemIndex) => itemIndex !== index) })
}

function llmMcpServers(node: TaskGraphNode): McpServerConfig[] {
  const value = node.config.mcp_servers
  return Array.isArray(value)
    ? value.map((item) => {
        const server = item as Partial<McpServerConfig>
        return {
          name: typeof server.name === 'string' ? server.name : '',
          transport: server.transport === 'sse' ? 'sse' : 'stdio',
          command: typeof server.command === 'string' ? server.command : '',
          args: Array.isArray(server.args) ? server.args.map(String) : [],
          url: typeof server.url === 'string' ? server.url : '',
          env: server.env && typeof server.env === 'object' && !Array.isArray(server.env)
            ? server.env as Record<string, string>
            : {},
        }
      })
    : []
}

function addMcpServer() {
  if (!selectedNode.value) return
  updateSelectedConfig({
    mcp_servers: [
      ...llmMcpServers(selectedNode.value),
      { name: '', transport: 'stdio', command: '', args: [], env: {} },
    ],
  })
}

function updateMcpServer(index: number, patch: Partial<McpServerConfig>) {
  if (!selectedNode.value) return
  const servers = llmMcpServers(selectedNode.value)
  servers[index] = { ...servers[index], ...patch }
  updateSelectedConfig({ mcp_servers: servers })
}

function mcpTransport(value: string): 'stdio' | 'sse' {
  return value === 'sse' ? 'sse' : 'stdio'
}

function removeMcpServer(index: number) {
  if (!selectedNode.value) return
  updateSelectedConfig({ mcp_servers: llmMcpServers(selectedNode.value).filter((_, itemIndex) => itemIndex !== index) })
}

function mcpArgsText(server: McpServerConfig) {
  return (server.args ?? []).join('\n')
}

function updateMcpArgs(index: number, value: string) {
  updateMcpServer(index, { args: value.split(/\r?\n/).map((item) => item.trim()).filter(Boolean) })
}

function mcpEnvText(server: McpServerConfig) {
  return JSON.stringify(server.env ?? {}, null, 2)
}

function updateMcpEnv(index: number, value: string) {
  try {
    const parsed = JSON.parse(value) as Record<string, string>
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      updateMcpServer(index, { env: parsed })
    }
  } catch {
    // Keep the last valid env until the user leaves valid JSON.
  }
}

function llmCustomEnv(node: TaskGraphNode): Record<string, string> {
  const value = node.config.custom_env
  return value && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, string>
    : {}
}

function customEnvEntries(node: TaskGraphNode) {
  return Object.entries(llmCustomEnv(node))
}

function addCustomEnv() {
  if (!selectedNode.value) return
  const env = { ...llmCustomEnv(selectedNode.value) }
  let index = Object.keys(env).length + 1
  let key = `KEY_${index}`
  while (env[key] !== undefined) {
    index += 1
    key = `KEY_${index}`
  }
  env[key] = ''
  updateSelectedConfig({ custom_env: env })
}

function updateCustomEnvKey(oldKey: string, newKey: string) {
  if (!selectedNode.value || !newKey || oldKey === newKey) return
  const env = { ...llmCustomEnv(selectedNode.value) }
  const value = env[oldKey] ?? ''
  delete env[oldKey]
  env[newKey] = value
  updateSelectedConfig({ custom_env: env })
}

function updateCustomEnvValue(key: string, value: string) {
  if (!selectedNode.value) return
  updateSelectedConfig({ custom_env: { ...llmCustomEnv(selectedNode.value), [key]: value } })
}

function removeCustomEnv(key: string) {
  if (!selectedNode.value) return
  const env = { ...llmCustomEnv(selectedNode.value) }
  delete env[key]
  updateSelectedConfig({ custom_env: env })
}

function llmCustomArgs(node: TaskGraphNode) {
  const value = node.config.custom_args
  return Array.isArray(value) ? value.map(String) : []
}

function addCustomArg() {
  if (!selectedNode.value) return
  updateSelectedConfig({ custom_args: [...llmCustomArgs(selectedNode.value), ''] })
}

function updateCustomArg(index: number, value: string) {
  if (!selectedNode.value) return
  const args = [...llmCustomArgs(selectedNode.value)]
  args[index] = value
  updateSelectedConfig({ custom_args: args })
}

function removeCustomArg(index: number) {
  if (!selectedNode.value) return
  updateSelectedConfig({ custom_args: llmCustomArgs(selectedNode.value).filter((_, itemIndex) => itemIndex !== index) })
}

function configNumber(node: TaskGraphNode, key: string) {
  const value = node.config[key]
  return typeof value === 'number' ? value : Number(value || 0)
}

function inputReference(inputId: string) {
  return `{{inputs.${inputId}}}`
}

function defaultValueForInputType(type: TaskGraphInputParam['type']) {
  if (type === 'number') return 1
  if (type === 'boolean') return false
  if (type === 'json') return {}
  if (type.startsWith('array<')) return []
  return ''
}

function updateGraphInput(index: number, patch: GraphInputPatch) {
  if (isReadonly.value) return
  const inputs = [...graphInputs.value]
  const current = inputs[index]
  if (!current) return
  const nextType = patch.type ?? current.type
  inputs[index] = {
    ...current,
    ...patch,
    default: patch.default ?? (patch.type && patch.type !== current.type ? defaultValueForInputType(nextType) : current.default),
  }
  localGraph.value = { ...localGraph.value, inputs }
}

function updateGraphInputType(index: number, value: string) {
  const validTypes = ['string', 'number', 'boolean', 'json', 'ticket_ref']
  if (validTypes.includes(value) || value.startsWith('array<')) {
    updateGraphInput(index, { type: value as TaskGraphInputParam['type'] })
  }
}

function addGraphInput() {
  if (isReadonly.value) return
  const existing = new Set(graphInputs.value.map((input) => input.id))
  let index = graphInputs.value.length + 1
  let id = `input-${index}`
  while (existing.has(id)) {
    index += 1
    id = `input-${index}`
  }
  localGraph.value = {
    ...localGraph.value,
      inputs: [
      ...graphInputs.value,
      { id, label: t('taskGraphInputWithIndex', { index: String(index) }), type: 'number', default: 1, min: 1 },
    ],
  }
}

function removeGraphInput(index: number) {
  if (isReadonly.value) return
  localGraph.value = {
    ...localGraph.value,
    inputs: graphInputs.value.filter((_, itemIndex) => itemIndex !== index),
  }
}

function llmInputs(node: TaskGraphNode) {
  const raw = node.config.inputs
  return raw && typeof raw === 'object' && !Array.isArray(raw)
    ? raw as Record<string, unknown>
    : {}
}

function updateLlmInput(key: string, value: string) {
  if (!selectedNode.value) return
  const inputs = { ...llmInputs(selectedNode.value), [key]: value }
  updateSelectedConfig({ inputs })
}

function renameLlmInput(oldKey: string, newKey: string) {
  if (!selectedNode.value || !newKey || oldKey === newKey) return
  const inputs = { ...llmInputs(selectedNode.value) }
  const value = inputs[oldKey]
  delete inputs[oldKey]
  inputs[newKey] = value ?? ''
  updateSelectedConfig({ inputs })
}

function removeLlmInput(key: string) {
  if (!selectedNode.value) return
  const inputs = { ...llmInputs(selectedNode.value) }
  delete inputs[key]
  updateSelectedConfig({ inputs })
}

function addLlmInput() {
  if (!selectedNode.value) return
  const inputs = { ...llmInputs(selectedNode.value) }
  let idx = 1
  while (inputs[`input_${idx}`] !== undefined) idx++
  inputs[`input_${idx}`] = ''
  updateSelectedConfig({ inputs })
}

function promptVars(node: TaskGraphNode) {
  const raw = node.config.prompt_vars
  return raw && typeof raw === 'object' && !Array.isArray(raw)
    ? raw as Record<string, unknown>
    : {}
}

function updatePromptVar(key: string, value: string) {
  if (!selectedNode.value) return
  const vars = { ...promptVars(selectedNode.value), [key]: value }
  updateSelectedConfig({ prompt_vars: vars })
}

function renamePromptVar(oldKey: string, newKey: string) {
  if (!selectedNode.value || !newKey || oldKey === newKey) return
  const vars = { ...promptVars(selectedNode.value) }
  const value = vars[oldKey]
  delete vars[oldKey]
  vars[newKey] = value ?? ''
  updateSelectedConfig({ prompt_vars: vars })
}

function removePromptVar(key: string) {
  if (!selectedNode.value) return
  const vars = { ...promptVars(selectedNode.value) }
  delete vars[key]
  updateSelectedConfig({ prompt_vars: vars })
}

function addPromptVar() {
  if (!selectedNode.value) return
  const vars = { ...promptVars(selectedNode.value) }
  let idx = 1
  while (vars[`var_${idx}`] !== undefined) idx++
  vars[`var_${idx}`] = ''
  updateSelectedConfig({ prompt_vars: vars })
}

// ─── Sub-Graph helpers ────────────────────────────────────────────────────────

function subGraphBindings(node: TaskGraphNode): Record<string, unknown> {
  const raw = node.config.input_bindings
  return raw && typeof raw === 'object' && !Array.isArray(raw)
    ? raw as Record<string, unknown>
    : {}
}

function updateSubGraphBinding(key: string, value: string) {
  if (!selectedNode.value) return
  const bindings = { ...subGraphBindings(selectedNode.value), [key]: value }
  updateSelectedConfig({ input_bindings: bindings })
}

async function loadSubGraphInputs() {
  if (!selectedNode.value) return
  const graphId = configString(selectedNode.value, 'graph_id')
  const graphScope = configString(selectedNode.value, 'graph_scope') || 'project'
  if (!graphId) return

  try {
    const resp = await fetch(`/api/projects/${props.project}/task-graphs/${graphScope}/${graphId}/inputs`)
    if (resp.ok) {
      const data = await resp.json()
      const inputs = data.inputs as Array<{ id: string; default?: unknown }> ?? []
      const currentBindings = subGraphBindings(selectedNode.value)
      const newBindings: Record<string, unknown> = {}
      for (const input of inputs) {
        newBindings[input.id] = currentBindings[input.id] ?? ''
      }
      updateSelectedConfig({ input_bindings: newBindings })
    }
  } catch { /* ignore */ }
}

function onSubGraphSelect(event: Event) {
  const graphId = inputValue(event)
  if (!graphId) {
    updateSelectedConfig({ graph_id: '', graph_scope: 'project' })
    return
  }
  // 从 availableGraphs 中查找选中图的 scope，自动同步 graph_scope
  const matched = availableGraphs.value.find(g => g.id === graphId)
  const scope = matched?.scope ?? 'project'
  updateSelectedConfig({ graph_id: graphId, graph_scope: scope })
}

function openSubGraph() {
  if (!selectedNode.value) return
  const graphId = configString(selectedNode.value, 'graph_id')
  if (graphId) {
    // Emit event to parent to navigate to sub-graph
    emit('navigate-graph', graphId)
  }
}
function bindSelectedLoopIterations(inputId: string) {
  if (!selectedNode.value || selectedNode.value.type !== 'loop') return
  updateSelectedConfig({ max_iterations_ref: inputReference(inputId) })
}

function getBranchRules(node: TaskGraphNode): BranchRule[] {
  const rules = node.config.rules
  if (!Array.isArray(rules)) return []
  return rules.map((rule) => {
    const value = rule && typeof rule === 'object' ? rule as Record<string, unknown> : {}
    const when = value.when && typeof value.when === 'object' ? value.when as Record<string, unknown> : {}
    return {
      id: String(value.id ?? ''),
      label: String(value.label ?? value.id ?? ''),
      when: {
        path: typeof when.path === 'string' ? when.path : '',
        op: typeof when.op === 'string' ? when.op : 'always',
        value: typeof when.value === 'string' || typeof when.value === 'number' || typeof when.value === 'boolean' ? when.value : '',
      },
    }
  })
}

function setBranchRules(rules: BranchRule[]) {
  updateSelectedConfig({ rules })
}

function updateBranchRule(index: number, patch: BranchRulePatch) {
  const node = selectedNode.value
  if (!node) return
  const rules = getBranchRules(node)
  const current = rules[index]
  if (!current) return
  rules[index] = {
    ...current,
    ...patch,
    when: {
      ...current.when,
      ...(patch.when ?? {}),
    },
  }
  setBranchRules(rules)
}

function addBranchRule() {
  const node = selectedNode.value
  if (!node) return
  const rules = getBranchRules(node)
  const id = kebab(`rule-${rules.length + 1}`, `rule-${rules.length + 1}`)
  setBranchRules([...rules, { id, label: t('taskGraphRuleWithIndex', { index: String(rules.length + 1) }), when: { op: 'always' } }])
}

function removeBranchRule(index: number) {
  const node = selectedNode.value
  if (!node) return
  const removed = getBranchRules(node)[index]
  const rules = getBranchRules(node).filter((_, itemIndex) => itemIndex !== index)
  const defaultRule = configString(node, 'default_rule_id')
  updateSelectedConfig({
    rules,
    default_rule_id: removed?.id === defaultRule ? rules[0]?.id ?? '' : defaultRule,
  })
}

function updateEdge(id: string, updater: (edge: TaskGraphEdge) => TaskGraphEdge) {
  localGraph.value = {
    ...localGraph.value,
    edges: localGraph.value.edges.map((edge) => (edge.id === id ? updater(edge) : edge)),
  }
}

function removeSelectedNode() {
  const node = selectedNode.value
  if (!node || isReadonly.value) return
  localGraph.value = {
    ...localGraph.value,
    nodes: localGraph.value.nodes.filter((item) => item.id !== node.id),
    edges: localGraph.value.edges.filter((edge) => edge.from !== node.id && edge.to !== node.id),
  }
  selectedNodeId.value = ''
  selectedEdgeId.value = ''
}

function removeSelectedEdge() {
  const edge = selectedEdge.value
  if (!edge || isReadonly.value) return
  localGraph.value = {
    ...localGraph.value,
    edges: localGraph.value.edges.filter((item) => item.id !== edge.id),
  }
  selectedEdgeId.value = ''
}

function isEditableKeyboardTarget(target: EventTarget | null) {
  return target instanceof HTMLInputElement
    || target instanceof HTMLTextAreaElement
    || target instanceof HTMLSelectElement
    || (target instanceof HTMLElement && target.isContentEditable)
}

function focusCanvasWrap() {
  canvasWrap.value?.focus({ preventScroll: true })
}

function handleEditorKeydown(event: KeyboardEvent) {
  if (isReadonly.value || isEditableKeyboardTarget(event.target)) return
  if (event.key !== 'Delete' && event.key !== 'Backspace') return
  if (selectedNode.value) {
    event.preventDefault()
    removeSelectedNode()
    return
  }
  if (selectedEdge.value) {
    event.preventDefault()
    removeSelectedEdge()
  }
}

function handleNodeMove(move: GraphCanvasNodeMove) {
  updateNode(move.id, (node) => ({ ...node, position: { x: move.x, y: move.y } }))
}

function handleNodeSelect(id: string) {
  activeConfigPanel.value = ''
  selectedNodeId.value = id
  selectedEdgeId.value = ''
  focusCanvasWrap()
}

function connectionDefaults(connection: GraphCanvasConnection) {
  const source = localGraph.value.nodes.find((node) => node.id === connection.sourceId)
  const target = localGraph.value.nodes.find((node) => node.id === connection.targetId)

  // 使用 sourceHandle 作为 from_pin（来自 GraphCanvas pin 的 handle）
  const fromPin = connection.sourceHandle ?? 'exec_out'
  let toPin = connection.targetHandle ?? 'exec_in'

  // 确定 edge kind：根据 source pin 的 category
  let edgeKind: TaskGraphEdge['kind'] = 'exec'
  if (source) {
    const pins = getNodePins(source)
    const srcPin = pins.find((p) => p.id === fromPin)
    if (srcPin?.category === 'data') edgeKind = 'data'
  }

  const edge: Omit<TaskGraphEdge, 'id'> = {
    from: connection.sourceId,
    to: connection.targetId,
    kind: edgeKind,
    from_pin: fromPin,
    to_pin: toPin,
    source_handle: fromPin === 'exec_out' ? undefined : fromPin,
    target_handle: toPin === 'exec_in' ? undefined : toPin,
  }

  // Branch 节点：自动分配未使用的 rule pin
  if (source?.type === 'branch' && fromPin === 'exec_out') {
    const rules = getBranchRules(source)
    const used = new Set(localGraph.value.edges.filter((item) => item.from === source.id).map((item) => item.from_pin ?? item.source_handle))
    const rule = rules.find((item) => !used.has(`rule:${item.id}`)) ?? rules[0]
    if (rule) {
      edge.from_pin = `rule:${rule.id}`
      edge.source_handle = `rule:${rule.id}`
      edge.label = rule.label
    }
  } else if (source?.type === 'loop' && fromPin === 'exec_out') {
    const used = new Set(localGraph.value.edges.filter((item) => item.from === source.id).map((item) => item.from_pin ?? item.source_handle))
    if (!used.has('body')) {
      edge.from_pin = 'body'
      edge.source_handle = 'body'
      edge.label = t('taskGraphLoopBody')
    } else if (!used.has('exit')) {
      edge.from_pin = 'exit'
      edge.source_handle = 'exit'
      edge.label = t('taskGraphLoopCompleted')
    }
  }

  return edge
}

function edgeIdSuffix(edge: Omit<TaskGraphEdge, 'id'>) {
  if (edge.source_handle?.startsWith('rule:')) return edge.source_handle.slice(5)
  if (edge.source_handle && !edge.source_handle.startsWith('pin:')) return edge.source_handle
  if (edge.target_handle && !edge.target_handle.startsWith('pin:')) return edge.target_handle
  return ''
}

function handleConnectionCreate(connection: GraphCanvasConnection) {
  if (isReadonly.value || connection.sourceId === connection.targetId) return
  const defaults = connectionDefaults(connection)
  const suffix = edgeIdSuffix(defaults)
  const edge: TaskGraphEdge = {
    id: edgeId(defaults.from, defaults.to, suffix),
    ...defaults,
  }
  localGraph.value = {
    ...localGraph.value,
    edges: [...localGraph.value.edges, edge],
  }
  selectedEdgeId.value = edge.id
  selectedNodeId.value = ''
}

function handleEdgeRemove(edge: GraphCanvasEdge) {
  if (isReadonly.value) return
  localGraph.value = {
    ...localGraph.value,
    edges: localGraph.value.edges.filter((item) => item.id !== edge.id),
  }
  if (selectedEdgeId.value === edge.id) selectedEdgeId.value = ''
}

function handleEdgeSelect(edge: GraphCanvasEdge) {
  selectedEdgeId.value = edge.id ?? ''
  selectedNodeId.value = ''
}

async function saveGraph() {
  closeSaveNotification()
  // 只读 graph 不可保存
  if (isReadonly.value) {
    showSaveNotification(
      'error',
      t('taskGraphSaveFailed'),
      saveFailureContent(t('taskGraphEditorReadonly'), t('taskGraphSaveReadonlyAction')),
    )
    return
  }
  // 校验失败时给出详细错误
  const result = validateTaskGraph(localGraph.value, { project: props.project })
  if (result.status === 'failed') {
    const details = result.errors.slice(0, 5).map((e) => e.message).join('\n• ')
    const suffix = result.errors.length > 5 ? `\n...（共 ${result.errors.length} 个错误）` : ''
    showSaveNotification(
      'error',
      t('taskGraphSaveFailed'),
      saveFailureContent(
        t('taskGraphSaveValidationReason'),
        t('taskGraphSaveValidationAction'),
        `• ${details}${suffix}`,
      ),
    )
    return
  }
  saving.value = true
  try {
    const saved = await saveProjectTaskGraph(props.project, localGraph.value, originalVersion.value)
    localGraph.value = cloneGraph(saved.graph)
    originalVersion.value = saved.graph.version
    emit('saved', saved.graph)
    const msg = saved.source === 'mock' ? t('taskGraphEditorMockSaved') : t('taskGraphEditorSaved')
    showSaveNotification('success', msg)
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err)
    showSaveNotification(
      'error',
      t('taskGraphSaveFailed'),
      saveFailureContent(t('taskGraphSaveRequestReason'), t('taskGraphSaveRequestAction'), msg),
    )
  } finally {
    saving.value = false
  }
}

function nodeErrors(id: string): TaskGraphValidationError[] {
  return validation.value.errors.filter((err) => err.target === 'node' && err.id === id)
}

function edgeErrors(id: string): TaskGraphValidationError[] {
  return validation.value.errors.filter((err) => err.target === 'edge' && err.id === id)
}

function handleClose() {
  if (validation.value.status === 'failed') {
    closeConfirmVisible.value = true
    return
  }
  emit('close')
}

function confirmClose() {
  closeConfirmVisible.value = false
  emit('close')
}

function cancelClose() {
  closeConfirmVisible.value = false
}
</script>

<template>
  <section class="task-graph-editor">
    <header class="task-graph-editor-head">
      <div>
        <span>{{ t('taskGraphEditor') }}</span>
        <h3>{{ localGraph.title }}</h3>
        <p>{{ localGraph.scope }}/{{ localGraph.id }} · v{{ localGraph.version }}</p>
      </div>
      <div>
        <button type="button" class="bb-top-action-button" :disabled="saving" @click="saveGraph" :title="saveBlockReason">
          <Save class="bb-top-action-svg" aria-hidden="true" />
          <span>{{ saving ? t('saving') : t('save') }}</span>
        </button>
        <button type="button" class="bb-top-action-button" :disabled="saving" @click="emit('run')">
          <Play class="bb-top-action-svg" aria-hidden="true" />
          <span>{{ t('taskGraphRun') }}</span>
        </button>
        <button type="button" class="bb-top-action-button" @click="handleClose">
          <X class="bb-top-action-svg" aria-hidden="true" />
          <span>{{ t('close') }}</span>
        </button>
      </div>
    </header>

    <div class="task-graph-editor-grid">
      <main class="task-graph-editor-main">
        <section class="task-graph-editor-summary-strip">
          <button
            type="button"
            :class="['task-graph-editor-summary-chip', { active: activeConfigPanel === 'inputs' }]"
            @click="toggleConfigPanel('inputs')"
          >
            <ListChecks aria-hidden="true" />
            <span>
              <strong>{{ t('taskGraphInputs') }}</strong>
              <small>{{ graphInputSummary }}</small>
            </span>
          </button>
          <button
            type="button"
            :class="['task-graph-editor-summary-chip', { active: activeConfigPanel === 'settings' }]"
            @click="toggleConfigPanel('settings')"
          >
            <Settings2 aria-hidden="true" />
            <span>
              <strong>{{ t('taskGraphSettings') }}</strong>
              <small>{{ graphSettingsSummary }}</small>
            </span>
          </button>
          <button
            v-if="activeConfigPanel"
            type="button"
            class="task-graph-editor-summary-close"
            @click="activeConfigPanel = ''"
          >
            <X aria-hidden="true" />
            <span>{{ t('taskGraphEditorCloseConfig') }}</span>
          </button>
        </section>

        <Transition name="task-graph-config-panel">
          <section
            v-if="activeConfigPanel"
            class="task-graph-editor-config-panel"
            :class="`task-graph-editor-config-panel-${activeConfigPanel}`"
          >
            <TaskGraphInputsPanel
              v-if="activeConfigPanel === 'inputs'"
              :inputs="graphInputs"
              :readonly="isReadonly"
              layout="drawer"
              @add="addGraphInput"
              @remove="removeGraphInput"
              @update="updateGraphInput"
              @type-change="updateGraphInputType"
            />

            <section v-else class="task-graph-settings">
              <h4>{{ t('taskGraphSettings') }}</h4>
              <label>
                <span>{{ t('label') }}</span>
                <input :value="localGraph.title" @input="localGraph = { ...localGraph, title: inputValue($event) }" />
              </label>
              <label>
                <span>{{ t('description') }}</span>
                <textarea :value="localGraph.description ?? ''" @input="localGraph = { ...localGraph, description: inputValue($event) }" />
              </label>
            </section>
          </section>
        </Transition>

        <div
          ref="canvasWrap"
          class="task-graph-editor-canvas-wrap"
          tabindex="0"
          :aria-label="t('taskGraphCanvasEditor')"
          @dragover.prevent
          @drop="dropPaletteNode"
          @mousedown="focusCanvasWrap"
          @keydown="handleEditorKeydown"
        >
          <GraphCanvas
            class="ticket-graph-canvas task-graph-editor-canvas"
            :readonly="isReadonly"
            :nodes="canvasNodes"
            :edges="canvasEdges"
            :canvas-size="canvasSize"
            :default-node-width="230"
            :default-node-height="48"
            :fit-padding="48"
            :saving-id="saving ? selectedNodeId : null"
            :node-grid-size="editorGridSize"
            :node-grid-snap-threshold="7"
            :node-align-threshold="10"
            :node-snap-start-distance="12"
            @viewport-change="canvasViewport = $event"
            @node-select="handleNodeSelect"
            @node-move="handleNodeMove"
            @node-move-end="handleNodeMove"
            @connection-create="handleConnectionCreate"
            @edge-remove="handleEdgeRemove"
            @edge-select="handleEdgeSelect"
          >
            <template #node="{ node, width, height }">
              <TaskGraphNodeShape :node="node" :width="width" :height="height" />
            </template>
            <template #edge-label="{ edge, midpoint }">
              <text
                v-if="edge.label"
                class="task-graph-editor-edge-label"
                :x="midpoint.x"
                :y="midpoint.y - 10"
              >
                {{ edge.label }}
              </text>
            </template>
          </GraphCanvas>

          <div class="task-graph-editor-overlay-stack">
            <TaskGraphNodePalette
              :node-types="nodeTypes"
              @add-node="addNode"
              @begin-drag="beginPaletteDrag"
            />

            <section class="task-graph-validation-card task-graph-editor-overlay">
              <h4>{{ t('taskGraphValidation') }}</h4>
              <div :class="['task-graph-validation-state', validation.status]">
                <CheckCircle2 v-if="validation.status === 'passed'" aria-hidden="true" />
                <AlertTriangle v-else aria-hidden="true" />
                <strong>{{ validation.status }}</strong>
              </div>
              <ul v-if="validation.errors.length > 0" class="task-graph-error-list">
                <li v-for="(err, index) in validation.errors.slice(0, 8)" :key="`${err.target}-${err.id}-${index}`">
                  {{ err.id ? `${err.id}: ` : '' }}{{ err.message }}
                </li>
              </ul>
            </section>
          </div>

          <TaskGraphNodeInspector
            v-if="selectedNode && !activeConfigPanel"
            :node="selectedNode"
            :readonly="isReadonly"
            :project-agents="projectAgents"
            :project-agents-error="projectAgentsError"
            :graph-inputs="graphInputs"
            :graph-input-ids="graphInputIds"
            :available-graphs="availableGraphs"
            :project="project"
            :graph-scope="localGraph.scope"
            :graph-id="localGraph.id"
            :selected-node-errors="selectedNodeErrors"
            :prompt-file-content="promptFileContent"
            @update-config="updateSelectedConfig"
            @switch-mode="switchLlmRunAs"
            @load-prompt-file="loadPromptFile"
            @update-prompt-template="updatePromptTemplate"
            @update-prompt-mode="updatePromptMode"
            @save-prompt-file="savePromptFile"
            @remove="removeSelectedNode"
            @close="selectedNodeId = ''"
            @update-label="updateSelectedLabel"
            @open-sub-graph="(id) => emit('navigate-graph', id)"
          />
        </div>
      </main>

    </div>

    <Teleport to="body">
      <div
        v-if="closeConfirmVisible"
        class="task-graph-close-confirm-backdrop"
        role="presentation"
        @click.self="cancelClose"
      >
        <section class="task-graph-close-confirm-dialog" role="alertdialog" aria-modal="true">
          <header>
            <AlertTriangle aria-hidden="true" />
            <h3>{{ t('taskGraphCloseConfirmTitle') }}</h3>
          </header>
          <p>{{ t('taskGraphCloseConfirmMessage') }}</p>
          <ul class="task-graph-close-confirm-errors">
            <li v-for="(err, index) in validation.errors.slice(0, 6)" :key="index">
              <strong v-if="err.id">{{ err.id }}:</strong> {{ err.message }}
            </li>
          </ul>
          <p class="task-graph-close-confirm-hint">{{ t('taskGraphCloseConfirmHint') }}</p>
          <footer>
            <button type="button" class="bb-top-action-button" @click="cancelClose">
              <span>{{ t('taskGraphCloseConfirmStay') }}</span>
            </button>
            <button type="button" class="bb-top-action-button task-graph-close-confirm-discard" @click="confirmClose">
              <span>{{ t('taskGraphCloseConfirmDiscard') }}</span>
            </button>
          </footer>
        </section>
      </div>
    </Teleport>
  </section>
</template>

<style scoped>
.task-graph-editor {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  align-content: stretch;
  gap: 0;
  min-width: 0;
  min-height: 0;
  height: 100%;
  overflow: hidden;
}

.task-graph-editor-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
  min-height: 58px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--bb-border-warm);
}

.task-graph-editor-head > div:first-child {
  min-width: 0;
  display: grid;
  gap: 2px;
}

.task-graph-editor-head > div:last-child {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
}

.task-graph-editor-head span,
.task-graph-editor-head p,
.task-graph-palette h4,
.task-graph-inspector h4 {
  color: var(--bb-text-muted);
  font-size: 12px;
  font-weight: 760;
}

.task-graph-editor-head h3 {
  margin: 3px 0 0;
  overflow-wrap: anywhere;
  color: var(--bb-text-strong);
  font-size: 16px;
}

.task-graph-editor-head p {
  margin: 2px 0 0;
  overflow-wrap: anywhere;
}



.task-graph-editor-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 0;
  min-height: 0;
  height: 100%;
  padding: 0 12px 12px;
  overflow: hidden;
}

.task-graph-inspector {
  display: grid;
  align-content: start;
  gap: 12px;
  min-width: 0;
}

.task-graph-settings,
.task-graph-node-card {
  display: grid;
  align-content: start;
  gap: 9px;
  min-width: 0;
  padding: 10px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface-soft);
}

.task-graph-node-readonly input,
.task-graph-node-readonly select,
.task-graph-node-readonly textarea,
.task-graph-node-readonly button {
  pointer-events: none;
  opacity: 0.7;
}

.task-graph-variable-list button {
  width: 100%;
  min-width: 0;
  min-height: 28px;
  padding: 5px 7px;
  border: 1px solid var(--task-graph-accent-border-light);
  border-radius: 8px;
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 11px;
  font-weight: 760;
  overflow-wrap: anywhere;
}

.task-graph-variable-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.task-graph-variable-list button {
  width: auto;
  cursor: pointer;
}

.task-graph-palette,
.task-graph-validation-card {
  display: grid;
  gap: 9px;
  padding: 10px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-surface) 92%, transparent);
  backdrop-filter: blur(6px);
  box-shadow: 0 10px 24px var(--task-graph-accent-shadow);
}

.task-graph-palette h4,
.task-graph-settings h4,
.task-graph-node-card h4 {
  margin: 0;
}

.task-graph-palette button {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 34px;
  padding: 0 9px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-strong);
  cursor: grab;
  font-size: 12px;
  font-weight: 760;
}

.task-graph-palette button span {
  width: 9px;
  height: 9px;
  border-radius: 999px;
}

.task-graph-editor-main {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  gap: 8px;
  position: relative;
  min-width: 0;
  min-height: 0;
  height: 100%;
  overflow: hidden;
}

.task-graph-editor-summary-strip {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  min-height: 44px;
  padding: 7px 0 1px;
  overflow-x: auto;
  scrollbar-width: none;
}

.task-graph-editor-summary-strip::-webkit-scrollbar {
  display: none;
}

.task-graph-editor-summary-chip,
.task-graph-editor-summary-close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-width: 0;
  min-height: 34px;
  padding: 0 10px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-muted);
  cursor: pointer;
  font: inherit;
}

.task-graph-editor-summary-chip {
  flex: 0 0 auto;
  justify-content: start;
  min-width: 154px;
}

.task-graph-editor-summary-chip.active {
  border-color: color-mix(in srgb, var(--bb-accent) 36%, var(--bb-hairline));
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
}

.task-graph-editor-summary-chip svg,
.task-graph-editor-summary-close svg {
  flex: 0 0 auto;
  width: 15px;
  height: 15px;
}

.task-graph-editor-summary-chip > span {
  display: grid;
  min-width: 0;
  gap: 1px;
  text-align: left;
}

.task-graph-editor-summary-chip strong,
.task-graph-editor-summary-close span {
  overflow: hidden;
  color: currentColor;
  font-size: 12px;
  font-weight: 820;
  line-height: 1;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-graph-editor-summary-chip small {
  overflow: hidden;
  color: var(--bb-text-muted);
  font-size: 10px;
  font-weight: 760;
  line-height: 1.1;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-graph-editor-summary-chip.active small {
  color: color-mix(in srgb, var(--bb-accent) 68%, var(--bb-text-muted));
}

.task-graph-editor-summary-close {
  margin-left: auto;
}

.task-graph-editor-config-panel {
  position: absolute;
  top: 12px;
  right: 12px;
  bottom: 12px;
  left: auto;
  z-index: 4;
  width: min(520px, calc(100% - 250px));
  max-height: none;
  overflow: auto;
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-surface) 96%, transparent);
  box-shadow: 0 18px 42px rgba(15, 23, 42, 0.16);
  backdrop-filter: blur(8px);
}

.task-graph-config-panel-enter-active,
.task-graph-config-panel-leave-active {
  transition: opacity 140ms ease, transform 140ms ease;
}

.task-graph-config-panel-enter-from,
.task-graph-config-panel-leave-to {
  opacity: 0;
  transform: translateX(8px);
}

.task-graph-editor-canvas-wrap {
  position: relative;
  min-height: 0;
  height: 100%;
  overflow: hidden;
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-editor-canvas-wrap:focus {
  outline: none;
}

.task-graph-editor-canvas-wrap:focus-visible {
  outline: 2px solid var(--bb-focus, var(--bb-accent));
  outline-offset: 2px;
}

.task-graph-editor-canvas {
  min-height: 0;
  height: 100%;
}

.task-graph-editor-canvas :deep(svg) {
  width: 100%;
  min-width: 0;
  height: 100%;
  min-height: 0;
}

.task-graph-editor-canvas :deep(.graph-edge.invalid > path) {
  stroke: var(--bb-error) !important;
  stroke-dasharray: 8 6;
}

.task-graph-editor-canvas :deep(.graph-edge.editor-selected > path) {
  stroke-width: 4;
  filter: drop-shadow(0 8px 12px var(--task-graph-accent-border-light));
}

.task-graph-editor-edge-label {
  fill: var(--bb-text-muted);
  font-size: 12px;
  font-weight: 760;
  paint-order: stroke;
  stroke: var(--bb-surface);
  stroke-width: 4px;
  text-anchor: middle;
}

.task-graph-editor-overlay-stack {
  position: absolute;
  top: 12px;
  left: 12px;
  z-index: 2;
  display: grid;
  gap: 10px;
  width: 214px;
  pointer-events: none;
}

.task-graph-editor-overlay {
  pointer-events: auto;
}

.task-graph-editor-node-overlay {
  position: absolute;
  right: 12px;
  top: 12px;
  bottom: 12px;
  z-index: 2;
  width: 336px;
  max-height: none;
  overflow: auto;
  box-shadow: 0 16px 36px rgba(15, 23, 42, 0.14);
  backdrop-filter: blur(8px);
}

.task-graph-node-card header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.task-graph-node-card header button,
.task-graph-rule-row button,
.task-graph-inline-add {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-height: 28px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-muted);
  cursor: pointer;
}

.task-graph-node-card header button svg,
.task-graph-rule-row button svg,
.task-graph-inline-add svg {
  width: 14px;
  height: 14px;
}

.task-graph-node-delete-button {
  padding: 0 8px;
}

.task-graph-node-delete-button span {
  font-size: 11px;
  font-weight: 760;
}

.task-graph-segmented {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 4px;
  padding: 3px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-segmented button,
.task-graph-mini-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-height: 28px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--bb-text-muted);
  cursor: pointer;
  font-size: 12px;
  font-weight: 760;
}

.task-graph-segmented button.active {
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
}

.task-graph-agent-summary,
.task-graph-inline-list,
.task-graph-mcp-editor,
.task-graph-advanced {
  display: grid;
  gap: 7px;
  min-width: 0;
}

.task-graph-agent-summary,
.task-graph-mcp-editor {
  padding: 8px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-agent-summary strong,
.task-graph-inline-list-head strong {
  color: var(--bb-text-strong);
  font-size: 12px;
}

.task-graph-agent-summary span,
.task-graph-muted-note {
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 11px;
  overflow-wrap: anywhere;
}

.task-graph-inline-list-head,
.task-graph-inline-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 6px;
}

.task-graph-inline-row:has(input + input) {
  grid-template-columns: minmax(0, 0.75fr) minmax(0, 1fr) auto;
}

.task-graph-mcp-editor > .task-graph-inline-row {
  grid-template-columns: minmax(0, 1fr) minmax(72px, 0.45fr) auto;
}

.task-graph-mini-button {
  width: 30px;
  border: 1px solid var(--bb-border-warm-medium);
  background: var(--bb-surface-soft);
}

.task-graph-mini-button svg {
  width: 14px;
  height: 14px;
}

.task-graph-advanced summary {
  color: var(--bb-text-muted);
  cursor: pointer;
  font-size: 12px;
  font-weight: 760;
}

.task-graph-settings label,
.task-graph-node-card label {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.task-graph-settings label span,
.task-graph-node-card label span {
  color: var(--bb-text-muted);
  font-size: 11px;
  font-weight: 760;
}

.task-graph-settings input,
.task-graph-settings select,
.task-graph-settings textarea,
.task-graph-node-card input,
.task-graph-node-card select,
.task-graph-node-card textarea {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  min-height: 32px;
  padding: 7px 8px;
  border: 1px solid var(--bb-border-warm-medium-strong);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-strong);
  font: inherit;
  font-size: 12px;
}

.task-graph-settings textarea,
.task-graph-node-card textarea {
  min-height: 80px;
  resize: vertical;
}

.task-graph-settings {
  gap: 8px;
}

.task-graph-settings label {
  grid-template-columns: 76px minmax(0, 1fr);
  align-items: start;
  gap: 8px;
}

.task-graph-settings label span {
  padding-top: 8px;
}

.task-graph-settings textarea {
  min-height: 64px;
  line-height: 1.4;
}

.task-graph-rule-list {
  display: grid;
  gap: 8px;
}

.task-graph-rule-row {
  display: grid;
  grid-template-columns: minmax(0, 0.8fr) minmax(0, 0.9fr) minmax(0, 0.8fr);
  gap: 6px;
}

.task-graph-rule-row input:nth-of-type(3),
.task-graph-rule-row input:nth-of-type(4),
.task-graph-rule-row button {
  grid-column: span 1;
}

.task-graph-inline-add {
  justify-self: start;
  padding: 0 9px;
  font-size: 12px;
  font-weight: 760;
}

.task-graph-validation-state {
  display: flex;
  align-items: center;
  gap: 7px;
  min-height: 32px;
  padding: 0 9px;
  border-radius: 8px;
  font-size: 12px;
}

.task-graph-validation-state svg {
  width: 15px;
  height: 15px;
}

.task-graph-validation-state.passed {
  background: color-mix(in srgb, var(--bb-success) 12%, var(--bb-surface));
  color: var(--bb-success);
}

.task-graph-validation-state.failed {
  background: color-mix(in srgb, var(--bb-error) 12%, var(--bb-surface));
  color: var(--bb-error);
}

.task-graph-error-list {
  display: grid;
  gap: 5px;
  margin: 0;
  padding: 0;
  list-style: none;
  color: var(--bb-error);
  font-size: 12px;
  line-height: 1.35;
}

.task-graph-settings p {
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 12px;
  line-height: 1.45;
}

@media (max-width: 1180px) {
  .task-graph-editor-grid {
    grid-template-columns: 1fr;
  }

  .task-graph-editor-config-panel,
  .task-graph-editor-node-overlay {
    width: min(520px, calc(100% - 24px));
  }

  .task-graph-editor-config-panel {
    left: 12px;
    right: 12px;
    max-height: min(420px, calc(100% - 76px));
  }

  .task-graph-editor-node-overlay {
    top: auto;
    left: 12px;
    max-height: min(420px, calc(100% - 24px));
  }
}

@media (max-width: 720px) {
  .task-graph-settings label {
    grid-template-columns: 1fr;
    gap: 5px;
  }

  .task-graph-settings label span {
    padding-top: 0;
  }
}

.task-graph-close-confirm-backdrop {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: grid;
  place-items: center;
  background: var(--task-graph-backdrop-bg);
}

.task-graph-close-confirm-dialog {
  display: grid;
  gap: 12px;
  width: min(480px, 90vw);
  padding: 20px;
  border-radius: 12px;
  background: var(--bb-surface);
  box-shadow: 0 8px 32px var(--task-graph-dialog-shadow);
}

.task-graph-close-confirm-dialog header {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--bb-warning);
}

.task-graph-close-confirm-dialog header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 700;
}

.task-graph-close-confirm-dialog header svg {
  width: 20px;
  height: 20px;
  flex-shrink: 0;
}

.task-graph-close-confirm-dialog > p {
  margin: 0;
  font-size: 13px;
  color: var(--bb-text);
  line-height: 1.5;
}

.task-graph-close-confirm-errors {
  display: grid;
  gap: 4px;
  margin: 0;
  padding: 10px 12px;
  list-style: none;
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-error) 8%, var(--bb-surface));
  font-size: 12px;
  color: var(--bb-error);
  max-height: 140px;
  overflow-y: auto;
}

.task-graph-close-confirm-errors strong {
  font-weight: 600;
}

.task-graph-close-confirm-hint {
  font-size: 12px;
  color: var(--bb-text-muted);
  margin: 0;
}

.task-graph-close-confirm-dialog footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding-top: 4px;
}

.task-graph-close-confirm-discard {
  opacity: 0.7;
}
</style>

<style>
/* Notification is teleported to body by TDesign, so this style must be global. */
.bb-task-graph-notification .t-notification__content {
  white-space: pre-line;
  word-break: break-word;
  max-height: none;
  display: block;
  -webkit-line-clamp: unset;
  -webkit-box-orient: unset;
}

.bb-task-graph-notification.t-notification {
  border: 1px solid var(--bb-hairline);
  border-radius: var(--bb-radius);
  box-shadow: var(--bb-shadow-popover);
  font-family: 'Noto Sans SC', system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
}
</style>
