<script setup lang="ts">
import { computed } from 'vue'
import type { GraphCanvasNode } from '@/components/GraphCanvas.vue'
import { taskGraphNodeVisual } from './taskGraphNodeVisuals'

const props = defineProps<{
  node: GraphCanvasNode
  width: number
  height: number
}>()

const visual = computed(() => taskGraphNodeVisual(props.node.kind))
const nodeLabel = computed(() => truncateLabel(props.node.label || props.node.title || props.node.id, 18))
const nodeMeta = computed(() => truncateLabel(props.node.meta || visual.value.categoryLabel, 24))
const statusLabel = computed(() =>
  props.node.status && props.node.status !== props.node.kind
    ? truncateLabel(props.node.status, 14)
    : '',
)
const headerHeight = 32
const footerHeight = 26
const footerY = computed(() => Math.max(headerHeight + 30, props.height - footerHeight))
const statusBadgeWidth = computed(() => Math.min(108, Math.max(72, props.width - 24)))
const footerPath = computed(() => {
  const radius = 6
  const y = footerY.value
  const h = props.height
  const w = props.width
  return `M 0 ${y} H ${w} V ${h - radius} Q ${w} ${h} ${w - radius} ${h} H ${radius} Q 0 ${h} 0 ${h - radius} Z`
})

function truncateLabel(value: string, maxLength: number) {
  return value.length > maxLength ? `${value.slice(0, maxLength - 1)}...` : value
}
</script>

<template>
  <g class="task-graph-node-shape" :class="[`task-graph-node-shape-${node.kind}`]">
    <rect class="task-graph-node-body" :width="width" :height="height" rx="6" />
    <rect class="task-graph-node-header" :width="width" :height="headerHeight" rx="6" />
    <rect class="task-graph-node-header-bottom" :width="width" y="24" height="8" />
    <line class="task-graph-node-divider" x1="0" :y1="headerHeight" :x2="width" :y2="headerHeight" />
    <circle class="task-graph-node-icon-disc" cx="18" cy="16" r="10" />
    <text x="18" y="20" class="task-graph-node-icon-text">{{ visual.icon }}</text>
    <text x="34" y="20" class="task-graph-node-title">{{ nodeLabel }}</text>
    <text x="12" y="50" class="task-graph-node-meta">{{ nodeMeta }}</text>
    <g class="task-graph-node-type-badge" :transform="`translate(${Math.max(82, width - 84)}, 8)`">
      <rect width="72" height="17" rx="5" />
      <text x="36" y="12">{{ visual.label }}</text>
    </g>
    <g v-if="statusLabel" class="task-graph-node-footer">
      <path class="task-graph-node-footer-bg" :d="footerPath" />
      <line class="task-graph-node-footer-divider" x1="0" :y1="footerY" :x2="width" :y2="footerY" />
      <g class="task-graph-node-status-badge" :transform="`translate(12, ${footerY + 5})`">
        <rect :width="statusBadgeWidth" height="16" rx="5" />
        <circle cx="9" cy="8" r="3" />
        <text x="18" y="11">{{ statusLabel }}</text>
      </g>
    </g>
  </g>
</template>

<style scoped>
.task-graph-node-body {
  fill: color-mix(in srgb, var(--graph-status-color, var(--bb-text-muted)) 7%, var(--bb-surface));
  stroke: color-mix(in srgb, var(--graph-status-color, var(--bb-text-muted)) 26%, rgba(15, 23, 42, 0.16));
  stroke-width: 1.3;
  filter: drop-shadow(0 10px 18px rgba(15, 23, 42, 0.08));
}

.task-graph-node-header,
.task-graph-node-header-bottom {
  fill: var(--graph-status-color, var(--bb-text-muted));
  opacity: 0.12;
}

.task-graph-node-divider {
  stroke: var(--bb-hairline);
  stroke-width: 1;
}

.task-graph-node-footer-bg {
  fill: color-mix(in srgb, var(--graph-status-color, var(--bb-text-muted)) 5%, var(--bb-surface));
}

.task-graph-node-footer-divider {
  stroke: color-mix(in srgb, var(--graph-status-color, var(--bb-text-muted)) 16%, var(--bb-hairline));
  stroke-width: 1;
}

.task-graph-node-icon-disc {
  fill: var(--graph-status-color, var(--bb-text-muted));
  opacity: 0.16;
}

.task-graph-node-icon-text {
  fill: var(--graph-status-color, var(--bb-text-muted));
  font-size: 9px;
  font-weight: 900;
  text-anchor: middle;
}

.task-graph-node-title {
  fill: var(--bb-text-strong);
  font-size: 14px;
  font-weight: 760;
}

.task-graph-node-meta {
  fill: var(--bb-text-muted);
  font-size: 11px;
  font-weight: 720;
}

.task-graph-node-type-badge rect,
.task-graph-node-status-badge rect {
  fill: color-mix(in srgb, var(--graph-status-color, var(--bb-text-muted)) 10%, var(--bb-surface));
  stroke: color-mix(in srgb, var(--graph-status-color, var(--bb-text-muted)) 22%, transparent);
  stroke-width: 1;
}

.task-graph-node-type-badge text,
.task-graph-node-status-badge text {
  fill: color-mix(in srgb, var(--graph-status-color, var(--bb-text-muted)) 74%, var(--bb-text-strong));
  font-size: 9px;
  font-weight: 820;
}

.task-graph-node-type-badge text {
  text-anchor: middle;
}

.task-graph-node-status-badge circle {
  fill: var(--graph-status-color, var(--bb-text-muted));
}
</style>
