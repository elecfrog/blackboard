<script setup lang="ts">
import {
  AlertCircle,
  CircleCheck,
  CirclePause,
  CircleX,
  GitFork,
  LoaderCircle,
  Lock,
  Pencil,
  Play,
  Workflow,
} from 'lucide-vue-next'
import { computed } from 'vue'
import BbDropdown, { type BbDropdownOption } from '@/components/BbDropdown.vue'
import { t } from '@/i18n'
import { type TaskGraphCatalogItem, type TaskGraphScope } from '@/data/taskGraphs'

const props = defineProps<{
  graphs: TaskGraphCatalogItem[]
  filter: 'all' | TaskGraphScope
  selectedRef: { scope: string; id: string } | null
  filterOptions: BbDropdownOption[]
  actionBusy: string
  project: string
}>()

const emit = defineEmits<{
  'update:filter': [value: string]
  open: [graph: TaskGraphCatalogItem]
  run: [graph: TaskGraphCatalogItem]
  customize: [graph: TaskGraphCatalogItem]
}>()

const filteredGraphs = computed(() =>
  props.graphs.filter((graph) => props.filter === 'all' || graph.scope === props.filter),
)

const systemGraphs = computed(() => filteredGraphs.value.filter((graph) => graph.scope === 'system'))
const projectGraphs = computed(() => filteredGraphs.value.filter((graph) => graph.scope === 'project'))

const catalogStats = computed(() => ({
  system: props.graphs.filter((graph) => graph.scope === 'system').length,
  project: props.graphs.filter((graph) => graph.scope === 'project').length,
}))

function isActiveRun(graph: TaskGraphCatalogItem) {
  return graph.last_run?.status === 'running' || graph.last_run?.status === 'pending' || graph.last_run?.status === 'paused'
}

function formatDate(value: string) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date)
}

function lastRunLabel(graph: TaskGraphCatalogItem) {
  const run = graph.last_run
  return run
    ? `${run.status} · ${formatDate(run.updated_at)}`
    : t('taskGraphNeverRun')
}

function runActionText(graph: TaskGraphCatalogItem) {
  return isActiveRun(graph) ? t('taskGraphOpenRun') : t('taskGraphRun')
}

function runActionStatus(graph: TaskGraphCatalogItem) {
  if (graph.compile_error) return 'compile_error'
  return graph.last_run?.status ?? 'idle'
}

function runActionIcon(graph: TaskGraphCatalogItem) {
  const status = runActionStatus(graph)
  if (status === 'compile_error') return AlertCircle
  if (status === 'pending' || status === 'running') return LoaderCircle
  if (status === 'paused') return CirclePause
  if (status === 'succeeded') return CircleCheck
  if (status === 'failed' || status === 'cancelled') return CircleX
  return Play
}
</script>

<template>
  <aside class="task-graph-catalog">
    <div class="task-graph-filters" :aria-label="t('taskGraphFilter')">
      <BbDropdown
        class="task-graph-filter-dropdown"
        :model-value="filter"
        :options="filterOptions"
        :label="t('taskGraphFilter')"
        :min-width="220"
        @change="emit('update:filter', $event)"
      />
    </div>

    <section v-if="systemGraphs.length > 0" class="task-graph-section">
      <header>
        <h3>{{ t('taskGraphSystem') }}</h3>
        <span>{{ systemGraphs.length }}</span>
      </header>
      <article
        v-for="graph in systemGraphs"
        :key="`${graph.scope}:${graph.id}`"
        :class="['task-graph-card', { selected: selectedRef?.scope === graph.scope && selectedRef?.id === graph.id }]"
      >
        <div class="task-graph-card-head">
          <button type="button" class="task-graph-card-main" @click="emit('open', graph)">
            <span class="task-graph-card-icon system">
              <Workflow aria-hidden="true" />
            </span>
            <span>
              <strong>{{ graph.title }}</strong>
              <span v-if="graph.compile_error" class="task-graph-card-state compile-error">
                <AlertCircle aria-hidden="true" />
                {{ t('taskGraphCompileError') }}
              </span>
              <span v-else-if="project !== 'blackboard'" class="task-graph-card-state readonly">
                <Lock aria-hidden="true" />
                {{ t('taskGraphReadonly') }}
              </span>
              <span v-else class="task-graph-card-state editable">
                <Pencil aria-hidden="true" />
                {{ t('taskGraphEditable') }}
              </span>
            </span>
          </button>
          <button
            type="button"
            class="task-graph-run-action"
            :data-status="runActionStatus(graph)"
            :title="graph.compile_error ? `${t('taskGraphCompileError')} · ${graph.compile_error}` : `${runActionText(graph)} · ${lastRunLabel(graph)}`"
            :aria-label="graph.compile_error ? `${t('taskGraphCompileError')} · ${graph.compile_error}` : `${runActionText(graph)} · ${lastRunLabel(graph)}`"
            :disabled="!!actionBusy || !!graph.compile_error"
            @click="emit('run', graph)"
          >
            <component :is="runActionIcon(graph)" aria-hidden="true" />
          </button>
        </div>
        <p>{{ graph.description }}</p>
      </article>
    </section>

    <section class="task-graph-section">
      <header>
        <h3>{{ t('taskGraphProject') }}</h3>
        <span>{{ projectGraphs.length }}</span>
      </header>
      <div v-if="projectGraphs.length === 0" class="task-graph-empty">{{ t('taskGraphNoProjectGraphs') }}</div>
      <article
        v-for="graph in projectGraphs"
        :key="`${graph.scope}:${graph.id}`"
        :class="['task-graph-card', { selected: selectedRef?.scope === graph.scope && selectedRef?.id === graph.id }]"
      >
        <div class="task-graph-card-head">
          <button type="button" class="task-graph-card-main" @click="emit('open', graph)">
            <span class="task-graph-card-icon project">
              <GitFork aria-hidden="true" />
            </span>
            <span>
              <strong>{{ graph.title }}</strong>
              <span class="task-graph-card-state editable">
                <Pencil aria-hidden="true" />
                {{ t('taskGraphEditable') }}
              </span>
            </span>
          </button>
          <button
            type="button"
            class="task-graph-run-action"
            :data-status="runActionStatus(graph)"
            :title="graph.compile_error ? `${t('taskGraphCompileError')} · ${graph.compile_error}` : `${runActionText(graph)} · ${lastRunLabel(graph)}`"
            :aria-label="graph.compile_error ? `${t('taskGraphCompileError')} · ${graph.compile_error}` : `${runActionText(graph)} · ${lastRunLabel(graph)}`"
            :disabled="!!actionBusy || !!graph.compile_error"
            @click="emit('run', graph)"
          >
            <component :is="runActionIcon(graph)" aria-hidden="true" />
          </button>
        </div>
        <p>{{ graph.description }}</p>
        <dl>
          <div>
            <dt>{{ t('taskGraphOrigin') }}</dt>
            <dd>{{ graph.origin ? `${graph.origin.scope}/${graph.origin.id}` : t('taskGraphNoOrigin') }}</dd>
          </div>
        </dl>
      </article>
    </section>
  </aside>
</template>

<style scoped>
.task-graph-catalog {
  display: grid;
  align-content: start;
  gap: 14px;
  padding: 12px;
  min-width: 0;
  min-height: 0;
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-filters {
  display: block;
}

.task-graph-filter-dropdown {
  width: 100%;
}

.task-graph-filter-dropdown :deep(.bb-popup-select-trigger) {
  min-height: 34px;
  padding: 4px 8px;
  border-radius: 8px;
}

.task-graph-filter-dropdown :deep(.bb-popup-select-badge) {
  flex-basis: 22px;
  width: 22px;
  height: 22px;
  border-radius: 6px;
  font-size: 11px;
}

.task-graph-filter-dropdown :deep(.bb-popup-select-copy strong) {
  font-size: 12px;
}

.task-graph-section {
  display: grid;
  gap: 8px;
}

.task-graph-section > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--bb-text-muted);
}

.task-graph-section h3 {
  margin: 0;
  color: var(--bb-text-strong);
  font-size: 14px;
}

.task-graph-card {
  display: grid;
  gap: 8px;
  padding: 9px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-card.selected {
  border-color: var(--bb-theme-primary-border-strong);
  background: var(--bb-theme-primary-soft-strong);
  box-shadow: inset 0 0 0 1px var(--bb-theme-primary-border);
}

.task-graph-card-head {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 36px;
  align-items: center;
  gap: 8px;
}

.task-graph-card-main {
  display: grid;
  grid-template-columns: 36px minmax(0, 1fr);
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  cursor: pointer;
  text-align: left;
}

.task-graph-card-main > span:last-child {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.task-graph-card-main strong {
  display: block;
  overflow-wrap: anywhere;
  color: var(--bb-text-strong);
  font-size: 12px;
  font-weight: 780;
  line-height: 1.22;
}

.task-graph-card p {
  display: none;
  margin: 0;
  line-height: 1.45;
}

.task-graph-card-icon {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  border-radius: 8px;
}

.task-graph-card-icon svg,
.task-graph-run-action svg,
.task-graph-card-state svg {
  width: 16px;
  height: 16px;
}

.task-graph-card-icon.system {
  background: color-mix(in srgb, var(--bb-focus) 12%, var(--bb-surface));
  color: var(--bb-focus);
}

.task-graph-card-icon.project {
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
}

.task-graph-card dl {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 6px;
  margin: 0;
}

.task-graph-card dl > div {
  min-width: 0;
  padding: 8px;
  border-radius: 8px;
  background: var(--bb-surface-soft);
}

.task-graph-card dd {
  margin: 2px 0 0;
  overflow-wrap: anywhere;
  color: var(--bb-text-strong);
  font-size: 12px;
  font-weight: 760;
}

.task-graph-run-action {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  padding: 0;
  border: 1px solid var(--task-graph-accent-border-light);
  border-radius: 8px;
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
  cursor: pointer;
}

.task-graph-run-action:hover {
  border-color: var(--task-graph-accent-border-medium);
  color: var(--bb-accent);
}

.task-graph-run-action:disabled {
  cursor: progress;
  opacity: 0.56;
}

.task-graph-run-action svg {
  color: currentColor;
}

.task-graph-run-action[data-status='running'] svg,
.task-graph-run-action[data-status='pending'] svg {
  animation: task-graph-spin 0.95s linear infinite;
}

.task-graph-run-action[data-status='running'],
.task-graph-run-action[data-status='pending'] {
  border-color: var(--task-graph-focus-border);
  background: color-mix(in srgb, var(--bb-focus) 12%, var(--bb-surface));
  color: var(--bb-focus);
}

.task-graph-run-action[data-status='paused'] {
  border-color: color-mix(in srgb, var(--bb-warning) 34%, var(--bb-hairline));
  background: color-mix(in srgb, var(--bb-warning) 12%, var(--bb-surface));
  color: var(--bb-warning);
}

.task-graph-run-action[data-status='succeeded'] {
  border-color: var(--task-graph-accent-border-medium);
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
}

.task-graph-run-action[data-status='failed'],
.task-graph-run-action[data-status='cancelled'] {
  border-color: var(--task-graph-error-border);
  background: color-mix(in srgb, var(--bb-error) 12%, var(--bb-surface));
  color: var(--bb-error);
}

@keyframes task-graph-spin {
  to {
    transform: rotate(360deg);
  }
}

.task-graph-card-state {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  width: max-content;
  max-width: 100%;
  min-height: 20px;
  padding: 0 7px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 820;
}

.task-graph-card-state.readonly {
  background: color-mix(in srgb, var(--bb-focus) 12%, var(--bb-surface));
  color: var(--bb-focus);
}

.task-graph-card-state.editable {
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
}

.task-graph-card-state.compile-error {
  background: color-mix(in srgb, var(--bb-error) 14%, var(--bb-surface));
  color: var(--bb-error);
}

.task-graph-empty {
  padding: 14px;
  border: 1px dashed var(--bb-border-warm-dashed);
  border-radius: 8px;
  background: var(--bb-surface-soft);
  color: var(--bb-text-muted);
  font-size: 12px;
  text-align: center;
}
</style>
