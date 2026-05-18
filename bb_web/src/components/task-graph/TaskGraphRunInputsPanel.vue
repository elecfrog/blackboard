<script setup lang="ts">
import type { TaskGraphInputParam } from '@/data/taskGraphs'
import { BbDenseRow, BbRefChip } from '@/components/common'
import { t } from '@/i18n'

defineProps<{
  inputs: TaskGraphInputParam[]
  values: Record<string, unknown>
  layout?: 'wide' | 'drawer'
}>()

const emit = defineEmits<{
  update: [inputId: string, value: unknown]
}>()

function eventValue(event: Event) {
  return event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLSelectElement
    ? event.target.value
    : ''
}

function inputDisplayText(input: TaskGraphInputParam, values: Record<string, unknown>) {
  const value = values[input.id] ?? input.default
  return typeof value === 'string' ? value : JSON.stringify(value ?? '')
}

function coerceInputValue(input: TaskGraphInputParam, raw: string | boolean) {
  if (input.type === 'number') return Number(raw || 0)
  if (input.type === 'boolean') return Boolean(raw)
  if (input.type === 'json') {
    try {
      return JSON.parse(String(raw || '{}'))
    } catch {
      return raw
    }
  }
  return raw
}

function inputReference(inputId: string) {
  return `{{inputs.${inputId}}}`
}
</script>

<template>
  <section v-if="inputs.length" :class="['task-graph-run-inputs', `task-graph-run-inputs-${layout ?? 'wide'}`]">
    <BbDenseRow
      v-for="input in inputs"
      :key="input.id"
      class="task-graph-run-input-row"
      :columns="layout === 'drawer' ? '' : 'minmax(180px, 1fr) minmax(180px, 0.55fr)'"
      gap="8px"
      control-height="32px"
    >
      <label class="task-graph-run-input-control bb-dense-cell">
        <span class="bb-dense-cell-label">{{ input.label || input.id }}</span>
        <input
          v-if="input.type === 'number'"
          class="bb-dense-control"
          type="number"
          :min="input.min"
          :max="input.max"
          :value="inputDisplayText(input, values)"
          @input="emit('update', input.id, coerceInputValue(input, eventValue($event)))"
        />
        <select
          v-else-if="input.type === 'boolean'"
          class="bb-dense-control"
          :value="String(values[input.id] ?? input.default ?? false)"
          @change="emit('update', input.id, coerceInputValue(input, eventValue($event) === 'true'))"
        >
          <option value="true">{{ t('taskGraphBooleanTrue') }}</option>
          <option value="false">{{ t('taskGraphBooleanFalse') }}</option>
        </select>
        <textarea
          v-else-if="input.type === 'json'"
          class="bb-dense-control"
          :value="inputDisplayText(input, values)"
          @input="emit('update', input.id, coerceInputValue(input, eventValue($event)))"
        />
        <input
          v-else
          class="bb-dense-control"
          :value="inputDisplayText(input, values)"
          @input="emit('update', input.id, coerceInputValue(input, eventValue($event)))"
        />
      </label>
      <div class="task-graph-run-input-reference bb-dense-cell">
        <span class="bb-dense-cell-label">{{ t('taskGraphInputReference') }}</span>
        <BbRefChip :value="inputReference(input.id)" />
      </div>
    </BbDenseRow>
  </section>
</template>

<style scoped>
.task-graph-run-inputs {
  display: grid;
  align-content: start;
  gap: 8px;
  align-self: start;
  min-width: 0;
  padding: 10px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface-soft);
}

.task-graph-run-inputs-drawer {
  border: 0;
  border-radius: 0;
  background: transparent;
}

.task-graph-run-inputs-drawer .task-graph-run-input-row {
  --bb-dense-row-columns: 1fr;
  --bb-dense-row-padding: 8px;
}

.task-graph-run-inputs-drawer .task-graph-run-input-reference {
  grid-template-columns: auto minmax(0, 1fr);
  align-items: center;
}

.task-graph-run-inputs-drawer .task-graph-run-input-reference .bb-dense-cell-label {
  line-height: 1.2;
}

.task-graph-run-input-reference .bb-ref-chip {
  width: 100%;
  min-height: 32px;
}

@media (max-width: 980px) {
  .task-graph-run-input-row {
    --bb-dense-row-columns: 1fr;
  }
}
</style>
