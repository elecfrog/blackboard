<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { MarkdownRenderer, TableOfContents } from '@/ui/markdown'
import AppNav from '@/components/AppNav.vue'
import type { BlackboardPayload, LaneDef } from '@/data/tickets'
import {
  boardRoute,
  loadBlackboardData,
  loadLanes,
  loadTicketContent,
  resolveLaneMeta,
  ticketRoute,
} from '@/data/tickets'
import { locale, ticketStatusLabel, t } from '@/i18n'

const props = defineProps<{ project: string; id: string }>()

const router = useRouter()
const payload = ref<BlackboardPayload | null>(null)
const lanes = ref<LaneDef[]>([])
const loading = ref(true)
const error = ref('')
const ticketContent = ref('')
const contentLoading = ref(false)

const ticket = computed(() => payload.value?.tickets.find((item) => item.id === props.id))
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
  ticketContent.value = ''
  try {
    // Tickets and lanes are independent reads, kick them off in parallel so
    // the detail panel still renders if one fails.
    const [data, laneResult] = await Promise.all([
      loadBlackboardData(project),
      loadLanes(project),
    ])
    payload.value = data
    lanes.value = laneResult.data
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

// Load ticket content on demand when the ticket id is resolved.
watch(
  () => ticket.value?.id,
  async (id) => {
    if (!id) { ticketContent.value = ''; return }
    contentLoading.value = true
    try {
      ticketContent.value = await loadTicketContent(props.project, id)
    } catch {
      ticketContent.value = ''
    } finally {
      contentLoading.value = false
    }
  },
)

watch(
  () => props.project,
  (project) => {
    if (project) reload(project)
  },
  { immediate: true },
)
</script>

<template>
  <div>
    <AppNav
      :title="ticket ? `${ticket.id} · ${ticket.title}` : id"
      :subtitle="`${project} · ${t('ticketDetailSubtitle')}`"
    >
      <template #actions>
        <button class="btn-base btn-outline" @click="router.push(boardRoute(project))">{{ t('ticketBackToBoard') }}</button>
      </template>
    </AppNav>

    <main class="page-content bb-page">
      <div v-if="loading" class="control-panel bb-empty">{{ t('ticketLoading') }}</div>
      <div v-else-if="error" class="control-panel bb-error">{{ error }}</div>
      <div v-else-if="!ticket" class="control-panel bb-error">{{ t('ticketNotFound') }}</div>

      <template v-else>
        <section class="ticket-detail-grid">
          <article class="control-panel ticket-document">
            <div v-if="contentLoading">{{ t('ticketLoadingContent') }}</div>
            <MarkdownRenderer v-else :content="ticketContent" :locale="locale" />
          </article>

          <aside class="ticket-side">
            <div class="control-panel meta-panel">
              <div class="meta-id">{{ ticket.id }}</div>
              <div class="meta-title">{{ ticket.title }}</div>
              <div class="meta-row"><span>{{ t('ticketProject') }}</span><strong>{{ project }}</strong></div>
              <div class="meta-row"><span>{{ t('ticketLane') }}</span><strong :style="{ color: laneMeta?.color }">{{ ticket.lane }} · {{ laneMeta?.label }}</strong></div>
              <div class="meta-row"><span>{{ t('status') }}</span><strong>{{ ticketStatusLabel(ticket.status) }}</strong></div>
              <div class="meta-row"><span>{{ t('assignee') }}</span><strong>{{ ticket.extra.assignee || t('unassigned') }}</strong></div>
              <!--
                The former frontmatter `current` field was removed in favour of
                the `# 当前进展` body section, which is rendered in full by the
                MarkdownRenderer on the left. No duplicate "current action"
                row is needed here.
              -->
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

            <TableOfContents :content="ticketContent" :locale="locale" />
          </aside>
        </section>
      </template>
    </main>
  </div>
</template>
