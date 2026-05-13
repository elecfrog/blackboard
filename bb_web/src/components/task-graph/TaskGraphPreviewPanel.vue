<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  AlertCircle,
  Eye,
  GitFork,
  Pencil,
  Play,
} from 'lucide-vue-next'
import GraphCanvas, { type GraphCanvasEdge, type GraphCanvasNode } from '@/components/GraphCanvas.vue'
import TaskGraphRunInputsPanel from '@/components/task-graph/TaskGraphRunInputsPanel.vue'
import TaskGraphRunHistory from '@/components/task-graph/TaskGraphRunHistory.vue'
import { t } from '@/i18n'
import {
  nodeToCanvasPins,
  nodeHeightForPins,
  type TaskGraphCatalogItem,
  type TaskGraphDefinition,
  type TaskGraphEdge,
  type TaskGraphNode,
  type TaskGraphRunSummary,
} from '@/data/taskGraphs'

const nodeColors: Record<string, string> = {
  start: '#64748b',
  end: '#059669',
  llm: '#2563eb',
  sub_graph: '#7c3aed',
  human_gate: '#d97706',
  branch: '#9333ea',
  loop: '#0891b2',
}

const props = defineProps<{
  selectedCatalogItem: TaskGraphCatalogItem | null
  selectedGraph: TaskGraphDefinition | null
  selectedRef: { scope: string; id: string } | null
  graphLoading: boolean
  activeRunId: string
  runInputValues: Record<string, unknown>
  runHistory: TaskGraphRunSummary[]
  runHistorySource: 'rest' | 'mock'
  runHistoryLoading: boolean
  actionBusy: string
  mode: string
  project: string
}>()

const emit = defineEmits<{
  'update:run-input': [inputId: string, value: unknown]
  run: []
  edit: []
  open: []
  customize: []
  'open-run': [run: TaskGraphRunSummary]
  'reload-history': []
  'navigate-run': [runId: string]
}>()

const selectedPreviewNodeId = ref('')

const selectedActiveRun = computed(() =>
  props.selectedRef ? props.runHistory.find((run) =>
    run.id === props.activeRunId ||
    (run.graph_ref?.scope === props.selectedRef?.scope && run.graph_ref?.id === props.selectedRef?.id && (run.status === 'running' || run.status === 'pending'))
  ) ?? null : null,
)

const selectedRunHistory = computed(() =>
  props.selectedRef ? props.runHistory.filter((run) =>
    run.graph_ref?.scope === props.selectedRef?.scope && run.graph_ref?.id === props.selectedRef?.id
  ).slice(0, 12) : [],
)

const hasEmbeddedDetailPanel = computed(() =>
  Boolean(props.selectedGraph && (props.mode === 'edit' || props.activeRunId)),
)

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

function loopMeta(node: TaskGraphNode) {
  const config = node.config ?? {}
  const ref = typeof config.max_iterations_ref === 'string' ? config.max_iterations_ref : ''
  const inputId = ref.match(/^\{\{inputs\.([^}]+)\}\}\}$/)?.[1]
  const inputDefault = inputId ? props.selectedGraph?.inputs?.find((input) => input.id === inputId)?.default : undefined
  const value = inputDefault ?? config.max_iterations ?? ref
  return value === undefined || value === '' ? t('taskGraphLoopsEmpty') : t('taskGraphLoops', { count: String(value) })
}

const previewNodes = computed<GraphCanvasNode[]>(() =>
  (props.selectedGraph?.nodes ?? []).map((node) => ({
    id: node.id,
    x: node.position?.x ?? 80,
    y: node.position?.y ?? 120,
    width: 220,
    height: nodeHeightForPins(node),
    status: node.type,
    kind: node.type,
    color: nodeColors[node.type] ?? '#64748b',
    label: node.label,
    meta: node.type === 'loop' ? loopMeta(node) : node.type,
    title: node.label,
    pins: nodeToCanvasPins(node),
    classes: [`task-node-${node.type}`],
  })),
)

const previewEdges = computed<GraphCanvasEdge[]>(() =>
  (props.selectedGraph?.edges ?? []).map((edge) => ({
    id: edge.id,
    from: edge.from,
    to: edge.to,
    label: edgeDisplayLabel(edge),
    sourceHandle: edge.from_pin ?? edge.source_handle,
    targetHandle: edge.to_pin ?? edge.target_handle,
    removable: false,
    dashed: edge.kind === 'data',
    classes: [
      edge.from_pin ? `task-edge-source-${edge.from_pin.replace(/[^a-z0-9-]/gi, '-')}` : '',
      edge.to_pin ? `task-edge-target-${edge.to_pin.replace(/[^a-z0-9-]/gi, '-')}` : '',
    ].filter((item): item is string => Boolean(item)),
  })),
)

const previewCanvasSize = computed(() => {
  const nodes = previewNodes.value
  if (nodes.length === 0) return { width: 980, height: 560 }
  return {
    width: Math.max(980, Math.max(...nodes.map((node) => node.x + (node.width ?? 220))) + 120),
    height: Math.max(560, Math.max(...nodes.map((node) => node.y + (node.height ?? 76))) + 120),
  }
})

const selectedPreviewNode = computed<TaskGraphNode | null>(() =>
  selectedPreviewNodeId.value
    ? (props.selectedGraph?.nodes ?? []).find((node) => node.id === selectedPreviewNodeId.value) ?? null
    : null,
)

function scopeLabel(scope: string) {
  return scope === 'system' ? t('taskGraphSystem') : t('taskGraphProject')
}

function formatDate(value: string) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date)
}

function lastRunLabel() {
  const run = selectedRunHistory.value[0]
  return run
    ? `${run.status} · ${formatDate(run.updated_at)}`
    : props.selectedCatalogItem?.last_run
      ? `${props.selectedCatalogItem.last_run.status} · ${formatDate(props.selectedCatalogItem.last_run.updated_at)}`
      : t('taskGraphNeverRun')
}
</script>

<template>
  <main :class="['task-graph-preview', { embedded: hasEmbeddedDetailPanel }]">
    <header v-if="!hasEmbeddedDetailPanel">
      <div>
        <span>{{ selectedCatalogItem ? scopeLabel(selectedCatalogItem.scope) : t('taskGraphSelected') }}</span>
        <h3>{{ selectedCatalogItem?.title ?? t('taskGraphEmptyPreview') }}</h3>
        <p v-if="selectedCatalogItem">
          {{ selectedCatalogItem.id }} · {{ t('taskGraphVersion') }} {{ selectedCatalogItem.version }} · {{ lastRunLabel() }}
        </p>
      </div>
      <div class="task-graph-preview-actions">
        <span v-if="mode === 'edit' && (selectedCatalogItem?.scope === 'project' || project === 'blackboard')" class="task-graph-edit-ready">
          {{ t('taskGraphEditorPending') }}
        </span>
        <button
          v-if="selectedCatalogItem"
          type="button"
          class="bb-top-action-button"
          @click="emit('open')"
        >
          <Eye class="bb-top-action-svg" aria-hidden="true" />
          <span>{{ t('taskGraphView') }}</span>
        </button>
        <button
          v-if="selectedCatalogItem && (selectedCatalogItem.scope === 'project' || project === 'blackboard')"
          type="button"
          class="bb-top-action-button"
          :disabled="!!actionBusy"
          @click="emit('edit')"
        >
          <Pencil class="bb-top-action-svg" aria-hidden="true" />
          <span>{{ t('taskGraphEdit') }}</span>
        </button>
        <button
          v-if="selectedCatalogItem && selectedCatalogItem.scope === 'system'"
          type="button"
          class="bb-top-action-button"
          :disabled="!!actionBusy"
          @click="emit('customize')"
        >
          <GitFork class="bb-top-action-svg" aria-hidden="true" />
          <span>{{ t('taskGraphCustomize') }}</span>
        </button>
        <button
          v-if="selectedCatalogItem"
          type="button"
          class="bb-top-action-button"
          :disabled="!!actionBusy"
          @click="emit('run')"
        >
          <Play class="bb-top-action-svg" aria-hidden="true" />
          <span>{{ selectedActiveRun ? t('taskGraphOpenRun') : t('taskGraphRun') }}</span>
        </button>
      </div>
    </header>

    <div v-if="graphLoading" class="bb-state-panel">{{ t('loading') }}</div>
    <div v-else-if="selectedCatalogItem?.compile_error" class="task-graph-compile-error-panel">
      <AlertCircle class="task-graph-compile-error-icon" aria-hidden="true" />
      <h4>{{ t('taskGraphCompileError') }}</h4>
      <p>{{ t('taskGraphCompileErrorDetail') }}</p>
      <pre class="task-graph-compile-error-detail">{{ selectedCatalogItem.compile_error }}</pre>
    </div>
    <div v-else-if="selectedGraph" class="task-graph-preview-body">
      <TaskGraphRunInputsPanel
        v-if="selectedGraph.inputs?.length"
        :inputs="selectedGraph.inputs"
        :values="runInputValues"
        @update="(id, val) => emit('update:run-input', id, val)"
      />
      <GraphCanvas
        class="ticket-graph-canvas task-graph-preview-canvas"
        readonly
        :fit-padding="40"
        :nodes="previewNodes"
        :edges="previewEdges"
        :canvas-size="previewCanvasSize"
        :default-node-width="220"
        :default-node-height="48"
        @node-select="selectedPreviewNodeId = $event"
      >
        <template #node="{ node, width, height }">
          <g class="task-graph-ue-node" :class="[`task-graph-ue-${node.kind}`]">
            <rect :width="width" :height="height" rx="6" />
            <rect class="graph-node-header" :width="width" height="32" rx="6" />
            <rect class="graph-node-header-bottom" :width="width" y="24" height="8" />
            <line class="graph-node-divider" x1="0" y1="32" :x2="width" y2="32" />
            <circle cx="16" cy="16" r="5" :fill="node.color ?? '#64748b'" />
            <text x="28" y="21" class="graph-node-title">{{ node.label }}</text>
          </g>
        </template>
        <template #edge-label="{ edge, midpoint }">
          <text
            v-if="edge.label"
            class="task-graph-edge-label"
            :x="midpoint.x"
            :y="midpoint.y - 10"
          >
            {{ edge.label }}
          </text>
        </template>
      </GraphCanvas>
      <Transition name="node-popover">
        <section v-if="selectedPreviewNode" class="task-graph-node-popover">
          <header>
            <h4>{{ selectedPreviewNode.label }}</h4>
            <button type="button" @click="selectedPreviewNodeId = ''">×</button>
          </header>
          <dl class="task-graph-node-popover-fields">
            <dt>ID</dt>
            <dd>{{ selectedPreviewNode.id }}</dd>
            <dt>{{ t('agentKind') }}</dt>
            <dd>{{ selectedPreviewNode.type }}</dd>
            <template v-for="(value, key) in selectedPreviewNode.config" :key="key">
              <dt>{{ key }}</dt>
              <dd>{{ typeof value === 'object' ? JSON.stringify(value, null, 2) : String(value ?? '') }}</dd>
            </template>
          </dl>
        </section>
      </Transition>
      <TaskGraphRunHistory
        :runs="selectedRunHistory"
        :source="runHistorySource"
        :loading="runHistoryLoading"
        @reload="emit('reload-history')"
        @open="(run) => emit('open-run', run)"
      />
    </div>
    <div v-else class="bb-empty">{{ t('taskGraphEmptyPreview') }}</div>
  </main>
</template>

<style scoped>
.task-graph-preview {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  overflow: hidden;
  min-width: 0;
  min-height: 0;
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-preview.embedded {
  grid-template-rows: minmax(0, 1fr);
}

.task-graph-preview > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px;
  border-bottom: 1px solid var(--bb-border-warm);
}

.task-graph-preview-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
}

.task-graph-preview h3 {
  display: block;
  overflow-wrap: anywhere;
  margin: 3px 0;
  font-size: 18px;
  color: var(--bb-text-strong);
}

.task-graph-preview p {
  margin: 0;
}

.task-graph-preview-body {
  display: grid;
  align-content: start;
  gap: 12px;
  min-height: 0;
  padding: 12px;
}

.task-graph-preview-canvas {
  min-height: 420px;
}

.task-graph-preview-canvas :deep(svg) {
  width: 100%;
  min-width: 0;
  height: clamp(360px, 50vh, 620px);
}

.task-graph-preview-canvas :deep(.graph-node-accent) {
  fill: var(--graph-status-color);
}

.task-graph-edge-label {
  fill: var(--bb-text-muted);
  font-size: 12px;
  font-weight: 760;
  paint-order: stroke;
  stroke: var(--bb-surface);
  stroke-width: 4px;
  text-anchor: middle;
}

.task-graph-edit-ready {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-height: 24px;
  padding: 0 8px;
  border-radius: 999px;
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
  font-size: 11px;
  font-weight: 820;
}

.task-graph-compile-error-panel {
  display: grid;
  place-items: center;
  gap: 8px;
  padding: 32px 24px;
  text-align: center;
}

.task-graph-compile-error-panel h4 {
  margin: 0;
  color: var(--bb-error);
  font-size: 16px;
  font-weight: 780;
}

.task-graph-compile-error-panel p {
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 13px;
}

.task-graph-compile-error-icon {
  width: 32px;
  height: 32px;
  color: var(--bb-error);
}

.task-graph-compile-error-detail {
  max-width: 100%;
  margin: 8px 0 0;
  padding: 12px 16px;
  overflow-x: auto;
  border: 1px solid var(--task-graph-error-border);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-error) 6%, var(--bb-surface-soft));
  color: var(--bb-text-strong);
  font-family: var(--bb-font-mono, monospace);
  font-size: 12px;
  line-height: 1.5;
  text-align: left;
  white-space: pre-wrap;
  word-break: break-word;
}

.task-graph-node-popover {
  padding: 12px 14px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 10px;
  background: var(--bb-surface);
  box-shadow: var(--bb-md-shadow-soft);
}

.task-graph-node-popover header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.task-graph-node-popover header h4 {
  margin: 0;
  font-size: 14px;
  font-weight: 700;
  color: var(--bb-text-strong);
}

.task-graph-node-popover header button {
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--bb-text-muted);
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
}

.task-graph-node-popover header button:hover {
  background: var(--bb-surface-muted);
  color: var(--bb-text);
}

.task-graph-node-popover-fields {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 4px 12px;
  margin: 0;
  font-size: 12px;
}

.task-graph-node-popover-fields dt {
  color: var(--bb-text-muted);
  font-weight: 600;
  white-space: nowrap;
}

.task-graph-node-popover-fields dd {
  margin: 0;
  color: var(--bb-text);
  word-break: break-all;
  white-space: pre-wrap;
  max-height: 120px;
  overflow-y: auto;
}

.node-popover-enter-active {
  transition: all 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

.node-popover-leave-active {
  transition: all 0.15s cubic-bezier(0.4, 0, 1, 1);
}

.node-popover-enter-from,
.node-popover-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
</style>
