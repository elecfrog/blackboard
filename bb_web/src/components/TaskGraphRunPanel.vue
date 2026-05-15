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
import TaskGraphNodeShape from '@/components/task-graph/TaskGraphNodeShape.vue'
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
  watchTaskGraphRun,
  nodeToCanvasPins,
  nodeHeightForPins,
  type TaskGraphEdge,
  type TaskGraphNode,
  type TaskGraphOutputArtifact,
  type TaskGraphRunDetail,
  type TaskGraphRunNode,
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
const selectedNodeId = ref('')
const cancelling = ref(false)
const agentSessionEvents = ref<AgentEvent[]>([])
const agentSessionError = ref('')
let stopRunEvents: (() => void) | null = null
let stopAgentSessionEvents: (() => void) | null = null
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

const canvasNodes = computed<GraphCanvasNode[]>(() =>
  (run.value?.graph_snapshot.nodes ?? []).map((node) => {
    const state = runNodeById.value.get(node.id)
    const status = state?.status ?? 'idle'
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
        ...(run.value?.cursor.includes(node.id) ? ['run-cursor'] : []),
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
    color: activeEdgeIds.value.has(edge.id) ? '#0f766e' : undefined,
    classes: [
      edge.from_pin ? `task-edge-source-${edge.from_pin.replace(/[^a-z0-9-]/gi, '-')}` : '',
      edge.to_pin ? `task-edge-target-${edge.to_pin.replace(/[^a-z0-9-]/gi, '-')}` : '',
      ...(activeEdgeIds.value.has(edge.id) ? ['run-edge-active'] : []),
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

const currentNodeSummary = computed(() => {
  const item = run.value
  if (!item) return ''
  const ids = item.cursor.length > 0
    ? item.cursor
    : item.nodes.filter((node) => node.status === 'running').map((node) => node.node_id)
  return ids
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
})

async function loadRun() {
  loading.value = true
  error.value = ''
  try {
    const result = await readTaskGraphRun(props.project, props.runId)
    run.value = result.run
    selectDefaultNode(true)
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
      selectDefaultNode(false)
    },
    (message) => {
      if (run.value && ['pending', 'running', 'paused'].includes(run.value.status)) {
        error.value = message
      }
    },
  )
}

function stopRunStream() {
  stopRunEvents?.()
  stopRunEvents = null
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
  selectedNodeId.value =
    item.nodes.find((node) => node.status === 'failed')?.node_id
    ?? item.paused?.node_id
    ?? item.cursor[0]
    ?? item.nodes.find((node) => node.status === 'running')?.node_id
    ?? item.nodes[0]?.node_id
    ?? ''
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
  return status === 'pending' || status === 'running' || status === 'paused'
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
          <span v-if="run.last_checkpoint_id">{{ t('taskGraphCheckpoint') }} {{ run.last_checkpoint_id }}</span>
          <span v-if="currentNodeSummary" class="task-graph-run-current-node">
            {{ t('taskGraphCurrentNode') }} {{ currentNodeSummary }}
          </span>
        </div>
      </div>
      <div>
        <button
          v-if="run?.parent_run_id"
          type="button"
          class="bb-top-action-button"
          @click="openChildRun(run!.parent_run_id!)"
        >
          <ArrowLeft class="bb-top-action-svg" aria-hidden="true" />
          <span>{{ t('taskGraphBackToParentRun') }}</span>
        </button>
        <button
          v-if="isRunActive"
          type="button"
          class="bb-top-action-button bb-cancel-button"
          :disabled="cancelling"
          @click="cancelRun"
        >
          <StopCircle class="bb-top-action-svg" aria-hidden="true" />
          <span>{{ cancelling ? t('taskGraphCancelling') : t('taskGraphCancelRun') }}</span>
        </button>
        <button type="button" class="bb-top-action-button" :disabled="loading" @click="loadRun">
          <RefreshCw class="bb-top-action-svg" aria-hidden="true" />
          <span>{{ t('refresh') }}</span>
        </button>
        <button type="button" class="bb-top-action-button" @click="emit('close')">
          <X class="bb-top-action-svg" aria-hidden="true" />
          <span>{{ t('close') }}</span>
        </button>
      </div>
    </header>

    <div v-if="loading" class="bb-state-panel">{{ t('loading') }}</div>
    <div v-else-if="error" class="bb-state-panel bb-error">{{ error }}</div>

    <template v-else-if="run">
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
            disabled
          >
            {{ action.label }}
          </button>
        </div>
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
          <header>
            <FileText aria-hidden="true" />
            <div>
              <h4>{{ selectedGraphNode?.label ?? t('taskGraphNodeDetail') }}</h4>
              <span>{{ selectedNodeId || '-' }} · {{ nodeTypeLabel(selectedGraphNode) }}</span>
            </div>
          </header>

          <dl v-if="selectedRunNode" class="task-graph-node-state">
            <div>
              <dt>{{ t('status') }}</dt>
              <dd>{{ selectedRunNode.status }}</dd>
            </div>
            <div>
              <dt>{{ t('taskGraphDuration') }}</dt>
              <dd>{{ formatDuration(selectedRunNode.duration_ms) }}</dd>
            </div>
            <div>
              <dt>{{ t('taskGraphStarted') }}</dt>
              <dd>{{ formatDate(selectedRunNode.started_at) }}</dd>
            </div>
            <div>
              <dt>{{ t('taskGraphCompleted') }}</dt>
              <dd>{{ formatDate(selectedRunNode.completed_at) }}</dd>
            </div>
            <div v-if="selectedRunNode.runtime">
              <dt>{{ t('taskGraphRuntime') }}</dt>
              <dd>{{ selectedRunNode.runtime }}</dd>
            </div>
            <div v-if="selectedRunNode.agent">
              <dt>{{ t('taskGraphAgent') }}</dt>
              <dd>{{ selectedRunNode.agent }}</dd>
            </div>
            <div v-if="selectedRunNode.model">
              <dt>{{ t('taskGraphModel') }}</dt>
              <dd>{{ selectedRunNode.model }}</dd>
            </div>
          </dl>

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

.task-graph-run-head > div:last-child {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
  flex: 0 0 auto;
  min-width: 0;
}

.task-graph-run-head .bb-top-action-button {
  height: 30px;
  min-height: 30px;
  padding: 0 9px;
  gap: 6px;
  border-radius: 6px;
  font-size: 11px;
  line-height: 1;
}

.task-graph-run-head .bb-top-action-svg {
  flex-basis: 14px;
  width: 14px;
  height: 14px;
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

.task-graph-node-state {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px;
  margin: 0;
}

.task-graph-node-state > div {
  min-width: 0;
  padding: 9px;
  border: 1px solid color-mix(in srgb, var(--bb-hairline) 75%, transparent);
  border-radius: 8px;
  background: var(--bb-surface-soft);
}

.task-graph-node-state dd {
  margin: 3px 0 0;
  overflow-wrap: anywhere;
  color: var(--bb-text-strong);
  font-size: 12px;
  font-weight: 760;
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

.task-graph-run-canvas :deep(.graph-node.run-cursor > rect:first-child) {
  stroke-dasharray: 8 5;
}

.task-graph-run-canvas :deep(.graph-edge.run-edge-active > path) {
  stroke-width: 4;
  filter: drop-shadow(0 8px 12px color-mix(in srgb, var(--bb-accent) 18%, transparent));
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

.task-graph-node-state {
  grid-template-columns: repeat(2, minmax(0, 1fr));
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
  background: var(--bb-text-strong);
  color: var(--bb-surface);
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

.bb-cancel-button {
  border-color: var(--bb-error) !important;
  background: var(--bb-surface) !important;
  color: var(--bb-error) !important;
}

.bb-cancel-button .bb-top-action-svg {
  color: var(--bb-error) !important;
}

.bb-cancel-button:hover:not(:disabled) {
  background: var(--bb-md-error-bg) !important;
}

.bb-cancel-button:disabled {
  opacity: 0.6;
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
