<script setup lang="ts">
import { computed } from 'vue'
import type { BlackboardTicket, LaneDef } from '@/data/tickets'
import {
  resolveLaneMeta,
} from '@/data/tickets'
import { t, ticketStatusLabel } from '@/i18n'

const props = defineProps<{
  project: string
  ticket: BlackboardTicket
  /// Lane catalog passed in from BoardView so individual cards do not have to
  /// re-fetch. Empty array is fine — `resolveLaneMeta` falls back to built-in
  /// metadata or the lane id itself.
  lanes?: LaneDef[]
  draggable?: boolean
  dragging?: boolean
  moving?: boolean
}>()

const emit = defineEmits<{
  open: []
  dragStart: [event: DragEvent]
  dragEnd: []
}>()

const lane = computed(() => resolveLaneMeta(props.ticket.lane, props.lanes ?? []))
const statusText = computed(() => ticketStatusLabel(props.ticket.status))

const progressPreview = computed(() => {
  const records = props.ticket.spec?.progress_record ?? []
  const latestRecord = records.length > 0 ? records[records.length - 1]?.summary : ''
  return latestRecord || props.ticket.spec?.summary || ''
})

// Assignee now lives in the open `extra` KV map. Fall back to `unassigned`
// both when the key is missing and when it is present but empty.
const assigneeLabel = computed(() => props.ticket.extra.assignee?.trim() || t('unassigned'))

function openFromClick() {
  if (props.moving) return
  emit('open')
}
</script>

<template>
  <article
    :class="['ticket-card', { dragging, moving }]"
    :style="{ '--family-color': lane.color }"
    :draggable="draggable"
    @click="openFromClick"
    @dragstart="emit('dragStart', $event)"
    @dragend="emit('dragEnd')"
  >
    <div class="ticket-card-top">
      <span class="ticket-id">{{ ticket.id }}</span>
      <span class="family-pill" :style="{ '--family-color': lane.color }">{{ ticket.lane }}</span>
    </div>
    <h3>{{ ticket.title }}</h3>
    <div v-if="progressPreview" class="ticket-progress-preview">
      <span>{{ t('currentProgress') }}</span>
      <p>{{ progressPreview }}</p>
    </div>
    <div class="ticket-meta-row">
      <span>{{ statusText }}</span>
      <span class="ticket-assignee-pill">{{ assigneeLabel }}</span>
    </div>
    <div v-if="ticket.dependencies.length > 0" class="ticket-deps">
      {{ t('dependencies') }} {{ ticket.dependencies.join(' / ') }}
    </div>
  </article>
</template>
