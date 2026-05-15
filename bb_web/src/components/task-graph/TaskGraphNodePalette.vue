<script setup lang="ts">
import { computed } from 'vue'
import { t } from '@/i18n'
import { type TaskGraphNodeVisual } from './taskGraphNodeVisuals'

type PaletteNodeType = TaskGraphNodeVisual

const emit = defineEmits<{
  'add-node': [item: PaletteNodeType]
  'begin-drag': [item: PaletteNodeType, event: DragEvent]
}>()

const props = defineProps<{
  nodeTypes: PaletteNodeType[]
}>()

const groupedNodeTypes = computed(() => {
  const groups: Array<{ key: string; label: string; items: PaletteNodeType[] }> = []
  for (const item of props.nodeTypes) {
    const key = item.category ?? 'node'
    let group = groups.find((entry) => entry.key === key)
    if (!group) {
      group = { key, label: item.categoryLabel ?? key, items: [] }
      groups.push(group)
    }
    group.items.push(item)
  }
  return groups
})
</script>

<template>
  <section class="task-graph-palette task-graph-editor-overlay">
    <h4>{{ t('taskGraphPalette') }}</h4>
    <div v-for="group in groupedNodeTypes" :key="group.key" class="task-graph-palette-group">
      <p>{{ group.label }}</p>
      <button
        v-for="item in group.items"
        :key="item.key ?? item.type"
        type="button"
        draggable="true"
        @click="emit('add-node', item)"
        @dragstart="emit('begin-drag', item, $event)"
      >
        <span class="task-graph-palette-icon" :style="{ '--node-color': item.color }">
          {{ item.icon || item.label.slice(0, 1) }}
        </span>
        <span class="task-graph-palette-copy">
          <strong>{{ item.label }}</strong>
          <small v-if="item.description">{{ item.description }}</small>
        </span>
      </button>
    </div>
  </section>
</template>

<style scoped>
.task-graph-palette,
.task-graph-validation-card {
  display: grid;
  gap: 10px;
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

.task-graph-palette-group {
  display: grid;
  gap: 6px;
}

.task-graph-palette-group p {
  margin: 0;
  color: var(--bb-text-faint);
  font-size: 10px;
  font-weight: 820;
  letter-spacing: 0;
  text-transform: uppercase;
}

.task-graph-palette button {
  display: grid;
  grid-template-columns: 24px minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  min-height: 38px;
  padding: 6px 8px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-strong);
  cursor: grab;
  font-size: 12px;
  font-weight: 760;
  text-align: left;
}

.task-graph-palette-icon {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  border-radius: 7px;
  background: color-mix(in srgb, var(--node-color, var(--bb-accent)) 12%, var(--bb-surface));
  color: var(--node-color, var(--bb-accent));
  font-size: 10px;
  font-weight: 900;
}

.task-graph-palette-copy {
  display: grid;
  min-width: 0;
  gap: 2px;
}

.task-graph-palette-copy strong,
.task-graph-palette-copy small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-graph-palette-copy strong {
  color: var(--bb-text-strong);
  font-size: 12px;
  line-height: 1.1;
}

.task-graph-palette-copy small {
  color: var(--bb-text-muted);
  font-size: 10px;
  font-weight: 720;
  line-height: 1.1;
}

.task-graph-editor-overlay {
  pointer-events: auto;
}
</style>
