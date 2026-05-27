<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import AppNav from '@/components/AppNav.vue'
import { BbButton } from '@/components/common'
import TicketStructuredDocument from '@/components/TicketStructuredDocument.vue'
import type { BlackboardPayload, LaneDef, TicketSpec } from '@/data/tickets'
import {
  boardRoute,
  loadBlackboardData,
  loadLanes,
  loadTicketDetail,
  patchTicket,
  resolveLaneMeta,
  ticketRoute,
} from '@/data/tickets'
import { ticketStatusLabel, t } from '@/i18n'

const props = defineProps<{ project: string; id: string }>()

const router = useRouter()
const payload = ref<BlackboardPayload | null>(null)
const lanes = ref<LaneDef[]>([])
const ticketDetail = ref<BlackboardPayload['tickets'][number] | null>(null)
const loading = ref(true)
const error = ref('')
const detailLoading = ref(false)
const detailError = ref('')
const specSaving = ref(false)

const indexedTicket = computed(() => payload.value?.tickets.find((item) => item.id === props.id))
const ticket = computed(() => ticketDetail.value ?? indexedTicket.value)
const laneMeta = computed(() =>
  ticket.value ? resolveLaneMeta(ticket.value.lane, lanes.value) : null,
)
const relatedTickets = computed(() => {
  if (!ticket.value || !payload.value) return []
  return ticket.value.dependencies
    .map((id) => payload.value!.tickets.find((item) => item.id === id))
    .filter(Boolean)
})

async function reload(project: string) {
  loading.value = true
  error.value = ''
  payload.value = null
  ticketDetail.value = null
  detailError.value = ''
  detailLoading.value = true
  try {
    // Tickets and lanes are independent reads, kick them off in parallel so
    // the detail panel still renders if one fails.
    const [data, laneResult, detail] = await Promise.all([
      loadBlackboardData(project),
      loadLanes(project),
      loadTicketDetail(project, props.id),
    ])
    payload.value = data
    lanes.value = laneResult.data
    ticketDetail.value = detail
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
    detailLoading.value = false
  }
}

watch(
  () => [props.project, props.id] as const,
  ([project]) => {
    if (project) reload(project)
  },
  { immediate: true },
)

async function updateTicketSpec(spec: TicketSpec | undefined) {
  if (!ticket.value || !spec || specSaving.value) return
  const target = ticket.value
  const previousPayload = payload.value
  const previousDetail = ticketDetail.value
  specSaving.value = true
  detailError.value = ''
  ticketDetail.value = { ...target, spec, updated_at: new Date().toISOString().slice(0, 10) }
  if (payload.value) {
    payload.value = {
      ...payload.value,
      tickets: payload.value.tickets.map((item) =>
        item.id === target.id ? { ...item, spec, updated_at: new Date().toISOString().slice(0, 10) } : item,
      ),
    }
  }

  try {
    const result = await patchTicket(props.project, target.id, { spec })
    ticketDetail.value = {
      ...target,
      ...result.ticket,
      file_path: result.ticket.path,
      dependencies: target.dependencies,
      attachments: result.ticket.attachments,
      spec: result.ticket.spec ?? spec,
      extra: result.ticket.extra,
    }
  } catch (err) {
    payload.value = previousPayload
    ticketDetail.value = previousDetail
    detailError.value = err instanceof Error ? err.message : String(err)
  } finally {
    specSaving.value = false
  }
}
</script>

<template>
  <div>
    <AppNav
      :title="ticket ? `${ticket.id} · ${ticket.title}` : id"
      :subtitle="`${project} · ${t('ticketDetailSubtitle')}`"
    >
      <template #actions>
        <BbButton variant="secondary" @click="router.push(boardRoute(project))">
          {{ t('ticketBackToBoard') }}
        </BbButton>
      </template>
    </AppNav>

    <main class="page-content bb-page">
      <div v-if="loading" class="control-panel bb-empty">{{ t('ticketLoading') }}</div>
      <div v-else-if="error" class="control-panel bb-error">{{ error }}</div>
      <div v-else-if="!ticket" class="control-panel bb-error">{{ t('ticketNotFound') }}</div>

      <template v-else>
        <section class="ticket-detail-grid">
          <article class="control-panel ticket-document">
            <TicketStructuredDocument
              :ticket="ticket"
              :loading="detailLoading"
              :error="detailError"
              :spec-saving="specSaving"
              @spec-change="updateTicketSpec"
            />
          </article>

          <aside class="ticket-side">
            <div class="control-panel meta-panel">
              <div class="meta-id">{{ ticket.id }}</div>
              <div class="meta-title">{{ ticket.title }}</div>
              <div class="meta-row"><span>{{ t('ticketProject') }}</span><strong>{{ project }}</strong></div>
              <div class="meta-row"><span>{{ t('ticketLane') }}</span><strong :style="{ color: laneMeta?.color }">{{ ticket.lane }} · {{ laneMeta?.label }}</strong></div>
              <div class="meta-row"><span>{{ t('status') }}</span><strong>{{ ticketStatusLabel(ticket.status) }}</strong></div>
              <div class="meta-row"><span>{{ t('assignee') }}</span><strong>{{ ticket.extra.assignee || t('unassigned') }}</strong></div>
            </div>

            <div v-if="relatedTickets.length > 0" class="control-panel meta-panel">
              <h3>{{ t('dependencies') }}</h3>
              <button
                v-for="item in relatedTickets"
                :key="item!.id"
                class="dep-link"
                @click="router.push(ticketRoute(project, item!.id))"
              >
                {{ item!.id }} · {{ item!.title }}
              </button>
            </div>

          </aside>
        </section>
      </template>
    </main>
  </div>
</template>
