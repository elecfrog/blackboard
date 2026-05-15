<script setup lang="ts">
import { Plus, Trash2 } from 'lucide-vue-next'
import type { TaskGraphInputParam } from '@/data/taskGraphs'
import { t } from '@/i18n'

type GraphInputPatch = Partial<Omit<TaskGraphInputParam, 'default'>> & {
  default?: unknown
}

defineProps<{
  inputs: TaskGraphInputParam[]
  readonly?: boolean
  layout?: 'wide' | 'drawer'
}>()

const emit = defineEmits<{
  add: []
  remove: [index: number]
  update: [index: number, patch: GraphInputPatch]
  typeChange: [index: number, value: string]
}>()

const inputTypeOptions = [
  'string',
  'number',
  'boolean',
  'json',
  'ticket_ref',
  'array<string>',
  'array<number>',
  'array<ticket_ref>',
  'array<json>',
]

function inputValue(event: Event) {
  return event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLSelectElement
    ? event.target.value
    : ''
}

function kebab(value: string, fallback: string) {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return normalized || fallback
}

function defaultValueForInputType(type: TaskGraphInputParam['type']) {
  if (type === 'number') return 1
  if (type === 'boolean') return false
  if (type === 'json') return {}
  if (type.startsWith('array<')) return []
  return ''
}

function inputDefaultText(input: TaskGraphInputParam) {
  const value = input.default ?? defaultValueForInputType(input.type)
  return typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean'
    ? String(value)
    : JSON.stringify(value)
}

function coerceInputDefault(type: TaskGraphInputParam['type'], value: string) {
  if (type === 'number') return Number(value || 0)
  if (type === 'boolean') return value === 'true'
  if (type === 'json') {
    try {
      return JSON.parse(value || '{}')
    } catch {
      return value
    }
  }
  return value
}

function inputReference(inputId: string) {
  return `{{inputs.${inputId}}}`
}
</script>

<template>
  <section :class="['task-graph-inputs', `task-graph-inputs-${layout ?? 'wide'}`]">
    <header>
      <h4>{{ t('taskGraphInputs') }}</h4>
      <button type="button" :disabled="readonly" @click="emit('add')">
        <Plus aria-hidden="true" />
      </button>
    </header>
    <div v-if="inputs.length === 0" class="task-graph-input-empty">
      {{ t('taskGraphInputsEmpty') }}
    </div>
    <div
      v-for="(item, index) in inputs"
      :key="`${item.id}-${index}`"
      class="task-graph-input-row"
    >
      <label class="task-graph-input-cell task-graph-input-cell-id">
        <span>{{ t('taskGraphInputId') }}</span>
        <input
          :value="item.id"
          :disabled="readonly"
          placeholder="batch-count"
          @input="emit('update', index, { id: kebab(inputValue($event), `input-${index + 1}`) })"
        />
      </label>
      <label class="task-graph-input-cell task-graph-input-cell-label">
        <span>{{ t('label') }}</span>
        <input
          :value="item.label ?? ''"
          :disabled="readonly"
          placeholder="Inbox batch count"
          @input="emit('update', index, { label: inputValue($event) })"
        />
      </label>
      <label class="task-graph-input-cell task-graph-input-cell-type">
        <span>{{ t('taskGraphInputType') }}</span>
        <select
          :value="item.type"
          :disabled="readonly"
          @change="emit('typeChange', index, inputValue($event))"
        >
          <option v-for="type in inputTypeOptions" :key="type" :value="type">
            {{ type }}
          </option>
        </select>
      </label>
      <label class="task-graph-input-cell task-graph-input-cell-default">
        <span>{{ t('taskGraphInputDefault') }}</span>
        <input
          :value="inputDefaultText(item)"
          :disabled="readonly"
          placeholder="1"
          @input="emit('update', index, { default: coerceInputDefault(item.type, inputValue($event)) })"
        />
      </label>
      <div class="task-graph-input-cell task-graph-input-cell-reference">
        <span>{{ t('taskGraphInputReference') }}</span>
        <div class="task-graph-variable-chip">{{ inputReference(item.id) }}</div>
      </div>
      <button
        type="button"
        class="task-graph-input-remove"
        :title="t('taskGraphInputRemove')"
        :disabled="readonly"
        @click="emit('remove', index)"
      >
        <Trash2 aria-hidden="true" />
      </button>
    </div>
  </section>
</template>

<style scoped>
.task-graph-inputs {
  display: grid;
  align-content: start;
  gap: 9px;
  min-width: 0;
  overflow-x: auto;
  padding: 10px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface-soft);
}

.task-graph-inputs header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.task-graph-inputs header button,
.task-graph-input-remove {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-height: 28px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-muted);
  cursor: pointer;
}

.task-graph-inputs header button svg,
.task-graph-input-remove svg {
  width: 14px;
  height: 14px;
}

.task-graph-input-empty {
  color: var(--bb-text-muted);
  font-size: 12px;
}

.task-graph-input-row {
  display: grid;
  grid-template-columns:
    minmax(112px, 1fr)
    minmax(128px, 1.1fr)
    minmax(108px, 0.9fr)
    minmax(64px, 0.5fr)
    minmax(132px, 1.1fr)
    30px;
  gap: 6px;
  align-items: end;
  min-width: 0;
  padding: 7px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-inputs-drawer {
  border: 0;
  border-radius: 0;
  background: transparent;
}

.task-graph-inputs-drawer header {
  position: sticky;
  top: 0;
  z-index: 1;
  min-height: 34px;
  padding-bottom: 4px;
  background: color-mix(in srgb, var(--bb-surface) 96%, transparent);
  backdrop-filter: blur(8px);
}

.task-graph-inputs-drawer .task-graph-input-row {
  grid-template-columns: minmax(82px, 0.8fr) minmax(108px, 1fr) minmax(94px, 0.75fr) 30px;
  grid-template-areas:
    "id label type remove"
    "default default reference reference";
  align-items: end;
}

.task-graph-inputs-drawer .task-graph-input-cell-id {
  grid-area: id;
}

.task-graph-inputs-drawer .task-graph-input-cell-label {
  grid-area: label;
}

.task-graph-inputs-drawer .task-graph-input-cell-type {
  grid-area: type;
}

.task-graph-inputs-drawer .task-graph-input-cell-default {
  grid-area: default;
}

.task-graph-inputs-drawer .task-graph-input-cell-reference {
  grid-area: reference;
}

.task-graph-inputs-drawer .task-graph-input-remove {
  grid-area: remove;
}

.task-graph-input-cell {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.task-graph-input-cell > span {
  overflow: hidden;
  color: var(--bb-text-muted);
  font-size: 10px;
  font-weight: 760;
  line-height: 1;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-graph-input-row input,
.task-graph-input-row select,
.task-graph-input-row .task-graph-variable-chip {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  height: 30px;
  min-height: 30px;
  padding: 5px 7px;
  overflow: hidden;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-graph-input-row input,
.task-graph-input-row select {
  border: 1px solid var(--bb-border-warm-medium-strong);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-strong);
  font: inherit;
  font-size: 12px;
}

.task-graph-input-row select {
  padding-right: 24px;
}

.task-graph-input-cell-reference .task-graph-variable-chip {
  display: flex;
  align-items: center;
}

.task-graph-input-remove {
  box-sizing: border-box;
  width: 30px;
  min-width: 30px;
  height: 30px;
  min-height: 30px;
  align-self: end;
  padding: 0;
}

.task-graph-variable-chip {
  border: 1px solid var(--task-graph-reference-chip-border);
  border-radius: 8px;
  background: var(--task-graph-reference-chip-bg);
  color: var(--task-graph-reference-chip-text);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 11px;
  font-weight: 760;
}

@media (max-width: 720px) {
  .task-graph-input-row {
    min-width: 604px;
  }

  .task-graph-inputs-drawer .task-graph-input-row {
    grid-template-columns: minmax(0, 1fr) 30px;
    grid-template-areas:
      "id remove"
      "label label"
      "type type"
      "default default"
      "reference reference";
    min-width: 0;
  }
}
</style>
