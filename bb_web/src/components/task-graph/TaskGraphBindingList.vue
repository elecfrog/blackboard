<script setup lang="ts">
import { Trash2 } from 'lucide-vue-next'
import { t } from '@/i18n'

const props = withDefaults(defineProps<{
  title: string
  bindings: Record<string, unknown>
  readonly?: boolean
  readonlyKeys?: boolean
  removable?: boolean
  inputIds?: string[]
  valuePlaceholder?: string
  addLabel?: string
}>(), {
  readonly: false,
  readonlyKeys: false,
  removable: true,
  inputIds: () => [],
  valuePlaceholder: '',
  addLabel: '',
})

const emit = defineEmits<{
  rename: [oldKey: string, newKey: string]
  update: [key: string, value: string]
  remove: [key: string]
  add: []
  useInput: [key: string, inputId: string]
}>()

function inputValue(event: Event) {
  return event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLSelectElement
    ? event.target.value
    : ''
}

function bindingValueString(value: unknown) {
  return typeof value === 'string' ? value : value == null ? '' : JSON.stringify(value)
}
</script>

<template>
  <div class="task-graph-binding-list">
    <span>{{ title }}</span>
    <div
      v-for="(val, key) in bindings"
      :key="key"
      class="task-graph-binding-row"
      :class="{ 'task-graph-binding-row-readonly': readonlyKeys || !removable }"
    >
      <label class="task-graph-binding-cell task-graph-binding-cell-key">
        <span>{{ t('taskGraphBindingKey') }}</span>
        <input
          :value="key"
          :disabled="readonly || readonlyKeys"
          placeholder="key"
          @change="emit('rename', String(key), inputValue($event))"
        />
      </label>
      <label class="task-graph-binding-cell task-graph-binding-cell-value">
        <span>{{ t('taskGraphBindingValue') }}</span>
        <input
          :value="bindingValueString(val)"
          :disabled="readonly"
          :placeholder="valuePlaceholder"
          @input="emit('update', String(key), inputValue($event))"
        />
      </label>
      <button
        v-if="removable"
        type="button"
        class="task-graph-binding-remove"
        :title="t('taskGraphBindingRemove')"
        :disabled="readonly"
        @click="emit('remove', String(key))"
      >
        <Trash2 aria-hidden="true" />
      </button>
      <div v-if="inputIds.length > 0" class="task-graph-binding-actions">
        <button
          v-for="inputId in inputIds"
          :key="inputId"
          type="button"
          class="task-graph-prompt-var-bind"
          :title="`${t('taskGraphBindingUseInput')}: ${inputId}`"
          :disabled="readonly"
          @click="emit('useInput', String(key), inputId)"
        >
          {{ inputId }}
        </button>
      </div>
    </div>
    <button
      v-if="addLabel"
      type="button"
      class="task-graph-inline-add"
      :disabled="readonly"
      @click="emit('add')"
    >
      {{ addLabel }}
    </button>
  </div>
</template>

<style scoped>
.task-graph-binding-list {
  display: grid;
  gap: 8px;
  min-width: 0;
}

.task-graph-binding-list > span {
  color: var(--bb-text-muted);
  font-size: 11px;
  font-weight: 760;
}

.task-graph-binding-row {
  display: grid;
  grid-template-columns: minmax(82px, 0.85fr) minmax(0, 1.15fr) 28px;
  align-items: end;
  gap: 6px;
  min-width: 0;
  padding: 6px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-binding-row-readonly {
  grid-template-columns: minmax(92px, 0.9fr) minmax(0, 1.1fr);
}

.task-graph-binding-cell {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.task-graph-binding-cell > span {
  overflow: hidden;
  color: var(--bb-text-muted);
  font-size: 10px;
  font-weight: 760;
  line-height: 1;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-graph-binding-row input {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  height: 28px;
  min-height: 28px;
  padding: 5px 7px;
  overflow: hidden;
  border: 1px solid var(--bb-border-warm-medium-strong);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-strong);
  font: inherit;
  font-size: 11px;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-graph-binding-remove {
  box-sizing: border-box;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  min-width: 28px;
  height: 28px;
  min-height: 28px;
  align-self: end;
  padding: 0;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-muted);
  cursor: pointer;
}

.task-graph-binding-remove svg {
  width: 13px;
  height: 13px;
}

.task-graph-binding-actions {
  display: flex;
  grid-column: 1 / -1;
  flex-wrap: wrap;
  gap: 4px;
  min-width: 0;
}

.task-graph-binding-actions .task-graph-prompt-var-bind {
  box-sizing: border-box;
  max-width: 100%;
  min-height: 24px;
  padding: 3px 6px;
  overflow: hidden;
  border: 1px solid var(--task-graph-accent-border-light);
  border-radius: 7px;
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 10px;
  font-weight: 760;
  line-height: 1.15;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}

.task-graph-inline-add {
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
</style>
