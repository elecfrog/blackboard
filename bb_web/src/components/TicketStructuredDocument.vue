<script setup lang="ts">
import { computed } from 'vue'
import type { BlackboardTicket } from '@/data/tickets'
import { t } from '@/i18n'

const props = defineProps<{
  ticket: BlackboardTicket
  loading?: boolean
  error?: string
}>()

const spec = computed(() => props.ticket.spec)
const summaryCount = computed(() => (spec.value?.summary.trim() ? 1 : 0))
const stories = computed(() => spec.value?.stories ?? [])
const risks = computed(() => spec.value?.risks ?? [])
const progressRecords = computed(() => spec.value?.progress_record ?? [])
</script>

<template>
  <div class="ticket-json-document">
    <div v-if="loading" class="ticket-json-empty">{{ t('ticketLoadingContent') }}</div>
    <div v-else-if="error" class="ticket-json-empty ticket-json-empty--error">{{ error }}</div>
    <template v-else-if="spec">
      <section class="ticket-detail-section ticket-json-section">
        <header class="ticket-detail-section-head">
          <div class="ticket-detail-section-title">
            <h3>{{ t('ticketJsonSummary') }}</h3>
            <span class="ticket-detail-section-count">{{ summaryCount }}</span>
          </div>
        </header>
        <p class="ticket-json-summary">{{ spec.summary }}</p>
      </section>

      <section class="ticket-detail-section ticket-json-section">
        <header class="ticket-detail-section-head">
          <div class="ticket-detail-section-title">
            <h3>{{ t('ticketJsonStories') }}</h3>
            <span class="ticket-detail-section-count">{{ stories.length }}</span>
          </div>
        </header>
        <div class="ticket-json-card-grid">
          <article
            v-for="(story, index) in stories"
            :key="story.id"
            class="ticket-json-card ticket-json-story-card"
          >
            <div class="ticket-json-card-kicker">{{ ticket.id }} / {{ index + 1 }}</div>
            <dl class="ticket-json-story-lines">
              <div>
                <dt>{{ t('ticketJsonGiven') }}</dt>
                <dd>{{ story.given }}</dd>
              </div>
              <div>
                <dt>{{ t('ticketJsonWhen') }}</dt>
                <dd>{{ story.when }}</dd>
              </div>
              <div>
                <dt>{{ t('ticketJsonThen') }}</dt>
                <dd>{{ story.then }}</dd>
              </div>
            </dl>
            <p v-if="story.sample" class="ticket-json-sample">
              <span>{{ t('ticketJsonSample') }}</span>
              {{ story.sample }}
            </p>
          </article>
        </div>
      </section>

      <section class="ticket-detail-section ticket-json-section">
        <header class="ticket-detail-section-head">
          <div class="ticket-detail-section-title">
            <h3>{{ t('ticketJsonRisks') }}</h3>
            <span class="ticket-detail-section-count">{{ risks.length }}</span>
          </div>
        </header>
        <div v-if="risks.length > 0" class="ticket-json-card-grid">
          <article v-for="risk in risks" :key="risk.id" class="ticket-json-card">
            <div class="ticket-json-card-kicker">{{ risk.id }}</div>
            <p class="ticket-json-card-main">{{ risk.description }}</p>
            <dl class="ticket-json-meta-list">
              <div v-if="risk.mitigation">
                <dt>{{ t('ticketJsonMitigation') }}</dt>
                <dd>{{ risk.mitigation }}</dd>
              </div>
              <div v-if="risk.status">
                <dt>{{ t('ticketJsonStatus') }}</dt>
                <dd>{{ risk.status }}</dd>
              </div>
            </dl>
          </article>
        </div>
        <p v-else class="ticket-json-empty">{{ t('ticketJsonNoRisks') }}</p>
      </section>

      <section class="ticket-detail-section ticket-json-section">
        <header class="ticket-detail-section-head">
          <div class="ticket-detail-section-title">
            <h3>{{ t('ticketJsonProgressRecord') }}</h3>
            <span class="ticket-detail-section-count">{{ progressRecords.length }}</span>
          </div>
        </header>
        <div v-if="progressRecords.length > 0" class="ticket-json-card-grid">
          <article
            v-for="(record, index) in progressRecords"
            :key="`${record.at ?? 'record'}-${index}`"
            class="ticket-json-card"
          >
            <div v-if="record.at" class="ticket-json-card-kicker">{{ record.at }}</div>
            <p class="ticket-json-card-main">{{ record.summary }}</p>
            <div v-if="record.evidence?.length" class="ticket-json-evidence">
              <span>{{ t('ticketJsonEvidence') }}</span>
              <ul>
                <li v-for="item in record.evidence" :key="item">{{ item }}</li>
              </ul>
            </div>
          </article>
        </div>
        <p v-else class="ticket-json-empty">{{ t('ticketJsonNoProgress') }}</p>
      </section>

    </template>
    <div v-else class="ticket-json-empty ticket-json-empty--error">{{ t('ticketJsonMissing') }}</div>
  </div>
</template>
