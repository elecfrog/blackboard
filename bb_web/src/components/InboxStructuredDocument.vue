<script setup lang="ts">
import { computed } from 'vue'
import { BbInfoGrid, BbInfoItem, BbSectionHeader } from '@/components/common'
import type { InboxNote, TicketAttachment } from '@/data/tickets'
import { t } from '@/i18n'

const props = defineProps<{
  note: InboxNote | null
  loading?: boolean
  error?: string
}>()

const document = computed(() => props.note?.document ?? null)
const sections = computed(() => {
  const doc = document.value
  if (!doc) return []
  return [
    { key: 'done', label: t('inboxJsonDone'), items: doc.done },
    { key: 'validation', label: t('inboxJsonValidation'), items: doc.validation },
    { key: 'next_step', label: t('inboxJsonNextStep'), items: doc.next_step },
    { key: 'related_locations', label: t('inboxJsonRelatedLocations'), items: doc.related_locations },
    { key: 'related_tickets', label: t('inboxJsonRelatedTickets'), items: doc.related_tickets ?? [] },
  ].filter((section) => section.items.length > 0)
})
const attachments = computed(() => document.value?.attachments ?? [])
const extraEntries = computed(() => Object.entries(document.value?.extra ?? {}))

function attachmentLabel(attachment: TicketAttachment) {
  return attachment.label || attachment.target
}
</script>

<template>
  <div class="ticket-json-document inbox-json-document">
    <div v-if="loading" class="ticket-json-empty">{{ t('inboxBodyLoading') }}</div>
    <div v-else-if="error" class="ticket-json-empty ticket-json-empty--error">{{ error }}</div>
    <template v-else-if="document">
      <section class="ticket-detail-section ticket-json-section">
        <BbSectionHeader :title="document.title" count="JSON" />
        <BbInfoGrid class="ticket-json-meta-list inbox-json-meta" variant="rows">
          <BbInfoItem :label="t('inboxJsonTime')" :value="document.time" />
          <BbInfoItem :label="t('inboxJsonSource')" :value="document.source" />
          <BbInfoItem :label="t('inboxJsonProject')" :value="document.project" />
          <BbInfoItem :label="t('inboxJsonTopic')" :value="document.topic" />
          <BbInfoItem v-if="note" :label="t('inboxJsonFile')" :value="note.name" />
        </BbInfoGrid>
      </section>

      <section
        v-for="section in sections"
        :key="section.key"
        class="ticket-detail-section ticket-json-section"
      >
        <BbSectionHeader :title="section.label" :count="section.items.length" />
        <div class="ticket-json-card-grid">
          <article
            v-for="(item, index) in section.items"
            :key="`${section.key}-${index}-${item}`"
            class="ticket-json-card"
          >
            <div class="ticket-json-card-kicker">{{ index + 1 }}</div>
            <p class="ticket-json-card-main">{{ item }}</p>
          </article>
        </div>
      </section>

      <section v-if="attachments.length > 0" class="ticket-detail-section ticket-json-section">
        <BbSectionHeader :title="t('attachments')" :count="attachments.length" />
        <div class="ticket-json-card-grid">
          <article
            v-for="attachment in attachments"
            :key="`${attachment.kind}-${attachment.target}`"
            class="ticket-json-card"
          >
            <div class="ticket-json-card-kicker">{{ attachment.kind }}</div>
            <p class="ticket-json-card-main">{{ attachmentLabel(attachment) }}</p>
            <p v-if="attachment.description" class="ticket-json-sample">
              <span>{{ t('description') }}</span>
              {{ attachment.description }}
            </p>
          </article>
        </div>
      </section>

      <section v-if="extraEntries.length > 0" class="ticket-detail-section ticket-json-section">
        <BbSectionHeader :title="t('inboxJsonExtra')" :count="extraEntries.length" />
        <BbInfoGrid class="ticket-json-meta-list inbox-json-meta" variant="rows">
          <BbInfoItem v-for="[key, value] in extraEntries" :key="key" :label="key">
            {{ typeof value === 'object' ? JSON.stringify(value) : String(value) }}
          </BbInfoItem>
        </BbInfoGrid>
      </section>
    </template>
    <div v-else class="ticket-json-empty ticket-json-empty--error">{{ t('inboxBodyUnavailable') }}</div>
  </div>
</template>
