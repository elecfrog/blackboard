<script setup lang="ts">
import { t } from '@/i18n'
import { type TaskGraphNode } from '@/data/taskGraphs'

defineProps<{
  nodeTypes: Array<{ type: TaskGraphNode['type']; label: string; color: string }>
}>()

const emit = defineEmits<{
  'add-node': [type: TaskGraphNode['type']]
  'begin-drag': [type: TaskGraphNode['type'], event: DragEvent]
}>()
</script>

<template>
  <section class="task-graph-palette task-graph-editor-overlay">
    <h4>{{ t('taskGraphPalette') }}</h4>
    <button
      v-for="item in nodeTypes"
      :key="item.type"
      type="button"
      draggable="true"
      @click="emit('add-node', item.type)"
      @dragstart="emit('begin-drag', item.type, $event)"
    >
      <span :style="{ background: item.color }" />
      {{ item.label }}
    </button>
  </section>
</template>

<style scoped>
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
  color: var(--bb-text-muted);
  font-size: 12px;
  font-weight: 760;
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

.task-graph-editor-overlay {
  pointer-events: auto;
}
</style>
