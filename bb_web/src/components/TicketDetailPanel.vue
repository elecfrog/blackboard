<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { MarkdownRenderer, TableOfContents } from '@/ui/markdown'
import { GitFork, X } from 'lucide-vue-next'
import UnifiedPopupSelect from '@/components/UnifiedPopupSelect.vue'
import type { ProjectAgentProfile } from '@/data/agents'
import type { BlackboardTicket, LaneDef } from '@/data/tickets'
import {
  extractProgressText,
  loadTicketContent,
  resolveLaneMeta,
  ticketStatusOrder,
  ticketRoute,
} from '@/data/tickets'
import { locale, t, ticketStatusLabel } from '@/i18n'

const props = defineProps<{
  project: string
  ticket: BlackboardTicket
  tickets: BlackboardTicket[]
  lanes: LaneDef[]
  agents: ProjectAgentProfile[]
  assigneeSaving?: boolean
  statusSaving?: boolean
}>()

const emit = defineEmits<{
  close: []
  assigneeChange: [value: string]
  statusChange: [value: string]
}>()

const router = useRouter()

const lane = computed(() => resolveLaneMeta(props.ticket.lane, props.lanes))
const progressPreview = computed(() => extractProgressText(props.ticket.content ?? ''))
const currentAssignee = computed(() => props.ticket.extra.assignee?.trim() || '')

// On-demand content loading: the list endpoint no longer returns ticket body.
// When the detail panel opens (or the ticket changes), fetch the content.
const ticketContent = ref('')
const contentLoading = ref(false)

async function fetchContent() {
  contentLoading.value = true
  try {
    ticketContent.value = await loadTicketContent(props.project, props.ticket.id)
  } catch {
    ticketContent.value = ''
  } finally {
    contentLoading.value = false
  }
}

watch(
  () => props.ticket.id,
  () => { fetchContent() },
  { immediate: true },
)
/// Workflow status options. Kept in lockstep with bb-pm's allowed workflow
/// status list (todo / in_progress / blocked / review / done / archived). The label
/// comes from i18n so locale switching stays coherent.
const statusOptions = computed(() => {
  const known: Array<{ value: string; label: string }> = ticketStatusOrder.map((value) => ({
    value,
    label: ticketStatusLabel(value),
  }))
  // Tolerate legacy/unknown status values on existing tickets: surface them
  // as a disabled-looking option so the user can still see and keep them.
  if (props.ticket.status && !ticketStatusOrder.includes(props.ticket.status as typeof ticketStatusOrder[number])) {
    known.unshift({
      value: props.ticket.status,
      label: `${props.ticket.status} · ${t('ticketLegacyStatus')}`,
    })
  }
  return known
})
const assigneeOptions = computed(() => {
  const known: Array<{ value: string; label: string }> = [
    { value: '', label: t('unassigned') },
    ...props.agents.map((agent) => ({
      value: agent.id,
      label: agent.id,
    })),
  ]
  if (
    currentAssignee.value &&
    !props.agents.some((agent) => agent.id === currentAssignee.value)
  ) {
    known.splice(1, 0, {
      value: currentAssignee.value,
      label: currentAssignee.value,
    })
  }
  return known
})
const relatedTickets = computed(() =>
  props.ticket.dependencies
    .map((id) => props.tickets.find((item) => item.id === id))
    .filter((item): item is BlackboardTicket => Boolean(item)),
)

function openRelated(id: string) {
  router.push(ticketRoute(props.project, id))
}

function openGraph() {
  router.push(`/projects/${props.project}/graph?focus=${props.ticket.id}`)
}

function onStatusChange(next: string) {
  if (next === props.ticket.status) return
  emit('statusChange', next)
}

function onAssigneeChange(next: string) {
  if (next === currentAssignee.value) return
  emit('assigneeChange', next)
}
</script>

<template>
  <div class="ticket-detail-overlay" role="dialog" aria-modal="true">
    <button class="ticket-detail-backdrop" type="button" :aria-label="t('close')" @click="$emit('close')" />
    <aside class="ticket-detail-panel">
      <header class="ticket-detail-head">
        <div>
          <div class="ticket-detail-kicker">
            <span class="ticket-id">{{ ticket.id }}</span>
            <span class="family-pill" :style="{ '--family-color': lane.color }">
              {{ ticket.lane }} · {{ lane.label }}
            </span>
          </div>
          <h2>{{ ticket.title }}</h2>
        </div>
        <div class="ticket-detail-head-actions">
          <button class="bb-top-action-button bb-top-action-button--compact" type="button" @click="openGraph">
            <GitFork class="bb-top-action-svg" aria-hidden="true" />
            {{ t('graph') }}
          </button>
          <button class="bb-icon-button" type="button" :aria-label="t('close')" @click="$emit('close')">
            <X class="bb-icon-glyph" aria-hidden="true" />
          </button>
        </div>
      </header>

      <section class="ticket-detail-meta-grid">
        <div class="ticket-detail-workflow-card">
          <span>{{ t('workflow') }}</span>
          <UnifiedPopupSelect
            :model-value="ticket.status"
            :options="statusOptions"
            :label="t('workflow')"
            :disabled="statusSaving"
            :class="[
              'ticket-detail-workflow-select',
              `status-${ticket.status}`,
            ]"
            @change="onStatusChange"
          />
        </div>
        <div class="ticket-detail-assignee-card">
          <span>{{ t('assignee') }}</span>
          <UnifiedPopupSelect
            :model-value="currentAssignee"
            :options="assigneeOptions"
            :label="t('assignee')"
            :disabled="assigneeSaving"
            class="ticket-detail-assignee-select"
            @change="onAssigneeChange"
          />
        </div>
        <div>
          <span>{{ t('update') }}</span>
          <strong>{{ ticket.updated_at || t('ticketDetailNoDate') }}</strong>
        </div>
      </section>

      <section v-if="progressPreview" class="ticket-detail-progress">
        <span>{{ t('currentProgress') }}</span>
        <p>{{ progressPreview }}</p>
      </section>

      <section v-if="relatedTickets.length > 0" class="ticket-detail-links">
        <h3>{{ t('dependencies') }}</h3>
        <button
          v-for="item in relatedTickets"
          :key="item.id"
          type="button"
          class="dep-link"
          @click="openRelated(item.id)"
        >
          {{ item.id }} · {{ item.title }}
        </button>
      </section>

      <div class="ticket-detail-content-grid">
        <article v-if="contentLoading" class="ticket-detail-document">
          <p>{{ t('loading') }}</p>
        </article>
        <template v-else>
          <article class="ticket-detail-document">
            <MarkdownRenderer :content="ticketContent" :locale="locale" />
          </article>
          <aside class="ticket-detail-toc">
            <TableOfContents :content="ticketContent" :locale="locale" />
          </aside>
        </template>
      </div>
    </aside>
  </div>
</template>
