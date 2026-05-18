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
import { BbActionGroup, BbButton, BbEmptyState, BbField, BbInfoGrid, BbInfoItem, BbInlineAlert, BbStatusPill } from '@/components/common'

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
        <small>{{ scheduleCountLabel }} · {{ t('taskGraphScheduleTimezone') }}</small>
      </div>
      <BbActionGroup class="task-graph-schedule-head-actions" gap="xs">
        <BbButton size="sm" variant="secondary" :disabled="loading" @click="emit('reload')">
          {{ t('refresh') }}
        </BbButton>
        <BbButton
          size="sm"
          variant="secondary"
          :disabled="!selectedRef || !!actionBusy"
          @click="showCreate = !showCreate"
        >
          <template #leading>
            <Plus aria-hidden="true" />
          </template>
          <span>{{ t('taskGraphScheduleCreate') }}</span>
        </BbButton>
      </BbActionGroup>
    </header>

    <form v-if="showCreate && selectedRef" class="task-graph-schedule-form" @submit.prevent="createSchedule">
      <BbField :label="t('taskGraphScheduleName')">
        <input v-model="scheduleName" :placeholder="defaultScheduleName" />
      </BbField>
      <div class="task-graph-schedule-presets" role="group" :aria-label="t('taskGraphSchedulePreset')">
        <BbButton
          v-for="item in presets"
          :key="item.id"
          size="sm"
          :variant="preset === item.id ? 'primary' : 'secondary'"
          @click="preset = item.id"
        >
          {{ t(item.labelKey) }}
        </BbButton>
      </div>
      <BbField v-if="preset === 'cron'" :label="t('taskGraphScheduleCron')">
        <input v-model="customCron" :placeholder="t('taskGraphScheduleCronPlaceholder')" />
      </BbField>
      <BbInlineAlert v-if="localError" tone="error">{{ localError }}</BbInlineAlert>
      <BbActionGroup gap="xs">
        <BbButton variant="secondary" @click="showCreate = false">{{ t('cancel') }}</BbButton>
        <BbButton type="submit" variant="primary" :disabled="isBusy('create')">{{ t('create') }}</BbButton>
      </BbActionGroup>
    </form>

    <BbEmptyState v-if="loading" :message="t('loading')" />
    <BbEmptyState v-else-if="!selectedRef" :message="t('taskGraphEmptyPreview')" />
    <BbEmptyState v-else-if="schedules.length === 0" :message="t('taskGraphScheduleEmpty')" />
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
        <BbInfoGrid>
          <BbInfoItem :label="t('status')">
            <BbStatusPill
              :status="schedule.state.last_status || (schedule.enabled ? 'waiting' : 'paused')"
              :label="statusLabel(schedule)"
            />
          </BbInfoItem>
          <BbInfoItem :label="t('taskGraphScheduleNextRun')" :value="nextRunLabel(schedule)" />
          <BbInfoItem :label="t('taskGraphScheduleLastRun')" :value="lastRunLabel(schedule)" />
        </BbInfoGrid>
        <BbActionGroup class="task-graph-schedule-actions" gap="xs" :wrap="false">
          <BbButton
            size="mini"
            variant="secondary"
            icon-only
            :title="t('taskGraphScheduleRunNow')"
            :disabled="!!actionBusy"
            @click="emit('run-now', schedule.id)"
          >
            <Play aria-hidden="true" />
          </BbButton>
          <BbButton
            size="mini"
            variant="secondary"
            icon-only
            :title="schedule.enabled ? t('taskGraphSchedulePause') : t('taskGraphScheduleEnable')"
            :disabled="!!actionBusy"
            @click="toggleSchedule(schedule)"
          >
            <CirclePause v-if="schedule.enabled" aria-hidden="true" />
            <CirclePlay v-else aria-hidden="true" />
          </BbButton>
          <BbButton
            v-if="schedule.state.last_run_id"
            size="mini"
            variant="secondary"
            icon-only
            :title="t('taskGraphOpenRun')"
            @click="emit('navigate-run', schedule.state.last_run_id)"
          >
            <ExternalLink aria-hidden="true" />
          </BbButton>
          <BbButton
            size="mini"
            variant="danger"
            icon-only
            :title="t('taskGraphScheduleDelete')"
            :disabled="!!actionBusy"
            @click="deleteSchedule(schedule)"
          >
            <Trash2 aria-hidden="true" />
          </BbButton>
        </BbActionGroup>
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

.task-graph-schedule-form {
  display: grid;
  gap: 8px;
  padding: 9px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface-soft);
}

.task-graph-schedule-presets {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
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
  .task-graph-schedule-row .bb-info-grid {
    grid-template-columns: 1fr;
  }

  .task-graph-schedules > header {
    display: grid;
  }
}
</style>
