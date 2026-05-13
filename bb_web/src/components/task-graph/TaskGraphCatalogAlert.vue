<script setup lang="ts">
import { AlertCircle, X } from 'lucide-vue-next'
import { t } from '@/i18n'

defineProps<{
  tone: 'error' | 'warning'
  message: string
}>()

const emit = defineEmits<{
  close: []
}>()
</script>

<template>
  <Teleport to="body">
    <div
      class="task-graph-alert-backdrop"
      role="presentation"
      @click.self="emit('close')"
    >
      <section
        class="task-graph-alert-dialog"
        :data-tone="tone"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="task-graph-alert-title"
      >
        <header>
          <AlertCircle aria-hidden="true" />
          <h3 id="task-graph-alert-title">{{ t('taskGraphAlertTitle') }}</h3>
          <button type="button" :title="t('close')" @click="emit('close')">
            <X aria-hidden="true" />
          </button>
        </header>
        <p>{{ message }}</p>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.task-graph-alert-backdrop {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: grid;
  place-items: start center;
  padding: 18vh 18px 18px;
  background: var(--task-graph-strip-bg);
}

.task-graph-alert-dialog {
  width: min(520px, 100%);
  overflow: hidden;
  border: 1px solid color-mix(in srgb, var(--bb-error) 28%, transparent);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text);
  box-shadow: var(--bb-shadow-popover);
}

.task-graph-alert-dialog[data-tone='warning'] {
  border-color: color-mix(in srgb, var(--bb-warning) 34%, transparent);
}

.task-graph-alert-dialog > header {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  padding: 12px;
  border-bottom: 1px solid var(--bb-hairline);
  color: var(--bb-error);
}

.task-graph-alert-dialog[data-tone='warning'] > header {
  color: var(--bb-warning);
}

.task-graph-alert-dialog h3 {
  margin: 0;
  color: var(--bb-text-strong);
  font-size: 15px;
}

.task-graph-alert-dialog p {
  margin: 0;
  padding: 12px;
  color: var(--bb-text);
  font-size: 13px;
  line-height: 1.55;
  overflow-wrap: anywhere;
}

.task-graph-alert-dialog svg {
  width: 18px;
  height: 18px;
}

.task-graph-alert-dialog button {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-muted);
  cursor: pointer;
}
</style>
