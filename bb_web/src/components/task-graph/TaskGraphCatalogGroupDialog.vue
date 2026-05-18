<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Check, FolderPlus, Pencil, Trash2, X } from 'lucide-vue-next'
import { BbActionGroup, BbButton, BbField } from '@/components/common'
import { t } from '@/i18n'
import type { TaskGraphCatalogGroupKind } from '@/data/taskGraphs'

const props = defineProps<{
  mode: 'create' | 'rename' | 'delete'
  kind?: TaskGraphCatalogGroupKind
  initialTitle?: string
  fallbackTitle?: string
  busy: boolean
  error: string
}>()

const emit = defineEmits<{
  close: []
  submit: [title?: string]
}>()

const title = ref('')
const titleInput = ref<HTMLInputElement | null>(null)

watch(
  () => props.initialTitle,
  (value) => {
    title.value = value ?? ''
  },
  { immediate: true },
)

const isDelete = computed(() => props.mode === 'delete')
const icon = computed(() => {
  if (props.mode === 'create') return FolderPlus
  if (props.mode === 'rename') return Pencil
  return Trash2
})
const dialogTitle = computed(() => {
  if (props.mode === 'create') return props.kind === 'system' ? t('taskGraphGroupCreateSystem') : t('taskGraphGroupCreateProject')
  if (props.mode === 'rename') return t('taskGraphGroupRename')
  return t('taskGraphGroupDelete')
})
const description = computed(() => {
  if (props.mode === 'create') return t('taskGraphGroupDescriptionCreate')
  if (props.mode === 'rename') return t('taskGraphGroupDescriptionRename')
  return t('taskGraphGroupDescriptionDelete', { title: props.fallbackTitle ?? t('unassigned') })
})
const submitLabel = computed(() => {
  if (props.mode === 'create') return props.busy ? t('saving') : t('create')
  if (props.mode === 'rename') return props.busy ? t('saving') : t('save')
  return props.busy ? t('saving') : t('ticketDelete')
})

function submitDialog() {
  if (isDelete.value) {
    emit('submit')
    return
  }
  emit('submit', title.value.trim())
}

defineExpose({
  focusInput: () => titleInput.value?.focus(),
})
</script>

<template>
  <Teleport to="body">
    <div class="task-graph-group-dialog-backdrop" role="presentation" @click.self="emit('close')">
      <form
        class="task-graph-group-dialog"
        :data-mode="mode"
        role="dialog"
        aria-modal="true"
        aria-labelledby="task-graph-group-dialog-title"
        aria-describedby="task-graph-group-dialog-description"
        @submit.prevent="submitDialog"
        @keydown.esc="emit('close')"
      >
        <header>
          <div class="task-graph-group-dialog-heading">
            <component :is="icon" aria-hidden="true" />
            <div>
              <h3 id="task-graph-group-dialog-title">{{ dialogTitle }}</h3>
              <p id="task-graph-group-dialog-description">{{ description }}</p>
            </div>
          </div>
          <BbButton type="button" variant="secondary" size="sm" icon-only :title="t('close')" :disabled="busy" @click="emit('close')">
            <X aria-hidden="true" />
          </BbButton>
        </header>

        <section v-if="!isDelete" class="task-graph-group-dialog-fields">
          <BbField :label="t('taskGraphGroupNameLabel')">
            <input
              ref="titleInput"
              v-model="title"
              autocomplete="off"
              required
              :placeholder="t('taskGraphCatalogGroupPlaceholder')"
            />
          </BbField>
        </section>
        <section v-else class="task-graph-group-dialog-confirm">
          <p>{{ t('taskGraphGroupDeleteConfirm', { title: initialTitle ?? '' }) }}</p>
        </section>

        <p v-if="error" class="task-graph-group-dialog-error" role="alert">{{ error }}</p>

        <footer>
          <BbActionGroup gap="sm">
            <BbButton type="button" variant="secondary" :disabled="busy" @click="emit('close')">
              {{ t('close') }}
            </BbButton>
            <BbButton type="submit" :variant="isDelete ? 'danger' : 'primary'" :disabled="busy">
              <template #leading>
                <component :is="isDelete ? Trash2 : Check" aria-hidden="true" />
              </template>
              {{ submitLabel }}
            </BbButton>
          </BbActionGroup>
        </footer>
      </form>
    </div>
  </Teleport>
</template>

<style scoped>
.task-graph-group-dialog-backdrop {
  position: fixed;
  inset: 0;
  z-index: 90;
  display: grid;
  place-items: start center;
  padding: clamp(72px, 14vh, 132px) 18px 24px;
  background: var(--task-graph-backdrop-bg);
  backdrop-filter: blur(2px);
}

.task-graph-group-dialog {
  box-sizing: border-box;
  display: grid;
  gap: 18px;
  width: min(560px, 100%);
  min-width: 0;
  padding: 20px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text);
  box-shadow: var(--bb-shadow-popover);
}

.task-graph-group-dialog header,
.task-graph-group-dialog footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
}

.task-graph-group-dialog-heading {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  align-items: start;
  gap: 10px;
  min-width: 0;
}

.task-graph-group-dialog-heading > svg {
  width: 18px;
  height: 18px;
  margin-top: 1px;
  color: var(--bb-theme-primary);
}

.task-graph-group-dialog[data-mode='delete'] .task-graph-group-dialog-heading > svg {
  color: var(--bb-error);
}

.task-graph-group-dialog h3 {
  margin: 0;
  color: var(--bb-text-strong);
  font-size: 16px;
  line-height: 1.25;
}

.task-graph-group-dialog header p,
.task-graph-group-dialog-confirm p {
  margin: 5px 0 0;
  color: var(--bb-text-muted);
  font-size: 12px;
  line-height: 1.45;
}

.task-graph-group-dialog-fields {
  display: grid;
  gap: 12px;
  min-width: 0;
}

.task-graph-group-dialog-fields input {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  min-height: 38px;
  padding: 7px 10px;
  border: 1px solid var(--bb-border-warm-medium-strong);
  border-radius: 8px;
  background: var(--bb-surface-soft);
  color: var(--bb-text-strong);
  font: inherit;
  font-size: 13px;
}

.task-graph-group-dialog-confirm {
  padding: 10px 12px;
  border: 1px solid color-mix(in srgb, var(--bb-warning) 24%, transparent);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-warning) 8%, var(--bb-surface));
}

.task-graph-group-dialog-confirm p {
  margin: 0;
  color: var(--bb-text);
}

.task-graph-group-dialog-error {
  margin: 0;
  padding: 8px 10px;
  border: 1px solid var(--task-graph-error-border);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-error) 8%, var(--bb-surface));
  color: var(--bb-error);
  font-size: 12px;
  line-height: 1.45;
}

.task-graph-group-dialog footer {
  justify-content: flex-end;
}
</style>
