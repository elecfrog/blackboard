<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  AlertCircle,
  ArrowLeft,
  FileText,
  PauseCircle,
  RefreshCw,
  StopCircle,
  X,
} from 'lucide-vue-next'
import GraphCanvas, { type GraphCanvasEdge, type GraphCanvasNode } from '@/components/GraphCanvas.vue'
import { BbActionGroup, BbButton, BbInfoGrid, BbInfoItem } from '@/components/common'
import TaskGraphNodeShape from '@/components/task-graph/TaskGraphNodeShape.vue'
import TaskGraphMutationTimeline from '@/components/task-graph/TaskGraphMutationTimeline.vue'
import TaskGraphToolLifecycleTimeline from '@/components/task-graph/TaskGraphToolLifecycleTimeline.vue'
import {
  taskGraphNodeMetaLabel,
  taskGraphNodeVisualForNode,
} from '@/components/task-graph/taskGraphNodeVisuals'
import {
  readAgentSessionEvents,
  watchAgentSessionEvents,
  type AgentEvent,
  type TokenUsage,
} from '@/data/agentSessions'
import {
  cancelTaskGraphRun,
  readTaskGraphRun,
  readTaskGraphRunCheckpoints,
  readTaskGraphRunEventLog,
  resumeTaskGraphGate,
  watchTaskGraphRun,
  nodeToCanvasPins,
  nodeHeightForPins,
  type GraphMutationBatchResult,
  type TaskGraphEdge,
  type TaskGraphNode,
  type TaskGraphOutputArtifact,
  type TaskGraphRunDetail,
  type TaskGraphRunEvent,
  type TaskGraphRunNode,
  type TaskGraphSuperstepCheckpoint,
  type TopologyMutationEventPayload,
} from '@/data/taskGraphs'
import { t } from '@/i18n'

const props = defineProps<{
  project: string
  runId: string
}>()

const emit = defineEmits<{
  close: []
  'navigate-run': [runId: string]
}>()

const run = ref<TaskGraphRunDetail | null>(null)
const loading = ref(true)
const error = ref('')
const streamError = ref('')
const selectedNodeId = ref('')
const cancelling = ref(false)
const runEvents = ref<TaskGraphRunEvent[]>([])
const checkpoints = ref<TaskGraphSuperstepCheckpoint[]>([])
const runArtifactsError = ref('')
const resumingActionId = ref('')
const resumeError = ref('')
const agentSessionEvents = ref<AgentEvent[]>([])
const agentSessionError = ref('')
let stopRunEvents: (() => void) | null = null
let stopAgentSessionEvents: (() => void) | null = null
let runArtifactsTimer: number | null = null
const runNodeFooterHeight = 26

const nodeById = computed(() =>
  new Map((run.value?.graph_snapshot.nodes ?? []).map((node) => [node.id, node])),
)

const runNodeById = computed(() =>
  new Map((run.value?.nodes ?? []).map((node) => [node.node_id, node])),
)

const activeEdgeIds = computed(() => {
  const ids = new Set<string>()
  for (const decision of run.value?.context.branch_decisions ?? []) {
    if (decision.selected_edge_id) ids.add(decision.selected_edge_id)
  }
  return ids
})

const latestCheckpoint = computed(() =>
  [...checkpoints.value].sort((left, right) => {
    const leftTime = Date.parse(left.completed_at ?? left.created_at)
    const rightTime = Date.parse(right.completed_at ?? right.created_at)
    return right.superstep - left.superstep || rightTime - leftTime
  })[0] ?? null,
)

const revisionLabel = computed(() => {
  const checkpoint = latestCheckpoint.value
  if (checkpoint && checkpoint.graph_revision_before !== checkpoint.graph_revision_after) {
    return `${checkpoint.graph_revision_before} -> ${checkpoint.graph_revision_after}`
  }
  return String(run.value?.current_graph_revision ?? 0)
})

const latestAppliedMutationEdgeIds = computed(() => {
  const ids = new Set<string>()
  const event = [...runEvents.value]
    .filter((item) => item.kind === 'topology_mutation')
    .sort((left, right) => right.superstep - left.superstep || Date.parse(right.created_at) - Date.parse(left.created_at))
    .find((item) => topologyMutationPayload(item.payload)?.result.status === 'applied')
  const result = topologyMutationPayload(event?.payload)?.result
  if (result?.status === 'applied') {
    for (const edgeId of result.summary.added_edges) ids.add(edgeId)
  }
  return ids
})

const canvasNodes = computed<GraphCanvasNode[]>(() =>
  (run.value?.graph_snapshot.nodes ?? []).map((node) => {
    const state = runNodeById.value.get(node.id)
    const status = visualRunNodeStatus(node.id, state)
    return {
      id: node.id,
      x: node.position?.x ?? 80,
      y: node.position?.y ?? 120,
      width: 230,
      height: Math.max(nodeHeightForPins(node) + runNodeFooterHeight, 64),
      status,
      kind: node.type,
      color: taskGraphNodeVisualForNode(node).color,
      label: node.label,
      meta: taskGraphNodeMetaLabel(node, run.value?.graph_snapshot.inputs ?? []),
      title: node.label,
      pins: nodeToCanvasPins(node),
      classes: [
        `task-node-${node.type}`,
        `run-node-${status}`,
        ...(selectedNodeId.value === node.id ? ['run-selected'] : []),
        ...(run.value?.active_nodes.includes(node.id) ? ['run-active-node'] : []),
      ],
    }
  }),
)

const canvasEdges = computed<GraphCanvasEdge[]>(() =>
  (run.value?.graph_snapshot.edges ?? []).map((edge) => ({
    id: edge.id,
    from: edge.from,
    to: edge.to,
    label: edgeDisplayLabel(edge),
    sourceHandle: edge.from_pin ?? edge.source_handle,
    targetHandle: edge.to_pin ?? edge.target_handle,
    removable: false,
    dashed: edge.kind === 'data',
    color: activeEdgeIds.value.has(edge.id)
      ? '#0f766e'
      : latestAppliedMutationEdgeIds.value.has(edge.id)
        ? '#2563eb'
        : undefined,
    classes: [
      edge.from_pin ? `task-edge-source-${edge.from_pin.replace(/[^a-z0-9-]/gi, '-')}` : '',
      edge.to_pin ? `task-edge-target-${edge.to_pin.replace(/[^a-z0-9-]/gi, '-')}` : '',
      ...(activeEdgeIds.value.has(edge.id) ? ['run-edge-active'] : []),
      ...(latestAppliedMutationEdgeIds.value.has(edge.id) ? ['run-edge-mutated'] : []),
    ].filter((item): item is string => Boolean(item)),
  })),
)

const canvasSize = computed(() => {
  const nodes = canvasNodes.value
  if (nodes.length === 0) return { width: 1100, height: 620 }
  return {
    width: Math.max(1100, Math.max(...nodes.map((node) => node.x + (node.width ?? 230))) + 140),
    height: Math.max(620, Math.max(...nodes.map((node) => node.y + (node.height ?? 92))) + 140),
  }
})

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

const selectedGraphNode = computed<TaskGraphNode | null>(() =>
  selectedNodeId.value ? nodeById.value.get(selectedNodeId.value) ?? null : null,
)

const selectedRunNode = computed<TaskGraphRunNode | null>(() =>
  selectedNodeId.value ? runNodeById.value.get(selectedNodeId.value) ?? null : null,
)

const selectedOutput = computed(() =>
  selectedNodeId.value ? run.value?.context.node_outputs[selectedNodeId.value] : undefined,
)

const selectedAgentSessionId = computed(() => selectedRunNode.value?.agent_session_id ?? '')

const elapsedLabel = computed(() => {
  const item = run.value
  if (!item) return '-'
  const start = new Date(item.started_at ?? item.created_at)
  const end = new Date(item.completed_at ?? item.updated_at)
  if (Number.isNaN(start.getTime()) || Number.isNaN(end.getTime())) return '-'
  return formatDuration(Math.max(0, end.getTime() - start.getTime()))
})

const activeNodeSummary = computed(() => {
  const item = run.value
  if (!item) return ''
  return item.active_nodes
    .map((id) => {
      const graphNode = nodeById.value.get(id)
      return graphNode ? `${graphNode.label} · ${nodeTypeLabel(graphNode)}` : ''
    })
    .filter(Boolean)
    .join(' / ')
})

watch(
  () => [props.project, props.runId],
  () => {
    void loadRun()
    startRunStream()
  },
)

watch(
  () => [props.project, selectedAgentSessionId.value],
  () => {
    startAgentSessionStream()
  },
)

onMounted(() => {
  void loadRun()
  startRunStream()
})

onBeforeUnmount(() => {
  stopRunStream()
  stopAgentSessionStream()
  clearRunArtifactsTimer()
})

async function loadRun() {
  loading.value = true
  error.value = ''
  streamError.value = ''
  try {
    const result = await readTaskGraphRun(props.project, props.runId)
    run.value = result.run
    selectDefaultNode(true)
    await refreshRunArtifacts()
  } catch (err) {
    run.value = null
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

function startRunStream() {
  stopRunStream()
  stopRunEvents = watchTaskGraphRun(
    props.project,
    props.runId,
    (nextRun) => {
      run.value = nextRun
      loading.value = false
      streamError.value = ''
      selectDefaultNode(false)
      scheduleRunArtifactsRefresh(['succeeded', 'failed', 'cancelled'].includes(nextRun.status) ? 0 : 500)
    },
    (message) => {
      if (run.value && ['queued', 'pending', 'running', 'paused'].includes(run.value.status)) {
        streamError.value = message
      }
    },
  )
}

function stopRunStream() {
  stopRunEvents?.()
  stopRunEvents = null
}

function clearRunArtifactsTimer() {
  if (!runArtifactsTimer) return
  window.clearTimeout(runArtifactsTimer)
  runArtifactsTimer = null
}

function scheduleRunArtifactsRefresh(delayMs: number) {
  clearRunArtifactsTimer()
  runArtifactsTimer = window.setTimeout(() => {
    runArtifactsTimer = null
    void refreshRunArtifacts()
  }, delayMs)
}

async function refreshRunArtifacts() {
  runArtifactsError.value = ''
  try {
    const [events, nextCheckpoints] = await Promise.all([
      readTaskGraphRunEventLog(props.project, props.runId),
      readTaskGraphRunCheckpoints(props.project, props.runId),
    ])
    runEvents.value = events
    checkpoints.value = nextCheckpoints
  } catch (err) {
    runArtifactsError.value = err instanceof Error ? err.message : String(err)
  }
}

async function startAgentSessionStream() {
  stopAgentSessionStream()
  agentSessionEvents.value = []
  agentSessionError.value = ''
  const sessionId = selectedAgentSessionId.value
  if (!sessionId) return

  try {
    const initial = await readAgentSessionEvents(props.project, sessionId)
    agentSessionEvents.value = mergeAgentEvents([], initial)
  } catch (err) {
    agentSessionError.value = err instanceof Error ? err.message : String(err)
  }

  const lastSeq = agentSessionEvents.value[agentSessionEvents.value.length - 1]?.seq ?? 0
  stopAgentSessionEvents = watchAgentSessionEvents(
    props.project,
    sessionId,
    (events) => {
      agentSessionEvents.value = mergeAgentEvents(agentSessionEvents.value, events)
    },
    (message) => {
      agentSessionError.value = message
    },
    lastSeq,
  )
}

function stopAgentSessionStream() {
  stopAgentSessionEvents?.()
  stopAgentSessionEvents = null
}

function mergeAgentEvents(existing: AgentEvent[], incoming: AgentEvent[]) {
  const bySeq = new Map(existing.map((event) => [event.seq, event]))
  for (const event of incoming) bySeq.set(event.seq, event)
  return [...bySeq.values()].sort((a, b) => a.seq - b.seq)
}

function selectDefaultNode(resetSelection = false) {
  const item = run.value
  if (!item) {
    selectedNodeId.value = ''
    return
  }
  if (!resetSelection && selectedNodeId.value && nodeById.value.has(selectedNodeId.value)) return
  const visibleNodeIds = new Set(item.graph_snapshot.nodes.map((node) => node.id))
  const firstByStatus = (statuses: TaskGraphRunNode['status'][]) =>
    item.nodes.find((node) => statuses.includes(node.status) && visibleNodeIds.has(node.node_id))?.node_id
  selectedNodeId.value =
    firstByStatus(['failed'])
    ?? (item.paused?.node_id && visibleNodeIds.has(item.paused.node_id) ? item.paused.node_id : undefined)
    ?? item.active_nodes.find((id) => visibleNodeIds.has(id))
    ?? firstByStatus(['running', 'queued'])
    ?? item.graph_snapshot.nodes[0]?.id
    ?? ''
}

function visualRunNodeStatus(nodeId: string, state?: TaskGraphRunNode): TaskGraphRunNode['status'] {
  if (state?.status === 'failed') return 'failed'
  if (state?.status === 'paused' || run.value?.paused?.node_id === nodeId) return 'paused'
  if (state?.status === 'running' || state?.status === 'queued') return state.status
  if (run.value?.active_nodes.includes(nodeId)) return 'queued'
  if (state?.status === 'succeeded') return 'succeeded'
  return state?.status ?? 'idle'
}

function topologyMutationPayload(value: unknown): TopologyMutationEventPayload | null {
  if (!value || typeof value !== 'object') return null
  const payload = value as Partial<TopologyMutationEventPayload>
  if (typeof payload.batch_id !== 'string') return null
  if (typeof payload.graph_revision_before !== 'number' || typeof payload.graph_revision_after !== 'number') return null
  if (!payload.result || typeof payload.result !== 'object') return null
  const result = payload.result as Partial<GraphMutationBatchResult>
  if (result.status !== 'applied' && result.status !== 'rejected') return null
  return payload as TopologyMutationEventPayload
}

function openNode(id: string) {
  selectedNodeId.value = id
}

function openChildRun(childRunId: string) {
  // Navigate to child run by emitting event (parent component handles navigation)
  emit('navigate-run', childRunId)
}

const isRunActive = computed(() => {
  const status = run.value?.status
  return status === 'queued' || status === 'pending' || status === 'running' || status === 'paused'
})

async function cancelRun() {
  if (!run.value || cancelling.value) return
  cancelling.value = true
  try {
    await cancelTaskGraphRun(props.project, props.runId)
    await loadRun()
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    cancelling.value = false
  }
}

async function resumeGate(actionId: string) {
  const item = run.value
  if (!item?.paused || resumingActionId.value) return
  resumingActionId.value = actionId
  resumeError.value = ''
  try {
    await resumeTaskGraphGate(props.project, item.id, item.paused.node_id, actionId)
    await loadRun()
    scheduleRunArtifactsRefresh(0)
  } catch (err) {
    resumeError.value = err instanceof Error ? err.message : String(err)
  } finally {
    resumingActionId.value = ''
  }
}

function formatDate(value?: string) {
  if (!value) return '-'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  }).format(date)
}

function formatDuration(ms?: number) {
  if (!ms && ms !== 0) return '-'
  if (ms < 1000) return `${ms}ms`
  const seconds = Math.round(ms / 1000)
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  const rest = seconds % 60
  return rest ? `${minutes}m ${rest}s` : `${minutes}m`
}

function outputPreview(value: unknown) {
  if (value === undefined || value === null) return ''
  if (typeof value === 'string') return value
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function agentEventTitle(event: AgentEvent) {
  if (event.type === 'tool_use') return event.tool ? `Tool ${event.tool}` : t('agentSessionToolUse')
  if (event.type === 'tool_result') return event.tool ? `Result ${event.tool}` : t('agentSessionToolResult')
  if (event.type === 'usage_update') return t('agentSessionUsage')
  if (event.type === 'status') return event.status || t('agentSessionStatus')
  if (event.type === 'thinking') return t('agentSessionThinking')
  if (event.type === 'error') return t('agentSessionError')
  if (event.type === 'log') return event.level || t('agentSessionLog')
  return t('agentSessionText')
}

function agentEventBody(event: AgentEvent) {
  if (event.content) return event.content
  if (event.status) return event.status
  if (event.session_id) return event.session_id
  return ''
}

function agentEventInput(event: AgentEvent) {
  return event.input === undefined ? '' : outputPreview(event.input)
}

function agentUsageLabel(usage?: Record<string, TokenUsage>) {
  if (!usage) return ''
  return Object.entries(usage)
    .map(([model, item]) => {
      const total = item.input_tokens + item.output_tokens
      const cache = item.cache_read_tokens + item.cache_write_tokens
      return `${model}: ${total} tokens${cache > 0 ? `, cache ${cache}` : ''}`
    })
    .join(' / ')
}

function artifactLabel(artifact?: TaskGraphOutputArtifact) {
  if (!artifact) return t('taskGraphNoArtifact')
  return `${artifact.content_type} · ${artifact.path}`
}

function nodeTypeLabel(node?: TaskGraphNode | null) {
  return node ? taskGraphNodeVisualForNode(node).label : '-'
}
</script>

<template>
  <section class="task-graph-run">
    <header class="task-graph-run-head">
      <div class="task-graph-run-head-main">
        <div v-if="run" class="task-graph-run-head-meta">
          <span>{{ t('taskGraphStarted') }} {{ formatDate(run.started_at ?? run.created_at) }}</span>
          <span>{{ t('update') }} {{ formatDate(run.updated_at) }}</span>
          <span>{{ t('taskGraphDuration') }} {{ elapsedLabel }}</span>
          <span>{{ t('taskGraphSuperstep') }} {{ run.current_superstep ?? 0 }}</span>
          <span>{{ t('taskGraphRevision') }} {{ revisionLabel }}</span>
          <span v-if="run.last_checkpoint_id">{{ t('taskGraphCheckpoint') }} {{ run.last_checkpoint_id }}</span>
          <span v-if="activeNodeSummary" class="task-graph-run-current-node">
            {{ t('taskGraphActiveNodes') }} {{ activeNodeSummary }}
          </span>
        </div>
      </div>
      <BbActionGroup class="task-graph-run-head-actions" gap="sm">
        <BbButton
          v-if="run?.parent_run_id"
          size="sm"
          variant="secondary"
          @click="openChildRun(run!.parent_run_id!)"
        >
          <template #leading>
            <ArrowLeft />
          </template>
          <span>{{ t('taskGraphBackToParentRun') }}</span>
        </BbButton>
        <BbButton
          v-if="isRunActive"
          size="sm"
          variant="danger"
          :disabled="cancelling"
          @click="cancelRun"
        >
          <template #leading>
            <StopCircle />
          </template>
          <span>{{ cancelling ? t('taskGraphCancelling') : t('taskGraphCancelRun') }}</span>
        </BbButton>
        <BbButton size="sm" variant="secondary" :disabled="loading" @click="loadRun">
          <template #leading>
            <RefreshCw />
          </template>
          <span>{{ t('refresh') }}</span>
        </BbButton>
        <BbButton size="sm" variant="secondary" @click="emit('close')">
          <template #leading>
            <X />
          </template>
          <span>{{ t('close') }}</span>
        </BbButton>
      </BbActionGroup>
    </header>

    <div v-if="loading" class="bb-state-panel">{{ t('loading') }}</div>
    <div v-else-if="error" class="bb-state-panel bb-error">{{ error }}</div>

    <template v-else-if="run">
      <div v-if="streamError" class="bb-state-panel task-graph-run-stream-warning">{{ streamError }}</div>

      <section v-if="run.paused" class="task-graph-run-paused">
        <PauseCircle aria-hidden="true" />
        <div>
          <strong>{{ run.paused.node_id }}</strong>
          <p>{{ run.paused.reason }}</p>
        </div>
        <div>
          <button
            v-for="action in run.paused.actions"
            :key="action.id"
            type="button"
            :disabled="Boolean(resumingActionId)"
            @click="resumeGate(action.id)"
          >
            {{ resumingActionId === action.id ? t('taskGraphResuming') : action.label }}
          </button>
        </div>
        <p v-if="resumeError" class="task-graph-run-resume-error">{{ resumeError }}</p>
      </section>

      <div class="task-graph-run-grid">
        <main class="task-graph-run-canvas-wrap">
          <GraphCanvas
            class="ticket-graph-canvas task-graph-run-canvas"
            readonly
            :fit-padding="48"
            :nodes="canvasNodes"
            :edges="canvasEdges"
            :canvas-size="canvasSize"
            :default-node-width="230"
            :default-node-height="64"
            :focus-id="selectedNodeId"
            @node-select="openNode"
            @node-open="openNode"
          >
            <template #node="{ node, width, height }">
              <TaskGraphNodeShape :node="node" :width="width" :height="height" />
            </template>
            <template #edge-label="{ edge, midpoint }">
              <text
                v-if="edge.label"
                class="task-graph-run-edge-label"
                :x="midpoint.x"
                :y="midpoint.y - 10"
              >
                {{ edge.label }}
              </text>
            </template>
          </GraphCanvas>

        </main>

        <aside class="task-graph-run-drawer">
          <section class="task-graph-run-detail-block task-graph-run-level">
            <TaskGraphMutationTimeline :events="runEvents" :checkpoints="checkpoints" />
            <p v-if="runArtifactsError" class="task-graph-run-artifacts-error">{{ runArtifactsError }}</p>
            <BbInfoGrid class="task-graph-run-level-state" columns="repeat(2, minmax(0, 1fr))">
              <BbInfoItem :label="t('taskGraphRevision')" :value="run.current_graph_revision" />
              <BbInfoItem
                :label="t('taskGraphActiveNodes')"
                :value="run.active_nodes.length > 0 ? run.active_nodes.join(', ') : '-'"
              />
            </BbInfoGrid>
          </section>

          <header>
            <FileText aria-hidden="true" />
            <div>
              <h4>{{ selectedGraphNode?.label ?? t('taskGraphNodeDetail') }}</h4>
              <span>{{ selectedNodeId || '-' }} · {{ nodeTypeLabel(selectedGraphNode) }}</span>
            </div>
          </header>

          <section class="task-graph-run-detail-block">
            <TaskGraphToolLifecycleTimeline :events="runEvents" :selected-node-id="selectedNodeId" />
          </section>

          <section v-if="run" class="task-graph-run-params">
            <h5>{{ t('taskGraphInputs') }}</h5>
            <BbInfoGrid
              v-if="Object.keys(run.context.input ?? {}).length > 0"
              class="task-graph-run-param-grid"
              columns="repeat(2, minmax(0, 1fr))"
            >
              <BbInfoItem v-for="[key, value] in Object.entries(run.context.input)" :key="key" :label="key">
                {{ typeof value === 'object' ? JSON.stringify(value) : String(value) }}
              </BbInfoItem>
            </BbInfoGrid>
            <p v-else class="task-graph-run-params-empty">{{ t('taskGraphInputsEmpty') }}</p>
          </section>

          <section v-if="selectedRunNode?.error" class="task-graph-run-error">
            <AlertCircle aria-hidden="true" />
            <div>
              <strong>{{ selectedRunNode.error.code }}</strong>
              <p>{{ selectedRunNode.error.message }}</p>
            </div>
          </section>

          <section v-if="selectedRunNode?.child_run_id" class="task-graph-run-detail-block">
            <h5>{{ t('taskGraphChildPipeline') }}</h5>
            <p>{{ t('taskGraphChildRun') }} <code>{{ selectedRunNode.child_run_id }}</code></p>
            <button type="button" class="task-graph-inline-link" @click="openChildRun(selectedRunNode.child_run_id!)">
              {{ t('taskGraphViewChildRun') }}
            </button>
          </section>

          <section class="task-graph-run-detail-block">
            <h5>{{ t('taskGraphArtifact') }}</h5>
            <p>{{ artifactLabel(selectedRunNode?.output_artifact) }}</p>
            <pre v-if="selectedRunNode?.output_artifact || selectedOutput">{{ outputPreview(selectedOutput) }}</pre>
          </section>

          <section v-if="selectedRunNode?.agent_session_id" class="task-graph-run-detail-block">
            <h5>{{ t('agentSessionTimeline') }}</h5>
            <p class="agent-session-meta">
              <code>{{ selectedRunNode.agent_session_id }}</code>
              <span v-if="selectedRunNode.agent_session">
                {{ selectedRunNode.agent_session.status }} · {{ selectedRunNode.agent_session.event_count }} events
              </span>
            </p>
            <p v-if="agentSessionError" class="agent-session-error">{{ agentSessionError }}</p>
            <p v-else-if="agentSessionEvents.length === 0" class="agent-session-empty">
              {{ t('agentSessionNoEvents') }}
            </p>
            <div v-else class="agent-session-timeline">
              <article
                v-for="event in agentSessionEvents"
                :key="event.seq"
                class="agent-session-event"
                :class="`agent-session-event-${event.type}`"
              >
                <header>
                  <span>{{ agentEventTitle(event) }}</span>
                  <code>#{{ event.seq }}</code>
                </header>
                <p v-if="agentEventBody(event)">{{ agentEventBody(event) }}</p>
                <pre v-if="agentEventInput(event)">{{ agentEventInput(event) }}</pre>
                <pre v-if="event.output">{{ event.output }}</pre>
                <p v-if="agentUsageLabel(event.usage)" class="agent-session-usage">
                  {{ agentUsageLabel(event.usage) }}
                </p>
              </article>
            </div>
          </section>

          <section v-else class="task-graph-run-detail-block">
            <h5>{{ t('taskGraphLogTail') }}</h5>
            <pre>{{ selectedRunNode?.log_tail || t('taskGraphNoLogTail') }}</pre>
          </section>
        </aside>
      </div>
    </template>
  </section>
</template>

<style scoped>
.task-graph-run {
  --task-graph-run-inset: 14px;
  display: grid;
  align-content: start;
  gap: 12px;
  min-width: 0;
  min-height: 0;
  padding-top: 8px;
}

.task-graph-run-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
  min-height: 32px;
  margin-inline: var(--task-graph-run-inset) var(--task-graph-run-inset);
}

.task-graph-run-head > div:first-child {
  min-width: 0;
}

.task-graph-run-head-actions {
  flex: 0 0 auto;
  min-width: 0;
}

.task-graph-run-head span,
.task-graph-run-head p,
.task-graph-run-drawer span,
.task-graph-run-detail-block p,
.task-graph-run-empty {
  color: var(--bb-text-muted);
  font-size: 12px;
}

.task-graph-run-head h3,
.task-graph-run-drawer h4 {
  margin: 2px 0;
  color: var(--bb-text-strong);
}

.task-graph-run-head p,
.task-graph-run-detail-block p {
  margin: 0;
  overflow-wrap: anywhere;
}

.task-graph-run-head-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 7px;
}

.task-graph-run-head-meta > span {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-height: 24px;
  padding: 0 8px;
  border: 1px solid var(--bb-hairline);
  border-radius: 6px;
  background: var(--bb-surface-soft);
  color: var(--bb-text-muted);
  font-size: 11px;
  font-weight: 760;
}

.task-graph-run-current-node {
  max-width: min(520px, 100%);
  overflow: hidden;
  color: var(--bb-text-strong);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-graph-run-stream-warning {
  margin-inline: var(--task-graph-run-inset);
  border-color: color-mix(in srgb, var(--bb-warning) 24%, transparent);
  background: color-mix(in srgb, var(--bb-warning) 8%, var(--bb-surface));
  color: var(--bb-warning);
}

.task-graph-run-params {
  margin: 0;
}

.task-graph-run-params h5 {
  margin: 0 0 8px;
  font-size: 12px;
  font-weight: 600;
  color: var(--bb-text-2);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.task-graph-run-param-grid {
  --bb-info-grid-gap: 7px;
  --bb-info-item-border: 1px solid color-mix(in srgb, var(--bb-hairline) 72%, transparent);
  --bb-info-label-font-size: 11px;
  --bb-info-label-font-weight: 600;
}

.task-graph-run-params-empty {
  margin: 0;
  color: var(--bb-text-3);
  font-size: 12px;
}

.task-graph-run-paused {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  padding: 10px;
  border: 1px solid color-mix(in srgb, var(--bb-warning) 24%, transparent);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-warning) 12%, var(--bb-surface));
  color: var(--bb-warning);
}

.task-graph-run-paused > svg {
  width: 18px;
  height: 18px;
}

.task-graph-run-paused p {
  margin: 2px 0 0;
  color: var(--bb-warning);
  font-size: 12px;
}

.task-graph-run-resume-error {
  grid-column: 1 / -1;
  color: var(--bb-error) !important;
}

.task-graph-run-paused > div:last-child {
  display: flex;
  gap: 6px;
}

.task-graph-run-paused button {
  min-height: 30px;
  padding: 0 9px;
  border: 1px solid color-mix(in srgb, var(--bb-warning) 20%, transparent);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-warning) 8%, var(--bb-surface));
  color: var(--bb-warning);
  font-size: 12px;
  font-weight: 760;
}

.task-graph-run-paused button:disabled {
  opacity: 0.62;
}

.task-graph-run-grid {
  display: grid;
  grid-template-columns: 1fr;
  align-items: start;
  gap: 12px;
  padding-inline: var(--task-graph-run-inset);
  min-height: 0;
}

.task-graph-run-canvas-wrap,
.task-graph-run-drawer {
  min-width: 0;
}

.task-graph-run-canvas {
  min-height: 430px;
}

.task-graph-run-canvas :deep(svg) {
  width: 100%;
  min-width: 0;
  height: clamp(320px, 48vh, 560px);
}

.task-graph-run-canvas :deep(.graph-node-accent),
.task-graph-run-canvas :deep(.graph-node circle) {
  fill: var(--graph-status-color);
}

.task-graph-run-canvas :deep(.graph-node.run-selected > rect:first-child) {
  stroke: var(--bb-accent);
  stroke-width: 2;
}

.task-graph-run-canvas :deep(.graph-node.run-active-node > rect:first-child) {
  stroke-dasharray: 8 5;
}

.task-graph-run-canvas :deep(.graph-edge.run-edge-active > path) {
  stroke-width: 4;
  filter: drop-shadow(0 8px 12px color-mix(in srgb, var(--bb-accent) 18%, transparent));
}

.task-graph-run-canvas :deep(.graph-edge.run-edge-mutated > path) {
  stroke-width: 4;
  filter: drop-shadow(0 8px 12px color-mix(in srgb, var(--bb-focus) 18%, transparent));
}

.task-graph-run-edge-label {
  fill: var(--bb-text-muted);
  font-size: 12px;
  font-weight: 760;
  paint-order: stroke;
  stroke: var(--bb-surface);
  stroke-width: 4px;
  text-anchor: middle;
}

.task-graph-run-drawer {
  display: grid;
  align-content: start;
  gap: 8px;
  padding: 10px;
  border: 1px solid color-mix(in srgb, var(--bb-hairline) 88%, transparent);
  border-radius: 8px;
  background: var(--bb-surface);
  max-height: min(720px, calc(100vh - 260px));
  overflow: auto;
}

.task-graph-run-drawer > header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.task-graph-run-drawer h4,
.task-graph-run-detail-block h5 {
  margin: 0;
  color: var(--bb-text-strong);
}

.task-graph-run-drawer > header svg {
  width: 16px;
  height: 16px;
  color: var(--bb-accent);
}

.task-graph-run-level {
  padding-bottom: 8px;
  border-bottom: 1px solid color-mix(in srgb, var(--bb-hairline) 72%, transparent);
}

.task-graph-run-level-state {
  --bb-info-grid-gap: 7px;
  --bb-info-item-border: 1px solid color-mix(in srgb, var(--bb-hairline) 72%, transparent);
}

.task-graph-run-artifacts-error {
  margin: 0;
  color: var(--bb-error) !important;
  font-size: 12px;
  overflow-wrap: anywhere;
}

.task-graph-run-error {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 8px;
  padding: 9px;
  border: 1px solid color-mix(in srgb, var(--bb-error) 18%, transparent);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-error) 12%, var(--bb-surface));
  color: var(--bb-error);
}

.task-graph-run-error svg {
  width: 16px;
  height: 16px;
}

.task-graph-run-error p {
  margin: 2px 0 0;
  font-size: 12px;
}

.task-graph-run-detail-block {
  display: grid;
  gap: 6px;
  min-width: 0;
}

.task-graph-run-detail-block ol {
  display: grid;
  gap: 4px;
  margin: 0;
  padding-left: 18px;
  color: var(--bb-text-muted);
  font-size: 12px;
}

.task-graph-run-detail-block pre {
  max-height: 210px;
  margin: 0;
  overflow: auto;
  padding: 9px;
  border: 1px solid color-mix(in srgb, var(--bb-hairline) 88%, transparent);
  border-radius: 8px;
  background: var(--bb-md-code-bg);
  color: var(--bb-md-code-text);
  font-size: 11px;
  line-height: 1.45;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.agent-session-meta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 12px;
}

.agent-session-meta code {
  font-size: 11px;
}

.agent-session-error,
.agent-session-empty {
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 12px;
}

.agent-session-error {
  color: var(--bb-error);
}

.agent-session-timeline {
  display: grid;
  gap: 7px;
}

.agent-session-event {
  display: grid;
  gap: 5px;
  padding: 8px;
  border: 1px solid color-mix(in srgb, var(--bb-hairline) 88%, transparent);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-surface-muted) 66%, var(--bb-surface));
}

.agent-session-event > header {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  color: var(--bb-text-strong);
  font-size: 12px;
  font-weight: 760;
}

.agent-session-event > p {
  margin: 0;
  color: var(--bb-text);
  font-size: 12px;
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.agent-session-event pre {
  max-height: 170px;
}

.agent-session-event-error {
  border-color: color-mix(in srgb, var(--bb-error) 22%, transparent);
  background: color-mix(in srgb, var(--bb-error) 10%, var(--bb-surface));
}

.agent-session-event-tool_use,
.agent-session-event-tool_result {
  border-color: color-mix(in srgb, var(--bb-accent) 18%, transparent);
}

.agent-session-usage {
  color: var(--bb-text-muted) !important;
}

@media (max-width: 1180px) {
  .task-graph-run-grid {
    grid-template-columns: 1fr;
  }

  .task-graph-run-paused {
    grid-template-columns: 1fr;
  }
}
</style>
