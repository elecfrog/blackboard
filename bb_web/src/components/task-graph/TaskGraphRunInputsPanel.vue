<script setup lang="ts">
import type { TaskGraphInputParam } from '@/data/taskGraphs'
import { t } from '@/i18n'

defineProps<{
  inputs: TaskGraphInputParam[]
  values: Record<string, unknown>
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
  <section v-if="inputs.length" class="task-graph-run-inputs">
    <div
      v-for="input in inputs"
      :key="input.id"
      class="task-graph-run-input-row"
    >
      <label class="task-graph-run-input-control">
        <span>{{ input.label || input.id }}</span>
        <input
          v-if="input.type === 'number'"
          type="number"
          :min="input.min"
          :max="input.max"
          :value="inputDisplayText(input, values)"
          @input="emit('update', input.id, coerceInputValue(input, eventValue($event)))"
        />
        <select
          v-else-if="input.type === 'boolean'"
          :value="String(values[input.id] ?? input.default ?? false)"
          @change="emit('update', input.id, coerceInputValue(input, eventValue($event) === 'true'))"
        >
          <option value="true">{{ t('taskGraphBooleanTrue') }}</option>
          <option value="false">{{ t('taskGraphBooleanFalse') }}</option>
        </select>
        <textarea
          v-else-if="input.type === 'json'"
          :value="inputDisplayText(input, values)"
          @input="emit('update', input.id, coerceInputValue(input, eventValue($event)))"
        />
        <input
          v-else
          :value="inputDisplayText(input, values)"
          @input="emit('update', input.id, coerceInputValue(input, eventValue($event)))"
        />
      </label>
      <div class="task-graph-run-input-reference">
        <span>{{ t('taskGraphInputReference') }}</span>
        <small v-text="inputReference(input.id)" />
      </div>
    </div>
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

.task-graph-run-input-row {
  display: grid;
  grid-template-columns: minmax(180px, 1fr) minmax(180px, 0.55fr);
  align-items: end;
  gap: 8px;
  min-width: 0;
  padding: 7px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-run-input-control,
.task-graph-run-input-reference {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.task-graph-run-input-control > span,
.task-graph-run-input-reference > span {
  overflow: hidden;
  color: var(--bb-text-muted);
  font-size: 10px;
  font-weight: 760;
  line-height: 1;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-graph-run-inputs input,
.task-graph-run-inputs select,
.task-graph-run-inputs textarea {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  min-height: 32px;
  padding: 6px 8px;
  border: 1px solid var(--bb-border-warm-medium-strong);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-strong);
  font: inherit;
  font-size: 12px;
}

.task-graph-run-inputs small {
  box-sizing: border-box;
  display: flex;
  align-items: center;
  width: 100%;
  min-width: 0;
  min-height: 32px;
  padding: 6px 8px;
  overflow: hidden;
  border: 1px solid var(--task-graph-reference-chip-border);
  border-radius: 8px;
  background: var(--task-graph-reference-chip-bg);
  color: var(--task-graph-reference-chip-text);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 11px;
  font-weight: 760;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 980px) {
  .task-graph-run-input-row {
    grid-template-columns: 1fr;
  }
}
</style>
