<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  AlertCircle,
  Eye,
  GitFork,
  ListChecks,
  Pencil,
  Play,
  X,
} from 'lucide-vue-next'
import GraphCanvas, { type GraphCanvasEdge, type GraphCanvasNode } from '@/components/GraphCanvas.vue'
import { BbActionGroup, BbButton, BbIconCommand, BbInfoGrid, BbInfoItem, BbSummaryChip } from '@/components/common'
import TaskGraphNodeShape from '@/components/task-graph/TaskGraphNodeShape.vue'
import TaskGraphRunInputsPanel from '@/components/task-graph/TaskGraphRunInputsPanel.vue'
import TaskGraphRunHistory from '@/components/task-graph/TaskGraphRunHistory.vue'
import TaskGraphSchedulePanel from '@/components/task-graph/TaskGraphSchedulePanel.vue'
import {
  taskGraphNodeMetaLabel,
  taskGraphNodeVisualForNode,
} from '@/components/task-graph/taskGraphNodeVisuals'
import { t } from '@/i18n'
import {
  nodeToCanvasPins,
  nodeHeightForPins,
  type TaskGraphCatalogItem,
  type TaskGraphDefinition,
  type TaskGraphEdge,
  type TaskGraphNode,
  type TaskGraphRef,
  type TaskGraphRunSummary,
  type TaskGraphSchedule,
  type TaskGraphScheduleCreateInput,
  type TaskGraphSchedulePatchInput,
} from '@/data/taskGraphs'

const props = defineProps<{
  selectedCatalogItem: TaskGraphCatalogItem | null
  selectedGraph: TaskGraphDefinition | null
  selectedRef: TaskGraphRef | null
  graphLoading: boolean
  activeRunId: string
  runInputValues: Record<string, unknown>
  runHistory: TaskGraphRunSummary[]
  runHistorySource: 'rest'
  runHistoryLoading: boolean
  schedules: TaskGraphSchedule[]
  schedulesLoading: boolean
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
  'create-schedule': [input: TaskGraphScheduleCreateInput]
  'patch-schedule': [id: string, patch: TaskGraphSchedulePatchInput]
  'delete-schedule': [id: string]
  'run-schedule-now': [id: string]
  'reload-schedules': []
}>()

const selectedPreviewNodeId = ref('')
const activePreviewPanel = ref<'inputs' | ''>('')

const selectedActiveRun = computed(() =>
  props.selectedRef ? props.runHistory.find((run) =>
    run.id === props.activeRunId ||
    (run.graph_ref?.scope === props.selectedRef?.scope && run.graph_ref?.id === props.selectedRef?.id && (run.status === 'queued' || run.status === 'running' || run.status === 'pending' || run.status === 'paused'))
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

const runInputSummary = computed(() => {
  const count = props.selectedGraph?.inputs?.length ?? 0
  return count === 0
    ? t('taskGraphEditorNoInputs')
    : t('taskGraphEditorInputsCount', { count })
})

function togglePreviewInputs() {
  activePreviewPanel.value = activePreviewPanel.value === 'inputs' ? '' : 'inputs'
  if (activePreviewPanel.value) selectedPreviewNodeId.value = ''
}

function handlePreviewNodeSelect(id: string) {
  activePreviewPanel.value = ''
  selectedPreviewNodeId.value = id
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

const previewNodes = computed<GraphCanvasNode[]>(() =>
  (props.selectedGraph?.nodes ?? []).map((node) => ({
    id: node.id,
    x: node.position?.x ?? 80,
    y: node.position?.y ?? 120,
    width: 230,
    height: Math.max(nodeHeightForPins(node), 64),
    status: node.type,
    kind: node.type,
    color: taskGraphNodeVisualForNode(node).color,
    label: node.label,
    meta: taskGraphNodeMetaLabel(node, props.selectedGraph?.inputs ?? []),
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
    width: Math.max(980, Math.max(...nodes.map((node) => node.x + (node.width ?? 230))) + 120),
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
      <BbActionGroup class="task-graph-preview-actions" gap="sm">
        <span v-if="mode === 'edit' && (selectedCatalogItem?.scope === 'project' || project === 'blackboard')" class="task-graph-edit-ready">
          {{ t('taskGraphEditorPending') }}
        </span>
        <BbButton
          v-if="selectedCatalogItem"
          variant="secondary"
          size="sm"
          @click="emit('open')"
        >
          <template #leading>
            <Eye />
          </template>
          {{ t('taskGraphView') }}
        </BbButton>
        <BbButton
          v-if="selectedCatalogItem && (selectedCatalogItem.scope === 'project' || project === 'blackboard')"
          variant="secondary"
          size="sm"
          :disabled="!!actionBusy"
          @click="emit('edit')"
        >
          <template #leading>
            <Pencil />
          </template>
          {{ t('taskGraphEdit') }}
        </BbButton>
        <BbButton
          v-if="selectedCatalogItem && selectedCatalogItem.scope === 'system'"
          variant="secondary"
          size="sm"
          :disabled="!!actionBusy"
          @click="emit('customize')"
        >
          <template #leading>
            <GitFork />
          </template>
          {{ t('taskGraphCustomize') }}
        </BbButton>
        <BbButton
          v-if="selectedCatalogItem"
          variant="primary"
          size="sm"
          :disabled="!!actionBusy"
          @click="emit('run')"
        >
          <template #leading>
            <Play />
          </template>
          {{ selectedActiveRun ? t('taskGraphOpenRun') : t('taskGraphRun') }}
        </BbButton>
      </BbActionGroup>
    </header>

    <div v-if="graphLoading" class="bb-state-panel">{{ t('loading') }}</div>
    <div v-else-if="selectedCatalogItem?.compile_error" class="task-graph-compile-error-panel">
      <AlertCircle class="task-graph-compile-error-icon" aria-hidden="true" />
      <h4>{{ t('taskGraphCompileError') }}</h4>
      <p>{{ t('taskGraphCompileErrorDetail') }}</p>
      <pre class="task-graph-compile-error-detail">{{ selectedCatalogItem.compile_error }}</pre>
    </div>
    <div v-else-if="selectedGraph" class="task-graph-preview-body">
      <section v-if="selectedGraph.inputs?.length" class="task-graph-preview-summary-strip">
        <BbSummaryChip
          :title="t('taskGraphInputs')"
          :subtitle="runInputSummary"
          :active="activePreviewPanel === 'inputs'"
          @click="togglePreviewInputs"
        >
          <template #icon>
            <ListChecks />
          </template>
        </BbSummaryChip>
        <BbButton
          v-if="activePreviewPanel"
          class="task-graph-preview-summary-close"
          variant="secondary"
          size="sm"
          @click="activePreviewPanel = ''"
        >
          <template #leading>
            <X />
          </template>
          {{ t('taskGraphEditorCloseConfig') }}
        </BbButton>
      </section>

      <Transition name="task-graph-preview-panel">
        <section
          v-if="activePreviewPanel === 'inputs' && selectedGraph.inputs?.length"
          class="task-graph-preview-config-panel"
        >
          <header>
            <h4>{{ t('taskGraphInputs') }}</h4>
            <BbButton
              icon-only
              size="mini"
              variant="ghost"
              :aria-label="t('taskGraphEditorCloseConfig')"
              @click="activePreviewPanel = ''"
            >
              <X />
            </BbButton>
          </header>
          <TaskGraphRunInputsPanel
            :inputs="selectedGraph.inputs"
            :values="runInputValues"
            layout="drawer"
            @update="(id, val) => emit('update:run-input', id, val)"
          />
        </section>
      </Transition>

      <GraphCanvas
        class="ticket-graph-canvas task-graph-preview-canvas"
        readonly
        :fit-padding="40"
        :nodes="previewNodes"
        :edges="previewEdges"
        :canvas-size="previewCanvasSize"
        :default-node-width="230"
        :default-node-height="64"
        @node-select="handlePreviewNodeSelect"
      >
        <template #node="{ node, width, height }">
          <TaskGraphNodeShape :node="node" :width="width" :height="height" />
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
            <BbIconCommand
              size="mini"
              variant="ghost"
              :title="t('taskGraphEditorCloseConfig')"
              @click="selectedPreviewNodeId = ''"
            >
              <X />
            </BbIconCommand>
          </header>
          <BbInfoGrid class="task-graph-node-popover-fields" variant="rows">
            <BbInfoItem label="ID" :value="selectedPreviewNode.id" variant="mono" />
            <BbInfoItem :label="t('agentKind')" :value="selectedPreviewNode.type" variant="mono" />
            <template v-for="(value, key) in selectedPreviewNode.config" :key="key">
              <BbInfoItem :label="String(key)" variant="mono">
                {{ typeof value === 'object' ? JSON.stringify(value, null, 2) : String(value ?? '') }}
              </BbInfoItem>
            </template>
          </BbInfoGrid>
        </section>
      </Transition>
      <TaskGraphRunHistory
        :runs="selectedRunHistory"
        :source="runHistorySource"
        :loading="runHistoryLoading"
        @reload="emit('reload-history')"
        @open="(run) => emit('open-run', run)"
      />
      <TaskGraphSchedulePanel
        :selected-graph="selectedGraph"
        :selected-ref="selectedRef"
        :schedules="schedules"
        :loading="schedulesLoading"
        :run-input-values="runInputValues"
        :action-busy="actionBusy"
        @create="(input) => emit('create-schedule', input)"
        @patch="(id, patch) => emit('patch-schedule', id, patch)"
        @delete="(id) => emit('delete-schedule', id)"
        @run-now="(id) => emit('run-schedule-now', id)"
        @reload="emit('reload-schedules')"
        @navigate-run="(id) => emit('navigate-run', id)"
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
  min-width: 0;
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
  position: relative;
  min-height: 0;
  padding: 12px;
}

.task-graph-preview-summary-strip {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  min-height: 36px;
  overflow-x: auto;
  scrollbar-width: none;
}

.task-graph-preview-summary-strip::-webkit-scrollbar {
  display: none;
}

.task-graph-preview-summary-close {
  margin-left: auto;
}

.task-graph-preview-config-panel {
  position: absolute;
  top: 60px;
  right: 12px;
  bottom: 12px;
  z-index: 4;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  gap: 8px;
  width: min(440px, calc(100% - 250px));
  min-height: 0;
  padding: 10px;
  overflow: auto;
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-surface) 96%, transparent);
  box-shadow: var(--bb-md-shadow-soft);
  backdrop-filter: blur(8px);
}

.task-graph-preview-config-panel > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-width: 0;
}

.task-graph-preview-config-panel > header h4 {
  margin: 0;
  color: var(--bb-text-strong);
  font-size: 13px;
  font-weight: 820;
}

.task-graph-preview-panel-enter-active,
.task-graph-preview-panel-leave-active {
  transition: opacity 140ms ease, transform 140ms ease;
}

.task-graph-preview-panel-enter-from,
.task-graph-preview-panel-leave-to {
  opacity: 0;
  transform: translateX(8px);
}

@media (max-width: 980px) {
  .task-graph-preview-config-panel {
    left: 12px;
    width: auto;
  }
}

.task-graph-preview-canvas {
  min-height: 420px;
}

.task-graph-preview-canvas :deep(svg) {
  width: 100%;
  min-width: 0;
  height: clamp(360px, 50vh, 620px);
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
  font-family: var(--bb-font-mono);
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

.task-graph-node-popover-fields {
  --bb-info-row-gap: 4px;
  --bb-info-row-gap-x: 12px;
  --bb-info-label-font-size: 12px;
  --bb-info-label-font-weight: 600;
  --bb-info-label-white-space: nowrap;
  --bb-info-value-font-weight: 500;
  --bb-info-value-max-height: 120px;
  --bb-info-value-overflow-y: auto;
  --bb-info-value-white-space: pre-wrap;
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
