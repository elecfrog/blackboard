<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { Archive, ExternalLink, GitFork, Menu, Plus, Trash2, X } from 'lucide-vue-next'
import { BbSectionHeader } from '@/components/common'
import TicketStructuredDocument from '@/components/TicketStructuredDocument.vue'
import UnifiedPopupSelect from '@/components/UnifiedPopupSelect.vue'
import type { ProjectAgentProfile } from '@/data/agents'
import type { BlackboardTicket, LaneDef, TicketAttachment } from '@/data/tickets'
import {
  loadTicketDetail,
  resolveLaneMeta,
  ticketStatusOrder,
  ticketRoute,
} from '@/data/tickets'
import { t, ticketStatusLabel } from '@/i18n'

const props = defineProps<{
  project: string
  ticket: BlackboardTicket
  tickets: BlackboardTicket[]
  lanes: LaneDef[]
  agents: ProjectAgentProfile[]
  assigneeSaving?: boolean
  statusSaving?: boolean
  attachmentsSaving?: boolean
  specSaving?: boolean
  deprecating?: boolean
}>()

const emit = defineEmits<{
  close: []
  assigneeChange: [value: string]
  statusChange: [value: string]
  attachmentsChange: [value: TicketAttachment[]]
  specChange: [value: BlackboardTicket['spec']]
  deprecate: []
}>()

const router = useRouter()

const lane = computed(() => resolveLaneMeta(props.ticket.lane, props.lanes))
const currentAssignee = computed(() => props.ticket.extra.assignee?.trim() || '')

const detailedTicket = ref<BlackboardTicket | null>(null)
const detailLoading = ref(false)
const detailError = ref('')
const documentTicket = computed(() => detailedTicket.value ?? props.ticket)
const deprecateConfirmVisible = ref(false)
const ticketMenuOpen = ref(false)

async function fetchDetail() {
  detailLoading.value = true
  detailError.value = ''
  try {
    detailedTicket.value = await loadTicketDetail(props.project, props.ticket.id)
  } catch (err) {
    detailedTicket.value = null
    detailError.value = err instanceof Error ? err.message : String(err)
  } finally {
    detailLoading.value = false
  }
}

watch(
  () => props.ticket.id,
  () => {
    deprecateConfirmVisible.value = false
    ticketMenuOpen.value = false
    fetchDetail()
  },
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
const attachmentKindOptions = ['wiki', 'ticket', 'file', 'url', 'artifact', 'run', 'external']
const currentAttachments = computed(() =>
  normalizeAttachmentList(props.ticket.attachments),
)
const attachmentDrafts = ref<TicketAttachment[]>([])
const attachmentError = ref('')
const attachmentsDirty = computed(
  () =>
    JSON.stringify(normalizeAttachmentList(attachmentDrafts.value)) !==
    JSON.stringify(currentAttachments.value),
)

watch(
  () => [props.ticket.id, props.ticket.attachments],
  () => {
    attachmentDrafts.value = currentAttachments.value.map(cloneAttachment)
    attachmentError.value = ''
  },
  { immediate: true, deep: true },
)

function openRelated(id: string) {
  router.push(ticketRoute(props.project, id))
}

function openGraph() {
  router.push(`/projects/${props.project}/graph?focus=${props.ticket.id}`)
}

function toggleTicketMenu() {
  if (props.deprecating) return
  ticketMenuOpen.value = !ticketMenuOpen.value
}

function closeTicketMenu() {
  ticketMenuOpen.value = false
}

function openDeprecateConfirm() {
  if (props.deprecating) return
  ticketMenuOpen.value = false
  deprecateConfirmVisible.value = true
}

function cancelDeprecate() {
  if (props.deprecating) return
  deprecateConfirmVisible.value = false
}

function confirmDeprecate() {
  if (props.deprecating) return
  deprecateConfirmVisible.value = false
  emit('deprecate')
}

function onStatusChange(next: string) {
  if (next === props.ticket.status) return
  emit('statusChange', next)
}

function onAssigneeChange(next: string) {
  if (next === currentAssignee.value) return
  emit('assigneeChange', next)
}

function onSpecChange(spec: BlackboardTicket['spec']) {
  emit('specChange', spec)
}

function cloneAttachment(attachment: TicketAttachment): TicketAttachment {
  return {
    kind: attachment.kind,
    target: attachment.target,
    ...(attachment.label ? { label: attachment.label } : {}),
    ...(attachment.description ? { description: attachment.description } : {}),
  }
}

function normalizeAttachmentList(attachments: TicketAttachment[]): TicketAttachment[] {
  return attachments
    .map((attachment) => {
      const kind = attachment.kind.trim().toLowerCase()
      const target = attachment.target.trim()
      const label = attachment.label?.trim()
      const description = attachment.description?.trim()
      if (!kind || !target) return null
      return {
        kind,
        target,
        ...(label ? { label } : {}),
        ...(description ? { description } : {}),
      }
    })
    .filter((attachment): attachment is TicketAttachment => Boolean(attachment))
}

function addAttachment() {
  attachmentError.value = ''
  attachmentDrafts.value = [
    ...attachmentDrafts.value,
    { kind: 'wiki', target: '', label: '' },
  ]
}

function removeAttachment(index: number) {
  attachmentError.value = ''
  attachmentDrafts.value = attachmentDrafts.value.filter((_, itemIndex) => itemIndex !== index)
}

function saveAttachments() {
  const hasPartialRow = attachmentDrafts.value.some((attachment) => {
    const hasAnyValue = Boolean(
      attachment.kind.trim() ||
        attachment.target.trim() ||
        attachment.label?.trim() ||
        attachment.description?.trim(),
    )
    return hasAnyValue && (!attachment.kind.trim() || !attachment.target.trim())
  })
  if (hasPartialRow) {
    attachmentError.value = t('attachmentIncomplete')
    return
  }
  attachmentError.value = ''
  emit('attachmentsChange', normalizeAttachmentList(attachmentDrafts.value))
}

function wikiAttachmentPath(target: string): string {
  return target
    .trim()
    .replace(/\\/g, '/')
    .replace(/^\/+/, '')
    .replace(/^wiki\//, '')
}

function attachmentHref(attachment: TicketAttachment): string {
  const kind = attachment.kind.trim().toLowerCase()
  const target = attachment.target.trim()
  if (!target) return ''
  if (kind === 'wiki') return `/projects/${props.project}/wiki/${wikiAttachmentPath(target)}`
  if (kind === 'ticket' && /^\d{6}$/.test(target)) return ticketRoute(props.project, target)
  if (kind === 'url' || /^https?:\/\//i.test(target)) return target
  return ''
}

function openAttachment(attachment: TicketAttachment) {
  const href = attachmentHref(attachment)
  if (!href) return
  if (/^https?:\/\//i.test(href)) {
    window.open(href, '_blank', 'noopener,noreferrer')
    return
  }
  router.push(href)
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
          <div class="ticket-detail-menu-wrap">
            <button
              class="bb-icon-button ticket-detail-menu-trigger"
              type="button"
              :aria-label="t('ticketActions')"
              :aria-expanded="ticketMenuOpen"
              :disabled="deprecating"
              @click="toggleTicketMenu"
            >
              <Menu class="bb-icon-glyph" aria-hidden="true" />
            </button>
            <button
              v-if="ticketMenuOpen"
              class="ticket-detail-menu-backdrop"
              type="button"
              :aria-label="t('close')"
              @click="closeTicketMenu"
            />
            <div v-if="ticketMenuOpen" class="ticket-detail-menu-popover" role="menu">
              <button
                class="ticket-detail-menu-item danger"
                type="button"
                role="menuitem"
                :disabled="deprecating"
                @click="openDeprecateConfirm"
              >
                <Trash2 class="bb-top-action-svg" aria-hidden="true" />
                <span>{{ t('ticketDelete') }}</span>
              </button>
            </div>
          </div>
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

      <section v-if="relatedTickets.length > 0" class="ticket-detail-section ticket-detail-links">
        <BbSectionHeader :title="t('dependencies')" :count="relatedTickets.length" />
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

      <div class="ticket-detail-content-grid ticket-detail-content-grid--single">
        <article class="ticket-detail-document">
          <TicketStructuredDocument
            :ticket="documentTicket"
            :loading="detailLoading"
            :error="detailError"
            :spec-saving="specSaving"
            @spec-change="onSpecChange"
          />

          <section class="ticket-detail-section ticket-detail-attachments">
            <BbSectionHeader :title="t('attachments')" :count="attachmentDrafts.length" as="div">
              <template #actions>
              <button class="ticket-detail-small-action" type="button" @click="addAttachment">
                <Plus class="bb-top-action-svg" aria-hidden="true" />
                {{ t('add') }}
              </button>
              </template>
            </BbSectionHeader>

            <div v-if="attachmentDrafts.length > 0" class="ticket-attachment-list">
              <div
                v-for="(attachment, index) in attachmentDrafts"
                :key="`${index}-${attachment.kind}-${attachment.target}`"
                class="ticket-attachment-row"
              >
                <select v-model="attachment.kind" class="ticket-attachment-kind" :aria-label="t('attachmentKind')">
                  <option v-for="kind in attachmentKindOptions" :key="kind" :value="kind">
                    {{ kind }}
                  </option>
                </select>
                <input
                  v-model.trim="attachment.target"
                  class="ticket-attachment-target"
                  type="text"
                  :placeholder="t('attachmentTarget')"
                />
                <input
                  v-model.trim="attachment.label"
                  class="ticket-attachment-label"
                  type="text"
                  :placeholder="t('attachmentLabel')"
                />
                <button
                  class="bb-icon-button ticket-attachment-icon"
                  type="button"
                  :aria-label="t('open')"
                  :disabled="!attachmentHref(attachment)"
                  @click="openAttachment(attachment)"
                >
                  <ExternalLink class="bb-icon-glyph" aria-hidden="true" />
                </button>
                <button
                  class="bb-icon-button ticket-attachment-icon"
                  type="button"
                  :aria-label="t('removeAttachment')"
                  @click="removeAttachment(index)"
                >
                  <Trash2 class="bb-icon-glyph" aria-hidden="true" />
                </button>
              </div>
            </div>
            <p v-else class="ticket-attachment-empty">{{ t('noAttachments') }}</p>

            <div class="ticket-detail-attachment-actions">
              <span v-if="attachmentError" class="ticket-detail-attachment-error">
                {{ attachmentError }}
              </span>
              <button
                class="ticket-detail-save-attachments"
                type="button"
                :disabled="attachmentsSaving || !attachmentsDirty"
                @click="saveAttachments"
              >
                {{ attachmentsSaving ? t('saving') : t('save') }}
              </button>
            </div>
          </section>
        </article>
      </div>

      <div
        v-if="deprecateConfirmVisible"
        class="ticket-deprecate-confirm-backdrop"
        @click.self="cancelDeprecate"
      >
        <section class="ticket-deprecate-confirm-dialog" role="alertdialog" aria-modal="true">
          <header>
            <Archive class="bb-top-action-svg" aria-hidden="true" />
            <div>
              <h3>{{ t('ticketDeprecateTitle') }}</h3>
              <p>{{ ticket.id }} · {{ ticket.title }}</p>
            </div>
          </header>
          <p>{{ t('ticketDeprecateConfirm', { id: ticket.id }) }}</p>
          <footer>
            <button class="bb-top-action-button" type="button" :disabled="deprecating" @click="cancelDeprecate">
              {{ t('cancel') }}
            </button>
            <button
              class="bb-top-action-button ticket-deprecate-confirm-submit"
              type="button"
              :disabled="deprecating"
              @click="confirmDeprecate"
            >
              <Archive class="bb-top-action-svg" aria-hidden="true" />
              {{ deprecating ? t('saving') : t('ticketDeprecateConfirmAction') }}
            </button>
          </footer>
        </section>
      </div>
    </aside>
  </div>
</template>
