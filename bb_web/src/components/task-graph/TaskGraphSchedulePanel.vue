<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  CalendarClock,
  CirclePause,
  CirclePlay,
  ExternalLink,
  Play,
  Plus,
  Trash2,
} from 'lucide-vue-next'
import { t } from '@/i18n'
import {
  type TaskGraphDefinition,
  type TaskGraphRef,
  type TaskGraphSchedule,
  type TaskGraphScheduleCreateInput,
  type TaskGraphScheduleKind,
  type TaskGraphSchedulePatchInput,
} from '@/data/taskGraphs'

type SchedulePreset = '15m' | 'hourly' | 'daily' | 'weekly' | 'cron'
type MessageKey = Parameters<typeof t>[0]

const props = defineProps<{
  selectedGraph: TaskGraphDefinition | null
  selectedRef: TaskGraphRef | null
  schedules: TaskGraphSchedule[]
  loading: boolean
  runInputValues: Record<string, unknown>
  actionBusy: string
}>()

const emit = defineEmits<{
  create: [input: TaskGraphScheduleCreateInput]
  patch: [id: string, patch: TaskGraphSchedulePatchInput]
  delete: [id: string]
  'run-now': [id: string]
  reload: []
  'navigate-run': [runId: string]
}>()

const showCreate = ref(false)
const scheduleName = ref('')
const preset = ref<SchedulePreset>('daily')
const customCron = ref('0 9 * * *')
const localError = ref('')

const presets: Array<{ id: SchedulePreset; labelKey: MessageKey; kind: TaskGraphScheduleKind; expression: string }> = [
  { id: '15m', labelKey: 'taskGraphScheduleEvery15m', kind: 'interval', expression: '15m' },
  { id: 'hourly', labelKey: 'taskGraphScheduleHourly', kind: 'interval', expression: '1h' },
  { id: 'daily', labelKey: 'taskGraphScheduleDaily9', kind: 'daily', expression: '09:00' },
  { id: 'weekly', labelKey: 'taskGraphScheduleWeeklyMon9', kind: 'weekly', expression: 'mon 09:00' },
  { id: 'cron', labelKey: 'taskGraphScheduleCustomCron', kind: 'cron', expression: '' },
]

const selectedPreset = computed(() => presets.find((item) => item.id === preset.value) ?? presets[2])

const defaultScheduleName = computed(() => {
  const title = props.selectedGraph?.title || props.selectedRef?.id || 'TaskGraph'
  return `${title} ${t('taskGraphScheduleDefaultSuffix')}`
})

const scheduleCountLabel = computed(() => t('taskGraphScheduleCount', { count: String(props.schedules.length) }))

function isBusy(suffix: string) {
  return props.actionBusy === `schedule:${suffix}` || props.actionBusy.startsWith(`schedule:${suffix}:`)
}

function formatDateTime(value?: string) {
  if (!value) return '-'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return new Intl.DateTimeFormat(undefined, {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date)
}

function scheduleLabel(schedule: TaskGraphSchedule) {
  const expression = schedule.schedule.expression
  if (schedule.schedule.kind === 'interval') {
    if (expression === '15m') return t('taskGraphScheduleEvery15m')
    if (expression === '1h') return t('taskGraphScheduleHourly')
  }
  if (schedule.schedule.kind === 'daily') return t('taskGraphScheduleDailyAt', { time: expression })
  if (schedule.schedule.kind === 'weekly') return t('taskGraphScheduleWeeklyAt', { expression })
  return `cron ${expression}`
}

function statusLabel(schedule: TaskGraphSchedule) {
  if (!schedule.enabled) return t('taskGraphSchedulePaused')
  return schedule.state.last_status || t('taskGraphScheduleWaiting')
}

function nextRunLabel(schedule: TaskGraphSchedule) {
  return formatDateTime(schedule.state.next_run_at)
}

function lastRunLabel(schedule: TaskGraphSchedule) {
  if (!schedule.state.last_run_at) return '-'
  const status = schedule.state.last_status ?? t('taskGraphScheduleUnknown')
  return `${status} · ${formatDateTime(schedule.state.last_run_at)}`
}

function createSchedule() {
  localError.value = ''
  if (!props.selectedRef) return
  const expression = preset.value === 'cron'
    ? customCron.value.trim()
    : selectedPreset.value.expression
  if (!expression) {
    localError.value = t('taskGraphScheduleCronRequired')
    return
  }
  emit('create', {
    name: scheduleName.value.trim() || defaultScheduleName.value,
    graph_ref: props.selectedRef,
    input: { ...props.runInputValues },
    enabled: true,
    timezone: 'Asia/Shanghai',
    schedule: {
      kind: selectedPreset.value.kind,
      expression,
    },
  })
  showCreate.value = false
  scheduleName.value = ''
  preset.value = 'daily'
}

function toggleSchedule(schedule: TaskGraphSchedule) {
  emit('patch', schedule.id, { enabled: !schedule.enabled })
}

function deleteSchedule(schedule: TaskGraphSchedule) {
  if (typeof window !== 'undefined' && !window.confirm(t('taskGraphScheduleDeleteConfirm'))) return
  emit('delete', schedule.id)
}

watch(
  () => props.selectedRef?.id,
  () => {
    showCreate.value = false
    localError.value = ''
    scheduleName.value = ''
    preset.value = 'daily'
  },
)
</script>

<template>
  <section class="task-graph-schedules">
    <header>
      <div>
        <h4>
          <CalendarClock aria-hidden="true" />
          <span>{{ t('taskGraphSchedules') }}</span>
        </h4>
        <small>{{ scheduleCountLabel }} · Asia/Shanghai</small>
      </div>
      <div class="task-graph-schedule-head-actions">
        <button type="button" :disabled="loading" @click="emit('reload')">{{ t('refresh') }}</button>
        <button
          type="button"
          :disabled="!selectedRef || !!actionBusy"
          @click="showCreate = !showCreate"
        >
          <Plus aria-hidden="true" />
          <span>{{ t('taskGraphScheduleCreate') }}</span>
        </button>
      </div>
    </header>

    <form v-if="showCreate && selectedRef" class="task-graph-schedule-form" @submit.prevent="createSchedule">
      <label>
        <span>{{ t('taskGraphScheduleName') }}</span>
        <input v-model="scheduleName" :placeholder="defaultScheduleName" />
      </label>
      <div class="task-graph-schedule-presets" role="group" :aria-label="t('taskGraphSchedulePreset')">
        <button
          v-for="item in presets"
          :key="item.id"
          type="button"
          :class="{ selected: preset === item.id }"
          @click="preset = item.id"
        >
          {{ t(item.labelKey) }}
        </button>
      </div>
      <label v-if="preset === 'cron'">
        <span>{{ t('taskGraphScheduleCron') }}</span>
        <input v-model="customCron" placeholder="0 9 * * *" />
      </label>
      <p v-if="localError" class="task-graph-schedule-error">{{ localError }}</p>
      <div class="task-graph-schedule-form-actions">
        <button type="button" @click="showCreate = false">{{ t('cancel') }}</button>
        <button type="submit" :disabled="isBusy('create')">{{ t('create') }}</button>
      </div>
    </form>

    <div v-if="loading" class="task-graph-schedule-empty">{{ t('loading') }}</div>
    <div v-else-if="!selectedRef" class="task-graph-schedule-empty">{{ t('taskGraphEmptyPreview') }}</div>
    <div v-else-if="schedules.length === 0" class="task-graph-schedule-empty">
      {{ t('taskGraphScheduleEmpty') }}
    </div>
    <div v-else class="task-graph-schedule-list">
      <article
        v-for="schedule in schedules"
        :key="schedule.id"
        class="task-graph-schedule-row"
        :data-enabled="schedule.enabled"
      >
        <div class="task-graph-schedule-main">
          <strong>{{ schedule.name }}</strong>
          <span>{{ scheduleLabel(schedule) }} · {{ schedule.id }}</span>
          <small v-if="schedule.state.last_error">{{ schedule.state.last_error }}</small>
        </div>
        <dl>
          <div>
            <dt>{{ t('status') }}</dt>
            <dd><span class="task-graph-schedule-status" :data-status="schedule.state.last_status || (schedule.enabled ? 'waiting' : 'paused')">{{ statusLabel(schedule) }}</span></dd>
          </div>
          <div>
            <dt>{{ t('taskGraphScheduleNextRun') }}</dt>
            <dd>{{ nextRunLabel(schedule) }}</dd>
          </div>
          <div>
            <dt>{{ t('taskGraphScheduleLastRun') }}</dt>
            <dd>{{ lastRunLabel(schedule) }}</dd>
          </div>
        </dl>
        <div class="task-graph-schedule-actions">
          <button
            type="button"
            :title="t('taskGraphScheduleRunNow')"
            :disabled="!!actionBusy"
            @click="emit('run-now', schedule.id)"
          >
            <Play aria-hidden="true" />
          </button>
          <button
            type="button"
            :title="schedule.enabled ? t('taskGraphSchedulePause') : t('taskGraphScheduleEnable')"
            :disabled="!!actionBusy"
            @click="toggleSchedule(schedule)"
          >
            <CirclePause v-if="schedule.enabled" aria-hidden="true" />
            <CirclePlay v-else aria-hidden="true" />
          </button>
          <button
            v-if="schedule.state.last_run_id"
            type="button"
            :title="t('taskGraphOpenRun')"
            @click="emit('navigate-run', schedule.state.last_run_id)"
          >
            <ExternalLink aria-hidden="true" />
          </button>
          <button
            type="button"
            :title="t('taskGraphScheduleDelete')"
            :disabled="!!actionBusy"
            @click="deleteSchedule(schedule)"
          >
            <Trash2 aria-hidden="true" />
          </button>
        </div>
      </article>
    </div>
  </section>
</template>

<style scoped>
.task-graph-schedules {
  display: grid;
  align-content: start;
  gap: 10px;
  min-width: 0;
  padding: 10px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-schedules > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.task-graph-schedules h4 {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin: 0;
  color: var(--bb-text-strong);
  font-size: 13px;
  font-weight: 780;
}

.task-graph-schedules h4 svg {
  width: 15px;
  height: 15px;
}

.task-graph-schedules small,
.task-graph-schedule-main span {
  color: var(--bb-text-muted);
  font-size: 12px;
}

.task-graph-schedule-head-actions,
.task-graph-schedule-form-actions,
.task-graph-schedule-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}

.task-graph-schedule-head-actions button,
.task-graph-schedule-form-actions button,
.task-graph-schedule-actions button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
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

.task-graph-schedule-actions button {
  width: 30px;
  padding: 0;
}

.task-graph-schedule-head-actions button:hover,
.task-graph-schedule-form-actions button:hover,
.task-graph-schedule-actions button:hover {
  border-color: var(--task-graph-accent-border-medium);
  color: var(--bb-accent);
}

.task-graph-schedule-head-actions button:disabled,
.task-graph-schedule-form-actions button:disabled,
.task-graph-schedule-actions button:disabled {
  cursor: not-allowed;
  opacity: 0.54;
}

.task-graph-schedule-head-actions svg,
.task-graph-schedule-actions svg {
  width: 14px;
  height: 14px;
}

.task-graph-schedule-form {
  display: grid;
  gap: 8px;
  padding: 9px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface-soft);
}

.task-graph-schedule-form label {
  display: grid;
  gap: 4px;
}

.task-graph-schedule-form label span {
  color: var(--bb-text-muted);
  font-size: 11px;
  font-weight: 760;
}

.task-graph-schedule-form input {
  box-sizing: border-box;
  width: 100%;
  min-height: 32px;
  padding: 6px 8px;
  border: 1px solid var(--bb-border-warm-medium-strong);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-strong);
  font: inherit;
  font-size: 12px;
}

.task-graph-schedule-presets {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.task-graph-schedule-presets button {
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

.task-graph-schedule-presets button.selected {
  border-color: var(--task-graph-accent-border-medium);
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
}

.task-graph-schedule-error {
  margin: 0;
  color: var(--bb-error);
  font-size: 12px;
}

.task-graph-schedule-empty {
  padding: 12px;
  border-radius: 8px;
  background: var(--bb-surface-soft);
  color: var(--bb-text-muted);
  font-size: 12px;
  text-align: center;
}

.task-graph-schedule-list {
  display: grid;
  gap: 8px;
}

.task-graph-schedule-row {
  display: grid;
  grid-template-columns: minmax(170px, 1fr) minmax(360px, 1.4fr) auto;
  align-items: center;
  gap: 10px;
  min-width: 0;
  padding: 9px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-schedule-row[data-enabled='false'] {
  opacity: 0.72;
}

.task-graph-schedule-main {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.task-graph-schedule-main strong {
  overflow-wrap: anywhere;
  color: var(--bb-text-strong);
  font-size: 12px;
  font-weight: 780;
}

.task-graph-schedule-main small {
  overflow-wrap: anywhere;
  color: var(--bb-error);
}

.task-graph-schedule-row dl {
  display: grid;
  grid-template-columns: repeat(3, minmax(96px, 1fr));
  gap: 6px;
  margin: 0;
}

.task-graph-schedule-row dl > div {
  min-width: 0;
  padding: 7px;
  border-radius: 8px;
  background: var(--bb-surface-soft);
}

.task-graph-schedule-row dt {
  color: var(--bb-text-muted);
  font-size: 10px;
  font-weight: 760;
}

.task-graph-schedule-row dd {
  margin: 3px 0 0;
  overflow-wrap: anywhere;
  color: var(--bb-text-strong);
  font-size: 12px;
  font-weight: 760;
}

.task-graph-schedule-status {
  display: inline-flex;
  align-items: center;
  min-height: 20px;
  padding: 0 7px;
  border-radius: 999px;
  background: var(--bb-surface);
  color: var(--bb-text-muted);
  font-size: 11px;
  font-weight: 820;
}

.task-graph-schedule-status[data-status='pending'],
.task-graph-schedule-status[data-status='running'],
.task-graph-schedule-status[data-status='waiting'] {
  background: color-mix(in srgb, var(--bb-focus) 12%, var(--bb-surface));
  color: var(--bb-focus);
}

.task-graph-schedule-status[data-status='succeeded'] {
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
}

.task-graph-schedule-status[data-status='failed'],
.task-graph-schedule-status[data-status='cancelled'] {
  background: color-mix(in srgb, var(--bb-error) 12%, var(--bb-surface));
  color: var(--bb-error);
}

.task-graph-schedule-status[data-status='paused'],
.task-graph-schedule-status[data-status='skipped'] {
  background: color-mix(in srgb, var(--bb-warning) 12%, var(--bb-surface));
  color: var(--bb-warning);
}

@media (max-width: 1100px) {
  .task-graph-schedule-row {
    grid-template-columns: 1fr;
  }

  .task-graph-schedule-actions {
    justify-content: flex-start;
  }
}

@media (max-width: 760px) {
  .task-graph-schedules > header,
  .task-graph-schedule-row dl {
    grid-template-columns: 1fr;
  }

  .task-graph-schedules > header {
    display: grid;
  }
}
</style>
