<script setup lang="ts">
import { RefreshCw } from 'lucide-vue-next'
import { computed } from 'vue'
import { BbButton, BbEmptyState, BbStatusPill } from '@/components/common'
import { t } from '@/i18n'
import { type TaskGraphRef, type TaskGraphRunSummary } from '@/data/taskGraphs'

const props = defineProps<{
  runs: TaskGraphRunSummary[]
  source: 'rest'
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
  return run.status === 'queued' || run.status === 'pending' || run.status === 'running' || run.status === 'paused'
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
        <span>{{ t('taskGraphRestSource') }} · {{ runs.length }}</span>
      </div>
      <BbButton size="sm" variant="secondary" :disabled="loading" @click="emit('reload')">
        <template #leading>
          <RefreshCw aria-hidden="true" />
        </template>
        {{ t('refresh') }}
      </BbButton>
    </header>
    <BbEmptyState v-if="runs.length === 0" :message="t('taskGraphRunHistoryEmpty')" />
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
              <BbStatusPill :status="run.status" />
            </td>
            <td>{{ formatDateTime(run.started_at ?? run.created_at) }}</td>
            <td>{{ formatDateTime(run.completed_at) }}</td>
            <td>{{ durationLabel(run) }}</td>
            <td>
              <span>v{{ runGraphRef(run).version ?? '-' }}</span>
              <span v-if="run.current_graph_revision !== undefined" class="task-graph-run-revision">
                rev {{ run.current_graph_revision }}
              </span>
            </td>
            <td>
              <BbButton size="sm" variant="secondary" @click="emit('open', run)">{{ t('taskGraphView') }}</BbButton>
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

.task-graph-run-revision {
  display: inline-flex;
  margin-left: 6px;
  color: var(--bb-text-muted);
  font-size: 11px;
  font-weight: 760;
}

</style>
