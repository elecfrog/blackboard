<script setup lang="ts">
import { computed } from 'vue'
import { t } from '@/i18n'
import type {
  TaskGraphRunEvent,
  TaskGraphToolLifecycleArtifact,
  TaskGraphToolLifecycleError,
  TaskGraphToolLifecyclePayload,
  TaskGraphToolLifecycleStatus,
} from '@/data/taskGraphs'

const props = defineProps<{
  events: TaskGraphRunEvent[]
  selectedNodeId?: string
}>()

type ToolLifecycleEventKind = 'tool_start' | 'tool_update' | 'tool_end'

interface ToolLifecycleEventRecord {
  event: TaskGraphRunEvent
  payload: TaskGraphToolLifecyclePayload
}

interface ToolTimelineItem {
  id: string
  nodeId: string
  nodeRunId: string
  toolCallId: string
  toolName: string
  toolKind: string
  attempt: number
  status: TaskGraphToolLifecycleStatus
  superstep: number
  firstSeq: number
  lastSeq: number
  updateCount: number
  startedAt?: string
  endedAt?: string
  durationMs?: number
  inputSummary?: unknown
  outputSummary?: unknown
  error?: TaskGraphToolLifecycleError
  artifacts: TaskGraphToolLifecycleArtifact[]
}

const toolEventKinds = new Set<ToolLifecycleEventKind>(['tool_start', 'tool_update', 'tool_end'])

const lifecycleEvents = computed<ToolLifecycleEventRecord[]>(() => {
  const records: ToolLifecycleEventRecord[] = []
  for (const event of props.events) {
    if (!toolEventKinds.has(event.kind as ToolLifecycleEventKind)) continue
    const payload = toolLifecyclePayload(event.payload)
    if (!payload) continue
    records.push({ event, payload })
  }
  return records.sort((left, right) => left.event.seq - right.event.seq)
})

const timelineItems = computed<ToolTimelineItem[]>(() => {
  const byAttempt = new Map<string, ToolTimelineItem>()
  for (const record of lifecycleEvents.value) {
    const payload = record.payload
    const attempt = payload.attempt || 1
    const key = `${payload.tool_call_id}:${attempt}`
    const existing = byAttempt.get(key)
    const item = existing ?? {
      id: key,
      nodeId: payload.node_id,
      nodeRunId: payload.node_run_id,
      toolCallId: payload.tool_call_id,
      toolName: payload.tool_name,
      toolKind: payload.tool_kind,
      attempt,
      status: payload.status,
      superstep: payload.superstep,
      firstSeq: record.event.seq,
      lastSeq: record.event.seq,
      updateCount: 0,
      artifacts: [],
    }

    item.status = payload.status
    item.superstep = Math.max(item.superstep, payload.superstep)
    item.lastSeq = Math.max(item.lastSeq, record.event.seq)
    if (record.event.kind === 'tool_update') item.updateCount += 1
    if (payload.started_at) item.startedAt = payload.started_at
    if (payload.ended_at) item.endedAt = payload.ended_at
    if (payload.duration_ms !== undefined) item.durationMs = payload.duration_ms
    if (payload.input_summary !== undefined) item.inputSummary = payload.input_summary
    if (payload.output_summary !== undefined) item.outputSummary = payload.output_summary
    if (payload.error) item.error = payload.error
    for (const artifact of payload.artifacts ?? []) {
      if (!item.artifacts.some((existingArtifact) => existingArtifact.path === artifact.path)) {
        item.artifacts.push(artifact)
      }
    }
    byAttempt.set(key, item)
  }

  return [...byAttempt.values()]
    .map((item) => ({
      ...item,
      durationMs: item.durationMs ?? inferredDurationMs(item.startedAt, item.endedAt),
    }))
    .sort((left, right) => right.lastSeq - left.lastSeq)
})

const visibleItems = computed(() => {
  const selectedNodeId = props.selectedNodeId
  if (!selectedNodeId) return timelineItems.value
  return timelineItems.value.filter((item) => item.nodeId === selectedNodeId)
})

const displayedItems = computed(() => visibleItems.value.slice(0, 12))

const omittedCount = computed(() => Math.max(0, visibleItems.value.length - displayedItems.value.length))

const title = computed(() =>
  props.selectedNodeId ? t('taskGraphToolLifecycleForNode') : t('taskGraphToolLifecycle'),
)

const countLabel = computed(() =>
  t('taskGraphToolLifecycleCount', {
    visible: visibleItems.value.length,
    total: timelineItems.value.length,
  }),
)

const emptyLabel = computed(() =>
  props.selectedNodeId ? t('taskGraphToolLifecycleEmptyForNode') : t('taskGraphToolLifecycleEmpty'),
)

function toolLifecyclePayload(value: unknown): TaskGraphToolLifecyclePayload | null {
  if (!isRecord(value)) return null
  if (typeof value.tool_call_id !== 'string') return null
  if (typeof value.tool_name !== 'string') return null
  if (typeof value.tool_kind !== 'string') return null
  if (typeof value.node_id !== 'string') return null
  if (typeof value.node_run_id !== 'string') return null
  if (typeof value.schema_version !== 'number') return null
  return value as unknown as TaskGraphToolLifecyclePayload
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value)
}

function inferredDurationMs(startedAt?: string, endedAt?: string) {
  if (!startedAt || !endedAt) return undefined
  const start = Date.parse(startedAt)
  const end = Date.parse(endedAt)
  if (Number.isNaN(start) || Number.isNaN(end)) return undefined
  return Math.max(0, end - start)
}

function statusLabel(status: TaskGraphToolLifecycleStatus) {
  switch (status) {
    case 'running':
      return t('taskGraphToolStatusRunning')
    case 'succeeded':
      return t('taskGraphToolStatusSucceeded')
    case 'failed':
      return t('taskGraphToolStatusFailed')
    case 'cancelled':
      return t('taskGraphToolStatusCancelled')
    case 'timeout':
      return t('taskGraphToolStatusTimeout')
    case 'paused':
      return t('taskGraphToolStatusPaused')
    case 'skipped':
      return t('taskGraphToolStatusSkipped')
    default:
      return status
  }
}

function formatDuration(ms?: number) {
  if (ms === undefined) return ''
  if (ms < 1000) return `${ms}ms`
  const seconds = Math.round(ms / 1000)
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  const rest = seconds % 60
  return rest ? `${minutes}m ${rest}s` : `${minutes}m`
}

function humanizeCode(value: string) {
  return value.replace(/_/g, ' ')
}

function outputBrief(value: unknown) {
  if (!isRecord(value)) return shortText(value)
  if (typeof value.output_preview === 'string' && value.output_preview.trim()) {
    return shortText(value.output_preview)
  }
  if (typeof value.output_chars === 'number' && value.output_chars > 0) {
    return t('taskGraphToolOutputChars', { count: value.output_chars })
  }
  return summaryBrief(value, ['status', 'provider_status', 'exit_code'])
}

function inputBrief(value: unknown) {
  return summaryBrief(value, [
    'command_preview',
    'mcp_tool',
    'project',
    'id',
    'path',
    'file',
    'target',
    'provider_call_id',
  ])
}

function summaryBrief(value: unknown, keys: string[]) {
  if (!isRecord(value)) return shortText(value)
  const parts = keys
    .map((key) => {
      const entry = value[key]
      if (entry === undefined || entry === null || entry === '') return ''
      return `${key}: ${shortText(entry, 140)}`
    })
    .filter(Boolean)
  if (parts.length > 0) return parts.join(' · ')
  return ''
}

function errorBrief(error?: TaskGraphToolLifecycleError) {
  if (!error) return ''
  return `${error.code}: ${error.message}`
}

function shortText(value: unknown, maxChars = 260) {
  if (value === undefined || value === null) return ''
  const text = typeof value === 'string' ? value : JSON.stringify(value)
  if (text.length <= maxChars) return text
  return `${text.slice(0, maxChars)}...`
}
</script>

<template>
  <section class="task-graph-tool-timeline" role="region" :aria-label="title">
    <header>
      <div>
        <h5>{{ title }}</h5>
        <span>{{ countLabel }}</span>
      </div>
    </header>

    <p v-if="visibleItems.length === 0" class="task-graph-tool-empty">
      {{ emptyLabel }}
    </p>

    <div v-else class="task-graph-tool-list">
      <article
        v-for="item in displayedItems"
        :key="item.id"
        class="task-graph-tool-item"
        :data-status="item.status"
      >
        <header>
          <span class="task-graph-tool-status-dot" aria-hidden="true"></span>
          <div>
            <strong>{{ item.toolName }}</strong>
            <span>{{ humanizeCode(item.toolKind) }}</span>
          </div>
          <code>#{{ item.lastSeq }}</code>
        </header>

        <div class="task-graph-tool-meta">
          <span>{{ statusLabel(item.status) }}</span>
          <span>{{ t('taskGraphToolNode') }} {{ item.nodeId }}</span>
          <span>{{ t('taskGraphSuperstep') }} {{ item.superstep }}</span>
          <span>{{ t('taskGraphToolAttempt') }} {{ item.attempt }}</span>
          <span v-if="item.updateCount > 0">
            {{ t('taskGraphToolUpdates', { count: item.updateCount }) }}
          </span>
          <span v-if="formatDuration(item.durationMs)">
            {{ t('taskGraphToolDuration') }} {{ formatDuration(item.durationMs) }}
          </span>
        </div>

        <p v-if="inputBrief(item.inputSummary)" class="task-graph-tool-brief">
          <span>{{ t('taskGraphToolInput') }}</span>
          {{ inputBrief(item.inputSummary) }}
        </p>

        <p v-if="outputBrief(item.outputSummary)" class="task-graph-tool-brief">
          <span>{{ t('taskGraphToolOutput') }}</span>
          {{ outputBrief(item.outputSummary) }}
        </p>

        <p v-if="errorBrief(item.error)" class="task-graph-tool-error">
          <span>{{ t('taskGraphToolError') }}</span>
          {{ errorBrief(item.error) }}
        </p>

        <div v-if="item.artifacts.length > 0" class="task-graph-tool-artifacts">
          <span>{{ t('taskGraphToolArtifacts') }}</span>
          <code v-for="artifact in item.artifacts" :key="artifact.path">
            {{ artifact.path }}
          </code>
        </div>
      </article>

      <p v-if="omittedCount > 0" class="task-graph-tool-omitted">
        {{ t('taskGraphToolLifecycleOmitted', { count: omittedCount }) }}
      </p>
    </div>
  </section>
</template>

<style scoped>
.task-graph-tool-timeline {
  display: grid;
  gap: 7px;
  min-width: 0;
}

.task-graph-tool-timeline > header,
.task-graph-tool-timeline > header > div,
.task-graph-tool-item > header,
.task-graph-tool-meta,
.task-graph-tool-artifacts {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  min-width: 0;
}

.task-graph-tool-timeline h5 {
  margin: 0;
  color: var(--bb-text-strong);
}

.task-graph-tool-timeline > header span,
.task-graph-tool-empty,
.task-graph-tool-item code,
.task-graph-tool-item header span,
.task-graph-tool-meta span,
.task-graph-tool-omitted {
  color: var(--bb-text-muted);
  font-size: 12px;
}

.task-graph-tool-empty,
.task-graph-tool-omitted {
  margin: 0;
}

.task-graph-tool-list {
  display: grid;
  gap: 6px;
}

.task-graph-tool-item {
  display: grid;
  gap: 6px;
  min-width: 0;
  padding: 8px;
  border: 1px solid color-mix(in srgb, var(--bb-hairline) 88%, transparent);
  border-radius: 8px;
  background:
    linear-gradient(135deg, color-mix(in srgb, var(--bb-surface-muted) 72%, transparent), transparent),
    var(--bb-surface);
}

.task-graph-tool-item[data-status='running'] {
  border-color: color-mix(in srgb, var(--bb-focus) 24%, transparent);
}

.task-graph-tool-item[data-status='succeeded'] {
  border-color: color-mix(in srgb, var(--bb-success) 22%, transparent);
}

.task-graph-tool-item[data-status='failed'],
.task-graph-tool-item[data-status='timeout'] {
  border-color: color-mix(in srgb, var(--bb-error) 28%, transparent);
}

.task-graph-tool-item[data-status='cancelled'],
.task-graph-tool-item[data-status='paused'],
.task-graph-tool-item[data-status='skipped'] {
  border-color: color-mix(in srgb, var(--bb-warning) 26%, transparent);
}

.task-graph-tool-item > header {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  color: var(--bb-text-strong);
}

.task-graph-tool-item > header > div {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.task-graph-tool-item strong {
  color: var(--bb-text-strong);
  font-size: 12px;
  font-weight: 760;
  overflow-wrap: anywhere;
}

.task-graph-tool-status-dot {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  background: var(--bb-text-muted);
}

.task-graph-tool-item[data-status='running'] .task-graph-tool-status-dot {
  background: var(--bb-focus);
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--bb-focus) 14%, transparent);
}

.task-graph-tool-item[data-status='succeeded'] .task-graph-tool-status-dot {
  background: var(--bb-success);
}

.task-graph-tool-item[data-status='failed'] .task-graph-tool-status-dot,
.task-graph-tool-item[data-status='timeout'] .task-graph-tool-status-dot {
  background: var(--bb-error);
}

.task-graph-tool-item[data-status='cancelled'] .task-graph-tool-status-dot,
.task-graph-tool-item[data-status='paused'] .task-graph-tool-status-dot,
.task-graph-tool-item[data-status='skipped'] .task-graph-tool-status-dot {
  background: var(--bb-warning);
}

.task-graph-tool-meta span {
  display: inline-flex;
  min-width: 0;
  max-width: 100%;
  padding: 2px 6px;
  border-radius: 6px;
  background: var(--bb-surface-soft);
  overflow-wrap: anywhere;
}

.task-graph-tool-brief,
.task-graph-tool-error {
  display: grid;
  gap: 3px;
  margin: 0;
  color: var(--bb-text);
  font-size: 12px;
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.task-graph-tool-brief span,
.task-graph-tool-error span,
.task-graph-tool-artifacts > span {
  color: var(--bb-text-muted);
  font-size: 11px;
  font-weight: 760;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.task-graph-tool-error {
  color: var(--bb-error);
}

.task-graph-tool-artifacts code {
  max-width: 100%;
  padding: 2px 6px;
  border-radius: 6px;
  background: var(--bb-md-code-bg);
  color: var(--bb-md-code-text);
  overflow-wrap: anywhere;
}
</style>
