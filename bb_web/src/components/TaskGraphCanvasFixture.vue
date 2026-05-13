<script setup lang="ts">
import { computed, ref } from 'vue'
import GraphCanvas, {
  type GraphCanvasConnection,
  type GraphCanvasEdge,
  type GraphCanvasNode,
  type GraphCanvasNodeMove,
  type GraphCanvasViewport,
} from '@/components/GraphCanvas.vue'
import { t } from '@/i18n'

const nodeWidth = 220
const nodeHeight = 76
const viewport = ref<GraphCanvasViewport>({ x: 40, y: 36, scale: 1 })
const positions = ref<Record<string, { x: number; y: number }>>({
  start: { x: 80, y: 140 },
  'plan-llm': { x: 340, y: 140 },
  'smoke-task': { x: 600, y: 140 },
  'failure-branch': { x: 860, y: 140 },
  'fix-loop': { x: 1120, y: 280 },
  'end-success': { x: 1120, y: 80 },
})

const fixtureNodes = [
  { id: 'start', type: 'start', label: t('taskGraphNodeTypeStart'), color: '#64748b' },
  { id: 'plan-llm', type: 'llm', label: t('taskGraphFixturePlanLlm'), color: '#2563eb' },
  { id: 'smoke-task', type: 'llm', label: t('taskGraphFixtureSmokeTask'), color: '#2563eb' },
  { id: 'failure-branch', type: 'branch', label: t('taskGraphFixtureHasFailures'), color: '#d97706' },
  { id: 'fix-loop', type: 'loop', label: t('taskGraphFixtureRetryMax'), color: '#7c3aed' },
  { id: 'end-success', type: 'end', label: t('taskGraphFixtureSuccess'), color: '#059669' },
]

const nodes = computed<GraphCanvasNode[]>(() =>
  fixtureNodes.map((node) => ({
    ...node,
    x: positions.value[node.id]?.x ?? 80,
    y: positions.value[node.id]?.y ?? 80,
    width: nodeWidth,
    height: nodeHeight,
    status: node.type,
  })),
)

const edges = ref<GraphCanvasEdge[]>([
  { id: 'start__plan-llm', from: 'start', to: 'plan-llm' },
  { id: 'plan-llm__smoke-task', from: 'plan-llm', to: 'smoke-task' },
  { id: 'smoke-task__failure-branch', from: 'smoke-task', to: 'failure-branch' },
  { id: 'failure-branch__fix-loop__has-failures', from: 'failure-branch', to: 'fix-loop', label: t('taskGraphFixtureHasFailuresShort') },
  { id: 'failure-branch__end-success__clean', from: 'failure-branch', to: 'end-success', label: t('taskGraphFixtureNoFailures') },
])

function handleNodeMove(move: GraphCanvasNodeMove) {
  positions.value = {
    ...positions.value,
    [move.id]: { x: move.x, y: move.y },
  }
}

function handleConnectionCreate(connection: GraphCanvasConnection) {
  const id = `${connection.sourceId}__${connection.targetId}`
  if (edges.value.some((edge) => edge.id === id)) return
  edges.value = [...edges.value, { id, from: connection.sourceId, to: connection.targetId }]
}
</script>

<template>
  <section class="task-graph-fixture">
    <header>
      <strong>{{ t('taskGraphFixtureTitle') }}</strong>
      <span>{{ Math.round(viewport.scale * 100) }}%</span>
    </header>
    <GraphCanvas
      class="ticket-graph-canvas"
      :nodes="nodes"
      :edges="edges"
      :default-node-width="nodeWidth"
      :default-node-height="nodeHeight"
      :canvas-size="{ width: 1400, height: 720 }"
      @viewport-change="viewport = $event"
      @node-move="handleNodeMove"
      @node-move-end="handleNodeMove"
      @connection-create="handleConnectionCreate"
      @edge-remove="edges = edges.filter((item) => item.id !== $event.id)"
    >
      <template #node="{ node, width, height }">
        <rect :width="width" :height="height" rx="6" />
        <rect class="graph-node-header" :width="width" height="32" rx="6" />
        <rect class="graph-node-header-bottom" :width="width" y="24" height="8" />
        <line class="graph-node-divider" x1="0" y1="32" :x2="width" y2="32" />
        <circle cx="16" cy="16" r="5" :fill="node.color ?? '#64748b'" />
        <text x="28" y="21" class="graph-node-title">{{ node.label }}</text>
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
  </section>
</template>

<style scoped>
.task-graph-fixture {
  display: grid;
  gap: 10px;
}

.task-graph-fixture header {
  display: flex;
  justify-content: space-between;
  color: var(--bb-text-muted);
  font-size: 12px;
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
</style>
