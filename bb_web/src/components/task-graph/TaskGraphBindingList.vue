<script setup lang="ts">
import { Trash2 } from 'lucide-vue-next'
import { BbButton, BbDenseRow, BbRefChip } from '@/components/common'
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
    <BbDenseRow
      v-for="(val, key) in bindings"
      :key="key"
      :columns="(readonlyKeys || !removable) ? 'minmax(92px, 0.9fr) minmax(0, 1.1fr)' : 'minmax(82px, 0.85fr) minmax(0, 1.15fr) 28px'"
      control-height="28px"
      padding="6px"
      class="task-graph-binding-row"
    >
      <label class="task-graph-binding-cell task-graph-binding-cell-key bb-dense-cell">
        <span class="bb-dense-cell-label">{{ t('taskGraphBindingKey') }}</span>
        <input
          class="bb-dense-control"
          :value="key"
          :disabled="readonly || readonlyKeys"
          :placeholder="t('taskGraphPlaceholderKey')"
          @change="emit('rename', String(key), inputValue($event))"
        />
      </label>
      <label class="task-graph-binding-cell task-graph-binding-cell-value bb-dense-cell">
        <span class="bb-dense-cell-label">{{ t('taskGraphBindingValue') }}</span>
        <input
          class="bb-dense-control"
          :value="bindingValueString(val)"
          :disabled="readonly"
          :placeholder="valuePlaceholder"
          @input="emit('update', String(key), inputValue($event))"
        />
      </label>
      <BbButton
        v-if="removable"
        class="task-graph-binding-remove"
        size="mini"
        variant="danger"
        icon-only
        :title="t('taskGraphBindingRemove')"
        :disabled="readonly"
        @click="emit('remove', String(key))"
      >
        <Trash2 aria-hidden="true" />
      </BbButton>
      <div v-if="inputIds.length > 0" class="task-graph-binding-actions">
        <BbRefChip
          v-for="inputId in inputIds"
          :key="inputId"
          size="sm"
          interactive
          :title="`${t('taskGraphBindingUseInput')}: ${inputId}`"
          :disabled="readonly"
          @click="emit('useInput', String(key), inputId)"
        >
          {{ inputId }}
        </BbRefChip>
      </div>
    </BbDenseRow>
    <BbButton
      v-if="addLabel"
      size="sm"
      variant="secondary"
      :disabled="readonly"
      @click="emit('add')"
    >
      {{ addLabel }}
    </BbButton>
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
  --bb-dense-control-font-size: 11px;
}

.task-graph-binding-remove {
  --bb-icon-button-size: 28px;
  align-self: end;
}

.task-graph-binding-actions {
  display: flex;
  grid-column: 1 / -1;
  flex-wrap: wrap;
  gap: 4px;
  min-width: 0;
}

</style>
