<script setup lang="ts">
import { ref } from 'vue'
import { Plus, X } from 'lucide-vue-next'
import { t } from '@/i18n'

defineProps<{
  busy: boolean
  error: string
}>()

const emit = defineEmits<{
  close: []
  create: []
  'update:title': [value: string]
}>()

const titleInput = ref<HTMLInputElement | null>(null)

defineExpose({
  focusInput: () => titleInput.value?.focus(),
})
</script>

<template>
  <Teleport to="body">
    <div
      v-if="true"
      class="task-graph-create-modal-backdrop"
      role="presentation"
      @click.self="emit('close')"
    >
      <form
        class="task-graph-create-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="task-graph-create-title"
        aria-describedby="task-graph-create-description"
        @submit.prevent="emit('create')"
        @keydown.esc="emit('close')"
      >
        <header>
          <div>
            <h3 id="task-graph-create-title">{{ t('taskGraphCreateDialogTitle') }}</h3>
            <p id="task-graph-create-description">{{ t('taskGraphCreateDialogDescription') }}</p>
          </div>
          <button type="button" :title="t('close')" :disabled="busy" @click="emit('close')">
            <X aria-hidden="true" />
          </button>
        </header>
        <section class="task-graph-create-modal-fields">
          <label>
            <span>{{ t('taskGraphCreateNameLabel') }}</span>
            <input
              ref="titleInput"
              autocomplete="off"
              required
              :placeholder="t('taskGraphCreatePlaceholder')"
              @input="emit('update:title', ($event.target as HTMLInputElement).value)"
            />
          </label>
        </section>
        <p v-if="error" class="task-graph-create-modal-error" role="alert">{{ error }}</p>
        <footer>
          <button type="button" class="bb-top-action-button" :disabled="busy" @click="emit('close')">
            <span>{{ t('close') }}</span>
          </button>
          <button type="submit" class="bb-top-action-button task-graph-create-submit" :disabled="busy">
            <Plus class="bb-top-action-svg" aria-hidden="true" />
            <span>{{ busy ? t('saving') : t('create') }}</span>
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>

<style scoped>
.task-graph-create-modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 90;
  display: grid;
  place-items: start center;
  padding: clamp(72px, 14vh, 132px) 18px 24px;
  background: var(--task-graph-backdrop-bg);
  backdrop-filter: blur(2px);
}

.task-graph-create-modal {
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

.task-graph-create-modal header,
.task-graph-create-modal footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
}

.task-graph-create-modal header > div {
  min-width: 0;
}

.task-graph-create-modal h3 {
  margin: 0;
  color: var(--bb-text-strong);
  font-size: 16px;
  line-height: 1.25;
}

.task-graph-create-modal header p {
  margin: 5px 0 0;
  color: var(--bb-text-muted);
  font-size: 12px;
  line-height: 1.45;
}

.task-graph-create-modal header button {
  display: grid;
  flex: 0 0 auto;
  place-items: center;
  width: 30px;
  height: 30px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-muted);
  cursor: pointer;
}

.task-graph-create-modal header button svg {
  width: 15px;
  height: 15px;
}

.task-graph-create-modal-fields {
  display: grid;
  gap: 12px;
  min-width: 0;
}

.task-graph-create-modal-fields label {
  display: grid;
  gap: 6px;
  min-width: 0;
}

.task-graph-create-modal-fields label span {
  color: var(--bb-text-muted);
  font-size: 12px;
  font-weight: 760;
}

.task-graph-create-modal-fields input {
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

.task-graph-create-modal-error {
  margin: 0;
  padding: 8px 10px;
  border: 1px solid var(--task-graph-error-border);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-error) 8%, var(--bb-surface));
  color: var(--bb-error);
  font-size: 12px;
  line-height: 1.45;
}

.task-graph-create-modal footer {
  justify-content: flex-end;
}

.task-graph-create-submit {
  border-color: var(--bb-text-strong);
  background: var(--bb-text-strong);
  color: var(--bb-surface);
}
</style>
