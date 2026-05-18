<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ChevronDown, ChevronRight } from 'lucide-vue-next'
import { BbToolbar } from '@/components/common'
import BbObjectItem from '@/components/common/BbObjectItem.vue'
import InboxStructuredDocument from '@/components/InboxStructuredDocument.vue'
import type { InboxNote, InboxNoteEntry } from '@/data/tickets'
import { loadInboxNote, loadInboxNotes } from '@/data/tickets'
import { t } from '@/i18n'

const props = withDefaults(defineProps<{
  project: string
  variant?: 'compact' | 'full'
}>(), {
  variant: 'full',
})

const notes = ref<InboxNoteEntry[]>([])
const loading = ref(true)
const error = ref('')

// Per-note expansion / detail state keyed by note name. Using a reactive map
// avoids triggering a full list re-render when a single row is toggled.
interface DetailState {
  loading: boolean
  error: string
  note: InboxNote | null
}

const details = reactive<Record<string, DetailState>>({})
const openName = ref<string | null>(null)

const visibleNotes = computed(() =>
  props.variant === 'compact' ? notes.value.slice(0, 4) : notes.value,
)
const selectedEntry = computed(() =>
  openName.value ? notes.value.find((note) => note.name === openName.value) ?? null : null,
)
const selectedDetail = computed(() =>
  openName.value ? details[openName.value] ?? null : null,
)

async function reload(project: string) {
  loading.value = true
  error.value = ''
  notes.value = []
  for (const key of Object.keys(details)) delete details[key]
  openName.value = null
  try {
    const result = await loadInboxNotes(project)
    notes.value = result.data
    if (props.variant === 'full' && result.data.length > 0) {
      openName.value = result.data[0].name
      void ensureDetail(result.data[0].name, project)
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

watch(
  () => props.project,
  (project) => {
    if (project) reload(project)
  },
  { immediate: true },
)

async function ensureDetail(name: string, project = props.project) {
  if (details[name]?.note) return // already fetched successfully
  if (details[name]?.loading) return
  details[name] = { loading: true, error: '', note: null }
  try {
    const result = await loadInboxNote(project, name)
    details[name] = {
      loading: false,
      error: result.data
        ? ''
        : result.error
          ? t('inboxBodyError', { error: result.error })
          : t('inboxBodyUnavailable'),
      note: result.data,
    }
  } catch (err) {
    details[name] = {
      loading: false,
      error: err instanceof Error ? err.message : String(err),
      note: null,
    }
  }
}

function selectNote(name: string) {
  openName.value = name
  void ensureDetail(name)
}

async function toggle(name: string) {
  if (openName.value === name) {
    openName.value = null
    return
  }
  selectNote(name)
}

watch(
  () => props.variant,
  (variant) => {
    if (variant === 'full' && notes.value.length > 0 && !openName.value) {
      selectNote(notes.value[0].name)
    }
  },
)

function formatTime(value?: string) {
  if (!value) return ''
  return value.replace('T', ' ').replace(/\.\d{3}Z$/, 'Z')
}

function noteTitle(note: InboxNoteEntry) {
  return note.title || note.topic || note.name
}

function noteMeta(note: InboxNoteEntry) {
  return [note.source, note.topic].filter(Boolean).join(' / ')
}

const rootSectionClass = computed(() =>
  props.variant === 'full' ? ['bb-workbench-workspace', 'bb-inbox-full'] : ['bb-inbox-panel', 'bb-inbox-panel--compact'],
)

</script>

<template>
  <section :class="rootSectionClass">
    <header class="bb-workspace-head">
      <div class="bb-workspace-head-main">
        <h2>{{ t('inbox') }}</h2>
        <p v-if="variant === 'full'">{{ t('inboxFullSubtitle') }}</p>
        <p v-else>{{ t('inboxCompactCount', { count: notes.length }) }}</p>
      </div>
      <BbToolbar v-if="variant === 'full'" class="bb-workspace-head-actions" variant="inline">
        <template #meta>
          <span class="bb-toolbar-count">{{ t('inboxToolbarNotes', { count: notes.length }) }}</span>
        </template>
      </BbToolbar>
    </header>

    <div class="bb-inbox-panel-body">
      <div v-if="loading" class="bb-empty">{{ t('inboxLoading') }}</div>
      <div v-else-if="error" class="bb-error">{{ error }}</div>
      <div v-else-if="notes.length === 0" class="bb-empty">{{ t('inboxEmpty') }}</div>
      <section v-else-if="variant === 'full'" class="bb-inbox-catalog">
        <div class="bb-inbox-split">
          <aside class="bb-inbox-list-pane">
            <header class="bb-inbox-pane-head">
              <div class="bb-inbox-pane-title">
                <span class="bb-inbox-pane-kicker">{{ t('inboxFullTitle') }}</span>
                <h3>{{ t('inboxToolbarNotes', { count: notes.length }) }}</h3>
              </div>
              <span class="bb-inbox-pane-meta">{{ t('inboxSelectNote') }}</span>
            </header>
            <ul class="bb-object-list bb-inbox-list--split">
              <li
                v-for="note in visibleNotes"
                :key="note.name"
                class="bb-object-list-row"
              >
                <BbObjectItem
                  :title="noteTitle(note)"
                  :active="openName === note.name"
                  @select="selectNote(note.name)"
                >
                  <template #trailing>
                    <ChevronRight />
                  </template>
                </BbObjectItem>
              </li>
            </ul>
          </aside>
          <article class="bb-inbox-reader">
            <header class="bb-inbox-reader-head">
              <div class="bb-inbox-reader-title-block">
                <span class="bb-inbox-reader-kicker">{{ t('inboxReaderTitle') }}</span>
                <h3 class="bb-inbox-reader-title">
                  {{ selectedEntry ? noteTitle(selectedEntry) : t('inboxSelectNote') }}
                </h3>
              </div>
              <span v-if="selectedEntry" class="bb-inbox-reader-file">
                {{ selectedEntry.name }}
              </span>
            </header>
            <div class="bb-inbox-reader-body">
              <div v-if="selectedDetail?.loading" class="bb-empty">{{ t('inboxBodyLoading') }}</div>
              <div v-else-if="selectedDetail?.error" class="bb-inbox-offline">
                {{ selectedDetail.error }}
              </div>
              <InboxStructuredDocument
                v-else-if="selectedDetail?.note"
                :note="selectedDetail.note"
              />
              <div v-else class="bb-empty">{{ t('inboxSelectNote') }}</div>
            </div>
          </article>
        </div>
      </section>
      <ul v-else class="bb-inbox-list">
        <li v-for="note in visibleNotes" :key="note.name" class="bb-inbox-item">
          <button
            type="button"
            class="bb-inbox-row"
            :aria-expanded="openName === note.name"
            @click="toggle(note.name)"
          >
            <span class="bb-inbox-name">{{ noteTitle(note) }}</span>
            <span v-if="note.time || note.modified_at" class="bb-inbox-time">{{ formatTime(note.time || note.modified_at) }}</span>
            <component
              :is="openName === note.name ? ChevronDown : ChevronRight"
              class="bb-inbox-caret"
              aria-hidden="true"
            />
          </button>
          <p v-if="noteMeta(note)" class="bb-inbox-meta">{{ noteMeta(note) }}</p>
          <p v-if="note.excerpt" class="bb-inbox-excerpt">{{ note.excerpt }}</p>
          <div v-if="openName === note.name" class="bb-inbox-detail">
            <div v-if="details[note.name]?.loading" class="bb-empty">{{ t('inboxBodyLoading') }}</div>
            <div v-else-if="details[note.name]?.error" class="bb-inbox-offline">
              {{ details[note.name]?.error }}
            </div>
            <InboxStructuredDocument
              v-else-if="details[note.name]?.note"
              :note="details[note.name]?.note ?? null"
            />
          </div>
        </li>
        <li v-if="variant === 'compact' && notes.length > visibleNotes.length" class="bb-inbox-more">
          {{ t('inboxMore', { count: notes.length - visibleNotes.length }) }}
        </li>
      </ul>
    </div>
  </section>
</template>
