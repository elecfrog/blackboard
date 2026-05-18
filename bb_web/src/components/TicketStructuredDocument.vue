<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { BbInfoGrid, BbInfoItem, BbSectionHeader } from '@/components/common'
import type { BlackboardTicket } from '@/data/tickets'
import { t } from '@/i18n'
import { Check, Pencil, Plus, Trash2, X } from 'lucide-vue-next'

const props = defineProps<{
  ticket: BlackboardTicket
  loading?: boolean
  error?: string
  specSaving?: boolean
}>()

const emit = defineEmits<{
  specChange: [spec: BlackboardTicket['spec']]
}>()

const spec = computed(() => props.ticket.spec)
const summaryCount = computed(() => (spec.value?.summary.trim() ? 1 : 0))
const stories = computed(() => spec.value?.stories ?? [])
const risks = computed(() => spec.value?.risks ?? [])
const progressRecords = computed(() => spec.value?.progress_record ?? [])
type StoryDraft = { id?: string; given: string; when: string; then: string; sample?: string }

function editableSpec() {
  if (!spec.value) return null
  return {
    ...spec.value,
    risks: spec.value.risks ?? [],
    progress_record: spec.value.progress_record ?? [],
  }
}

// ── Summary editing ──
const summaryEditing = ref(false)
const summaryDraft = ref('')
const summaryDirty = computed(() => summaryDraft.value !== (spec.value?.summary ?? ''))

function startSummaryEdit() {
  summaryDraft.value = spec.value?.summary ?? ''
  summaryEditing.value = true
}

function cancelSummaryEdit() {
  summaryEditing.value = false
}

function saveSummary() {
  const nextSpec = editableSpec()
  if (!nextSpec) return
  emit('specChange', {
    ...nextSpec,
    summary: summaryDraft.value,
  })
  summaryEditing.value = false
}

// ── Stories editing ──
const storiesEditing = ref(false)
const storyDrafts = ref<StoryDraft[]>([])
const storiesDirty = computed(() =>
  JSON.stringify(storyDrafts.value) !==
  JSON.stringify(
    stories.value.map((s) => ({
      id: s.id,
      given: s.given,
      when: s.when,
      then: s.then,
      sample: s.sample,
    }))
  )
)

function cloneStoryDrafts(): StoryDraft[] {
  return stories.value.map((s) => ({
    id: s.id,
    given: s.given,
    when: s.when,
    then: s.then,
    sample: s.sample,
  }))
}

function resetStoryDrafts() {
  storyDrafts.value = cloneStoryDrafts()
}

watch(
  () => stories.value,
  () => {
    if (!storiesEditing.value) resetStoryDrafts()
  },
  { immediate: true, deep: true },
)

function startStoriesEdit() {
  resetStoryDrafts()
  storiesEditing.value = true
}

function cancelStoriesEdit() {
  resetStoryDrafts()
  storiesEditing.value = false
}

function addStory() {
  storyDrafts.value = [
    ...storyDrafts.value,
    { given: '', when: '', then: '' },
  ]
}

function removeStory(index: number) {
  storyDrafts.value = storyDrafts.value.filter((_, i) => i !== index)
}

function saveStories() {
  const nextSpec = editableSpec()
  if (!nextSpec) return
  const updatedStories = storyDrafts.value.map((draft, i) => {
    const existing = stories.value[i]
    return {
      id: draft.id ?? existing?.id ?? '',
      given: draft.given,
      when: draft.when,
      then: draft.then,
      ...(draft.sample ? { sample: draft.sample } : {}),
    }
  })
  emit('specChange', {
    ...nextSpec,
    stories: updatedStories,
  })
  storiesEditing.value = false
}
</script>

<template>
  <div class="ticket-json-document">
    <div v-if="loading" class="ticket-json-empty">{{ t('ticketLoadingContent') }}</div>
    <div v-else-if="error" class="ticket-json-empty ticket-json-empty--error">{{ error }}</div>
    <template v-else-if="spec">
      <section class="ticket-detail-section ticket-json-section">
        <BbSectionHeader :title="t('ticketJsonSummary')" :count="summaryCount">
          <template #actions>
            <button
              v-if="!summaryEditing"
              class="ticket-detail-small-action"
              type="button"
              :aria-label="t('edit')"
              :disabled="specSaving"
              @click="startSummaryEdit"
            >
              <Pencil class="bb-top-action-svg" aria-hidden="true" />
              {{ t('edit') }}
            </button>
            <template v-else>
              <button
                class="ticket-detail-small-action"
                type="button"
                :aria-label="t('cancel')"
                :disabled="specSaving"
                @click="cancelSummaryEdit"
              >
                <X class="bb-top-action-svg" aria-hidden="true" />
                {{ t('cancel') }}
              </button>
              <button
                class="ticket-detail-small-action"
                type="button"
                :disabled="specSaving || !summaryDirty"
                @click="saveSummary"
              >
                <Check class="bb-top-action-svg" aria-hidden="true" />
                {{ specSaving ? t('saving') : t('save') }}
              </button>
            </template>
          </template>
        </BbSectionHeader>
        <template v-if="summaryEditing">
          <textarea
            v-model="summaryDraft"
            class="ticket-json-summary-edit"
            :placeholder="t('ticketJsonSummary')"
          />
        </template>
        <p v-else class="ticket-json-summary">{{ spec.summary }}</p>
      </section>

      <section class="ticket-detail-section ticket-json-section">
        <BbSectionHeader :title="t('ticketJsonStories')" :count="storiesEditing ? storyDrafts.length : stories.length">
          <template #actions>
            <button
              v-if="!storiesEditing"
              class="ticket-detail-small-action"
              type="button"
              :aria-label="t('edit')"
              :disabled="specSaving"
              @click="startStoriesEdit"
            >
              <Pencil class="bb-top-action-svg" aria-hidden="true" />
              {{ t('edit') }}
            </button>
            <template v-else>
              <button
                class="ticket-detail-small-action"
                type="button"
                :aria-label="t('cancel')"
                :disabled="specSaving"
                @click="cancelStoriesEdit"
              >
                <X class="bb-top-action-svg" aria-hidden="true" />
                {{ t('cancel') }}
              </button>
              <button class="ticket-detail-small-action" type="button" :disabled="specSaving" @click="addStory">
                <Plus class="bb-top-action-svg" aria-hidden="true" />
                {{ t('add') }}
              </button>
              <button
                class="ticket-detail-small-action"
                type="button"
                :disabled="specSaving || !storiesDirty"
                @click="saveStories"
              >
                <Check class="bb-top-action-svg" aria-hidden="true" />
                {{ specSaving ? t('saving') : t('save') }}
              </button>
            </template>
          </template>
        </BbSectionHeader>

        <div v-if="storiesEditing" class="ticket-json-card-grid">
          <article
            v-for="(story, index) in storyDrafts"
            :key="story.id ?? `draft-${index}`"
            class="ticket-json-card ticket-json-story-card ticket-json-story-card--editing"
          >
            <div class="ticket-json-card-kicker ticket-json-card-kicker--with-action">
              <span>{{ ticket.id }} / {{ index + 1 }}</span>
              <button
                class="bb-icon-button ticket-json-card-delete"
                type="button"
                :aria-label="t('removeAttachment')"
                :disabled="specSaving"
                @click="removeStory(index)"
              >
                <Trash2 class="bb-icon-glyph" aria-hidden="true" />
              </button>
            </div>
            <dl class="ticket-json-story-lines">
              <div>
                <dt>{{ t('ticketJsonGiven') }}</dt>
                <dd>
                  <textarea
                    v-model="story.given"
                    class="ticket-json-story-edit"
                    :placeholder="t('ticketJsonGiven')"
                  />
                </dd>
              </div>
              <div>
                <dt>{{ t('ticketJsonWhen') }}</dt>
                <dd>
                  <textarea
                    v-model="story.when"
                    class="ticket-json-story-edit"
                    :placeholder="t('ticketJsonWhen')"
                  />
                </dd>
              </div>
              <div>
                <dt>{{ t('ticketJsonThen') }}</dt>
                <dd>
                  <textarea
                    v-model="story.then"
                    class="ticket-json-story-edit"
                    :placeholder="t('ticketJsonThen')"
                  />
                </dd>
              </div>
            </dl>
            <div v-if="story.sample !== undefined" class="ticket-json-sample">
              <span>{{ t('ticketJsonSample') }}</span>
              <textarea
                v-model="story.sample"
                class="ticket-json-story-edit"
                :placeholder="t('ticketJsonSample')"
              />
            </div>
            <div v-else class="ticket-json-sample-add">
              <button
                class="ticket-json-sample-add-btn"
                type="button"
                :disabled="specSaving"
                @click="story.sample = ''"
              >
                <Plus class="bb-top-action-svg" aria-hidden="true" />
                {{ t('ticketJsonSample') }}
              </button>
            </div>
          </article>
        </div>

        <div v-else class="ticket-json-card-grid">
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
        <BbSectionHeader :title="t('ticketJsonRisks')" :count="risks.length" />
        <div v-if="risks.length > 0" class="ticket-json-card-grid">
          <article v-for="risk in risks" :key="risk.id" class="ticket-json-card">
            <div class="ticket-json-card-kicker">{{ risk.id }}</div>
            <p class="ticket-json-card-main">{{ risk.description }}</p>
            <BbInfoGrid class="ticket-json-meta-list" variant="rows">
              <BbInfoItem v-if="risk.mitigation" :label="t('ticketJsonMitigation')" :value="risk.mitigation" />
              <BbInfoItem v-if="risk.status" :label="t('ticketJsonStatus')" :value="risk.status" />
            </BbInfoGrid>
          </article>
        </div>
        <p v-else class="ticket-json-empty">{{ t('ticketJsonNoRisks') }}</p>
      </section>

      <section class="ticket-detail-section ticket-json-section">
        <BbSectionHeader :title="t('ticketJsonProgressRecord')" :count="progressRecords.length" />
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
