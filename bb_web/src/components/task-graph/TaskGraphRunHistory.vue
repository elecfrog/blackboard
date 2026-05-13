<script setup lang="ts">
import { RefreshCw } from 'lucide-vue-next'
import { computed } from 'vue'
import { t } from '@/i18n'
import { type TaskGraphRef, type TaskGraphRunSummary } from '@/data/taskGraphs'

const props = defineProps<{
  runs: TaskGraphRunSummary[]
  source: 'rest' | 'mock'
  loading: boolean
}>()

const emit = defineEmits<{
  reload: []
  open: [run: TaskGraphRunSummary]
}>()

function runGraphRef(run: TaskGraphRunSummary): TaskGraphRef & { version?: number } {
  return (run.graph_ref ?? run.graph) as TaskGraphRef & { version?: number }
}

function isActiveRun(run: TaskGraphRunSummary) {
  return run.status === 'pending' || run.status === 'running' || run.status === 'paused'
}

function formatDateTime(value?: string) {
  if (!value) return '-'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return new Intl.DateTimeFormat(undefined, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  }).format(date)
}

function durationLabel(run: TaskGraphRunSummary) {
  const start = new Date(run.started_at ?? run.created_at).getTime()
  const endSource = run.completed_at ?? (isActiveRun(run) ? new Date().toISOString() : run.updated_at)
  const end = new Date(endSource).getTime()
  if (Number.isNaN(start) || Number.isNaN(end) || end < start) return '-'
  const totalSeconds = Math.max(0, Math.round((end - start) / 1000))
  const seconds = totalSeconds % 60
  const totalMinutes = Math.floor(totalSeconds / 60)
  const minutes = totalMinutes % 60
  const hours = Math.floor(totalMinutes / 60)
  if (hours > 0) return `${hours}h ${minutes}m ${seconds}s`
  if (minutes > 0) return `${minutes}m ${seconds}s`
  return `${seconds}s`
}

function runIdLabel(runId: string) {
  return runId.replace(/^run-/, '#')
}
</script>

<template>
  <section class="task-graph-run-history">
    <header>
      <div>
        <h4>{{ t('taskGraphRunHistory') }}</h4>
        <span>{{ source === 'mock' ? t('taskGraphMockSource') : t('taskGraphRestSource') }} · {{ runs.length }}</span>
      </div>
      <button type="button" :disabled="loading" @click="emit('reload')">
        <RefreshCw aria-hidden="true" />
        {{ t('refresh') }}
      </button>
    </header>
    <div v-if="runs.length === 0" class="task-graph-run-history-empty">
      {{ t('taskGraphRunHistoryEmpty') }}
    </div>
    <div v-else class="task-graph-run-history-table">
      <table>
        <thead>
          <tr>
            <th>{{ t('taskGraphRunId') }}</th>
            <th>{{ t('status') }}</th>
            <th>{{ t('taskGraphRunStartedAt') }}</th>
            <th>{{ t('taskGraphRunCompletedAt') }}</th>
            <th>{{ t('taskGraphRunDuration') }}</th>
            <th>{{ t('taskGraphVersion') }}</th>
            <th>{{ t('taskGraphRunAction') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="run in runs"
            :key="run.id"
            :class="{ active: isActiveRun(run) }"
          >
            <td>{{ runIdLabel(run.id) }}</td>
            <td>
              <span class="task-graph-run-status-pill" :data-status="run.status">{{ run.status }}</span>
            </td>
            <td>{{ formatDateTime(run.started_at ?? run.created_at) }}</td>
            <td>{{ formatDateTime(run.completed_at) }}</td>
            <td>{{ durationLabel(run) }}</td>
            <td>v{{ runGraphRef(run).version ?? '-' }}</td>
            <td>
              <button type="button" @click="emit('open', run)">{{ t('taskGraphView') }}</button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>

<style scoped>
.task-graph-run-history {
  display: grid;
  align-content: start;
  gap: 8px;
  align-self: start;
  min-width: 0;
  padding: 10px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-run-history > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.task-graph-run-history h4 {
  margin: 0;
  color: var(--bb-text-strong);
  font-size: 13px;
}

.task-graph-run-history header button,
.task-graph-run-history-table button {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-height: 28px;
  padding: 0 8px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-muted);
  cursor: pointer;
  font-size: 12px;
  font-weight: 760;
}

.task-graph-run-history header button svg {
  width: 14px;
  height: 14px;
}

.task-graph-run-history-empty {
  padding: 12px;
  border-radius: 8px;
  background: var(--bb-surface-soft);
  color: var(--bb-text-muted);
  font-size: 12px;
  text-align: center;
}

.task-graph-run-history-table {
  overflow-x: auto;
}

.task-graph-run-history-table table {
  width: 100%;
  min-width: 780px;
  border-collapse: collapse;
  font-size: 12px;
}

.task-graph-run-history-table th,
.task-graph-run-history-table td {
  padding: 9px 8px;
  border-bottom: 1px solid var(--bb-border-warm);
  color: var(--bb-text-muted);
  text-align: left;
  white-space: nowrap;
}

.task-graph-run-history-table th {
  color: var(--bb-text-muted);
  font-weight: 760;
}

.task-graph-run-history-table tr.active {
  background: var(--bb-accent-soft);
}

.task-graph-run-status-pill {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 8px;
  border-radius: 999px;
  background: var(--bb-surface-soft);
  color: var(--bb-text-muted);
  font-weight: 820;
}

.task-graph-run-status-pill[data-status='running'],
.task-graph-run-status-pill[data-status='pending'] {
  background: color-mix(in srgb, var(--bb-focus) 12%, var(--bb-surface));
  color: var(--bb-focus);
}

.task-graph-run-status-pill[data-status='paused'] {
  background: color-mix(in srgb, var(--bb-warning) 12%, var(--bb-surface));
  color: var(--bb-warning);
}

.task-graph-run-status-pill[data-status='succeeded'] {
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
}

.task-graph-run-status-pill[data-status='failed'],
.task-graph-run-status-pill[data-status='cancelled'] {
  background: color-mix(in srgb, var(--bb-error) 12%, var(--bb-surface));
  color: var(--bb-error);
}
</style>
