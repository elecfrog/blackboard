<script setup lang="ts">
import { Plus, Trash2 } from 'lucide-vue-next'
import type { TaskGraphInputParam } from '@/data/taskGraphs'
import { BbButton, BbDenseRow, BbEmptyState, BbRefChip } from '@/components/common'
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
      <BbButton size="mini" variant="secondary" icon-only :disabled="readonly" @click="emit('add')">
        <Plus aria-hidden="true" />
      </BbButton>
    </header>
    <BbEmptyState v-if="inputs.length === 0" :message="t('taskGraphInputsEmpty')" />
    <BbDenseRow
      v-for="(item, index) in inputs"
      :key="`${item.id}-${index}`"
      class="task-graph-input-row"
      :columns="layout === 'drawer' ? '' : 'minmax(112px, 1fr) minmax(128px, 1.1fr) minmax(108px, 0.9fr) minmax(64px, 0.5fr) minmax(132px, 1.1fr) 30px'"
    >
      <label class="task-graph-input-cell task-graph-input-cell-id bb-dense-cell">
        <span class="bb-dense-cell-label">{{ t('taskGraphInputId') }}</span>
        <input
          class="bb-dense-control"
          :value="item.id"
          :disabled="readonly"
          :placeholder="t('taskGraphInputIdPlaceholder')"
          @input="emit('update', index, { id: kebab(inputValue($event), `input-${index + 1}`) })"
        />
      </label>
      <label class="task-graph-input-cell task-graph-input-cell-label bb-dense-cell">
        <span class="bb-dense-cell-label">{{ t('label') }}</span>
        <input
          class="bb-dense-control"
          :value="item.label ?? ''"
          :disabled="readonly"
          :placeholder="t('taskGraphInputsLabelPlaceholder')"
          @input="emit('update', index, { label: inputValue($event) })"
        />
      </label>
      <label class="task-graph-input-cell task-graph-input-cell-type bb-dense-cell">
        <span class="bb-dense-cell-label">{{ t('taskGraphInputType') }}</span>
        <select
          class="bb-dense-control"
          :value="item.type"
          :disabled="readonly"
          @change="emit('typeChange', index, inputValue($event))"
        >
          <option v-for="type in inputTypeOptions" :key="type" :value="type">
            {{ type }}
          </option>
        </select>
      </label>
      <label class="task-graph-input-cell task-graph-input-cell-default bb-dense-cell">
        <span class="bb-dense-cell-label">{{ t('taskGraphInputDefault') }}</span>
        <input
          class="bb-dense-control"
          :value="inputDefaultText(item)"
          :disabled="readonly"
          :placeholder="t('taskGraphInputDefaultPlaceholder')"
          @input="emit('update', index, { default: coerceInputDefault(item.type, inputValue($event)) })"
        />
      </label>
      <div class="task-graph-input-cell task-graph-input-cell-reference bb-dense-cell">
        <span class="bb-dense-cell-label">{{ t('taskGraphInputReference') }}</span>
        <BbRefChip :value="inputReference(item.id)" />
      </div>
      <BbButton
        class="task-graph-input-remove"
        size="mini"
        variant="danger"
        icon-only
        :title="t('taskGraphInputRemove')"
        :disabled="readonly"
        @click="emit('remove', index)"
      >
        <Trash2 aria-hidden="true" />
      </BbButton>
    </BbDenseRow>
  </section>
</template>

<style scoped>
.task-graph-inputs {
  --bb-empty-state-padding: 10px;
  --bb-empty-state-font-size: 12px;
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
  --bb-dense-row-columns: minmax(82px, 0.8fr) minmax(108px, 1fr) minmax(94px, 0.75fr) 30px;
  grid-template-areas:
    "id label type remove"
    "default default reference reference";
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

.task-graph-input-row select {
  padding-right: 24px;
}

.task-graph-input-cell-reference .bb-ref-chip {
  display: flex;
  align-items: center;
}

.task-graph-input-remove {
  --bb-icon-button-size: 30px;
  align-self: end;
}

@media (max-width: 720px) {
  .task-graph-input-row {
    min-width: 604px;
  }

  .task-graph-inputs-drawer .task-graph-input-row {
    --bb-dense-row-columns: minmax(0, 1fr) 30px;
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
