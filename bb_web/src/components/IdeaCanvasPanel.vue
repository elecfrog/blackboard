<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { GripHorizontal, Plus, RefreshCw, Trash2 } from 'lucide-vue-next'
import BbObjectItem from '@/components/common/BbObjectItem.vue'
import { BbActionGroup, BbButton, BbIconCommand, BbToolbar } from '@/components/common'
import {
  createIdeaCanvas,
  createStickyNote,
  deleteIdeaCanvas,
  deleteStickyNote,
  loadIdeaCanvas,
  loadIdeaCanvases,
  patchIdeaCanvas,
  patchStickyNote,
  type IdeaCanvasDetail,
  type IdeaCanvasEntry,
  type StickyNote,
} from '@/data/ideaCanvas'
import { t } from '@/i18n'

const NOTE_COLORS = ['yellow', 'pink', 'mint', 'blue']

const props = defineProps<{
  project: string
}>()

const canvases = ref<IdeaCanvasEntry[]>([])
const selectedCanvasId = ref<string | null>(null)
const canvas = ref<IdeaCanvasDetail | null>(null)
const titleDraft = ref('')
const loading = ref(false)
const canvasLoading = ref(false)
const creatingCanvas = ref(false)
const creatingNote = ref(false)
const savingTitle = ref(false)
const deletingCanvasId = ref<string | null>(null)
const deletingNoteId = ref<string | null>(null)
const pendingDeleteCanvas = ref<IdeaCanvasEntry | null>(null)
const error = ref('')
const surfaceRef = ref<HTMLElement | null>(null)
const textSaveTimers = new Map<string, number>()
const dragging = ref<{
  id: string
  startX: number
  startY: number
  originX: number
  originY: number
} | null>(null)

watch(
  () => props.project,
  (project) => {
    if (project) void reload()
  },
  { immediate: true },
)

onBeforeUnmount(() => {
  clearAllNoteTimers()
  stopDragListeners()
})

async function reload(preferredId = selectedCanvasId.value) {
  loading.value = true
  error.value = ''
  try {
    const payload = await loadIdeaCanvases(props.project)
    canvases.value = payload.canvases
    const target =
      preferredId && payload.canvases.some((item) => item.id === preferredId)
        ? preferredId
        : payload.canvases[0]?.id ?? null
    if (target) {
      await selectCanvas(target)
    } else {
      selectedCanvasId.value = null
      canvas.value = null
      titleDraft.value = ''
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

async function selectCanvas(id: string) {
  if (selectedCanvasId.value === id && canvas.value?.id === id) return
  selectedCanvasId.value = id
  canvasLoading.value = true
  error.value = ''
  try {
    const loaded = await loadIdeaCanvas(props.project, id)
    canvas.value = loaded
    titleDraft.value = loaded.title
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    canvasLoading.value = false
  }
}

async function addCanvas() {
  if (creatingCanvas.value) return
  creatingCanvas.value = true
  error.value = ''
  try {
    const title = t('ideaCanvasDefaultTitle', { count: canvases.value.length + 1 })
    const result = await createIdeaCanvas(props.project, { title })
    canvas.value = result.canvas
    selectedCanvasId.value = result.canvas.id
    titleDraft.value = result.canvas.title
    upsertCanvasEntry(result.canvas)
    await nextTick()
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    creatingCanvas.value = false
  }
}

async function saveCanvasTitle() {
  if (!canvas.value || savingTitle.value) return
  const title = titleDraft.value.trim()
  if (!title || title === canvas.value.title) {
    titleDraft.value = canvas.value.title
    return
  }
  savingTitle.value = true
  error.value = ''
  try {
    const result = await patchIdeaCanvas(props.project, canvas.value.id, { title })
    canvas.value = result.canvas
    titleDraft.value = result.canvas.title
    upsertCanvasEntry(result.canvas)
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    savingTitle.value = false
  }
}

function openCanvasDeleteConfirm(item: IdeaCanvasEntry) {
  if (deletingCanvasId.value) return
  pendingDeleteCanvas.value = item
}

function cancelCanvasDelete() {
  if (deletingCanvasId.value) return
  pendingDeleteCanvas.value = null
}

async function confirmCanvasDelete() {
  const item = pendingDeleteCanvas.value
  if (!item || deletingCanvasId.value) return
  deletingCanvasId.value = item.id
  error.value = ''
  const wasSelected = selectedCanvasId.value === item.id
  try {
    if (wasSelected) clearAllNoteTimers()
    await deleteIdeaCanvas(props.project, item.id)
    canvases.value = canvases.value.filter((canvasItem) => canvasItem.id !== item.id)
    if (wasSelected) {
      selectedCanvasId.value = null
      canvas.value = null
      titleDraft.value = ''
      const nextCanvasId = canvases.value[0]?.id ?? null
      if (nextCanvasId) await selectCanvas(nextCanvasId)
    }
    pendingDeleteCanvas.value = null
  } catch (err) {
    pendingDeleteCanvas.value = null
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    deletingCanvasId.value = null
  }
}

async function createNoteAt(event: MouseEvent) {
  if (!canvas.value || creatingNote.value) return
  const target = event.target as HTMLElement | null
  if (target?.closest('.bb-sticky-note')) return
  const surface = surfaceRef.value
  if (!surface) return

  const rect = surface.getBoundingClientRect()
  const x = Math.max(12, Math.round(event.clientX - rect.left - 108))
  const y = Math.max(12, Math.round(event.clientY - rect.top - 72))
  const color = NOTE_COLORS[canvas.value.notes.length % NOTE_COLORS.length]
  creatingNote.value = true
  error.value = ''
  try {
    const result = await createStickyNote(props.project, canvas.value.id, {
      text: '',
      x,
      y,
      color,
    })
    canvas.value = result.canvas
    upsertCanvasEntry(result.canvas)
    await focusNote(result.note.id)
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    creatingNote.value = false
  }
}

async function removeNote(note: StickyNote) {
  if (!canvas.value || deletingNoteId.value) return
  const canvasId = canvas.value.id
  deletingNoteId.value = note.id
  error.value = ''
  try {
    clearNoteTimer(note.id)
    await deleteStickyNote(props.project, canvasId, note.id)
    if (canvas.value?.id === canvasId) {
      canvas.value.notes = canvas.value.notes.filter((item) => item.id !== note.id)
      upsertCanvasEntry(canvas.value)
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    deletingNoteId.value = null
  }
}

function onNoteInput(note: StickyNote, event: Event) {
  const target = event.target
  if (!(target instanceof HTMLTextAreaElement)) return
  note.text = target.value
  scheduleNoteTextSave(note)
}

function scheduleNoteTextSave(note: StickyNote) {
  clearNoteTimer(note.id)
  textSaveTimers.set(
    note.id,
    window.setTimeout(() => {
      textSaveTimers.delete(note.id)
      void saveNoteText(note.id)
    }, 450),
  )
}

function flushNoteText(note: StickyNote) {
  clearNoteTimer(note.id)
  void saveNoteText(note.id)
}

async function saveNoteText(noteId: string) {
  if (!canvas.value) return
  const note = canvas.value.notes.find((item) => item.id === noteId)
  if (!note) return
  try {
    await patchStickyNote(props.project, canvas.value.id, noteId, { text: note.text })
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  }
}

function clearNoteTimer(noteId: string) {
  const timer = textSaveTimers.get(noteId)
  if (timer) window.clearTimeout(timer)
  textSaveTimers.delete(noteId)
}

function clearAllNoteTimers() {
  for (const timer of textSaveTimers.values()) window.clearTimeout(timer)
  textSaveTimers.clear()
}

function startDrag(event: PointerEvent, note: StickyNote) {
  const target = event.target as HTMLElement | null
  if (target?.closest('textarea, button')) return
  dragging.value = {
    id: note.id,
    startX: event.clientX,
    startY: event.clientY,
    originX: note.x,
    originY: note.y,
  }
  window.addEventListener('pointermove', onDragMove)
  window.addEventListener('pointerup', stopDrag)
}

function onDragMove(event: PointerEvent) {
  if (!dragging.value || !canvas.value) return
  const note = canvas.value.notes.find((item) => item.id === dragging.value?.id)
  if (!note) return
  note.x = Math.max(0, Math.round(dragging.value.originX + event.clientX - dragging.value.startX))
  note.y = Math.max(0, Math.round(dragging.value.originY + event.clientY - dragging.value.startY))
  event.preventDefault()
}

function stopDrag() {
  const active = dragging.value
  dragging.value = null
  stopDragListeners()
  if (!active || !canvas.value) return
  const note = canvas.value.notes.find((item) => item.id === active.id)
  if (!note) return
  void patchStickyNote(props.project, canvas.value.id, note.id, { x: note.x, y: note.y }).catch((err) => {
    error.value = err instanceof Error ? err.message : String(err)
  })
}

function stopDragListeners() {
  window.removeEventListener('pointermove', onDragMove)
  window.removeEventListener('pointerup', stopDrag)
}

function noteStyle(note: StickyNote) {
  return {
    transform: `translate(${note.x}px, ${note.y}px)`,
  }
}

function noteClass(note: StickyNote) {
  return [
    'bb-sticky-note',
    `bb-sticky-note--${NOTE_COLORS.includes(note.color) ? note.color : 'yellow'}`,
    { dragging: dragging.value?.id === note.id },
  ]
}

async function focusNote(noteId: string) {
  await nextTick()
  const editor = surfaceRef.value?.querySelector<HTMLTextAreaElement>(`[data-note-editor="${noteId}"]`)
  editor?.focus()
}

function upsertCanvasEntry(value: IdeaCanvasDetail) {
  const entry: IdeaCanvasEntry = {
    id: value.id,
    title: value.title,
    note_count: value.notes.length,
    created_at: value.created_at,
    updated_at: value.updated_at,
  }
  canvases.value = [entry, ...canvases.value.filter((item) => item.id !== value.id)]
}
</script>

<template>
  <section class="bb-workbench-workspace bb-idea-canvas-workspace">
    <header class="bb-workspace-head">
      <div class="bb-workspace-head-main">
        <h2>{{ t('ideaCanvas') }}</h2>
        <p>{{ t('ideaCanvasSubtitle') }}</p>
      </div>
      <BbToolbar class="bb-workspace-head-actions" variant="inline">
        <template #actions>
          <BbActionGroup>
            <BbIconCommand :title="t('refresh')" @click="reload()">
              <RefreshCw aria-hidden="true" />
            </BbIconCommand>
            <BbButton variant="primary" :disabled="creatingCanvas" @click="addCanvas">
              <template #leading>
                <Plus aria-hidden="true" />
              </template>
              {{ t('ideaCanvasNew') }}
            </BbButton>
          </BbActionGroup>
        </template>
      </BbToolbar>
    </header>

    <div v-if="loading" class="bb-empty">{{ t('ideaCanvasLoading') }}</div>
    <div v-else-if="error" class="bb-error">{{ error }}</div>

    <div v-else class="bb-idea-canvas-split">
      <aside class="bb-object-list-pane bb-idea-canvas-list">
        <ul class="bb-object-list">
          <li
            v-for="item in canvases"
            :key="item.id"
            class="bb-object-list-row"
          >
            <BbObjectItem
              :title="item.title"
              :active="selectedCanvasId === item.id"
              @select="selectCanvas(item.id)"
            />
            <button
              class="bb-object-row-action bb-object-row-action--danger"
              type="button"
              :title="t('ideaCanvasDeleteCanvas')"
              :aria-label="t('ideaCanvasDeleteCanvas')"
              :disabled="deletingCanvasId === item.id"
              @click.stop="openCanvasDeleteConfirm(item)"
            >
              <Trash2 aria-hidden="true" />
            </button>
          </li>
        </ul>
        <div v-if="canvases.length === 0" class="bb-empty">{{ t('ideaCanvasEmpty') }}</div>
      </aside>

      <main class="bb-idea-canvas-main">
        <div v-if="canvasLoading" class="bb-empty">{{ t('ideaCanvasLoading') }}</div>
        <div v-else-if="!canvas" class="bb-idea-canvas-blank">
          <BbButton variant="primary" :disabled="creatingCanvas" @click="addCanvas">
            <template #leading>
              <Plus aria-hidden="true" />
            </template>
            {{ t('ideaCanvasNew') }}
          </BbButton>
        </div>
        <template v-else>
          <div class="bb-idea-canvas-titlebar">
            <input
              v-model="titleDraft"
              class="bb-canvas-title-input"
              :aria-label="t('ideaCanvasTitle')"
              @blur="saveCanvasTitle"
              @keydown.enter.prevent="saveCanvasTitle"
            />
            <span>{{ t('ideaCanvasNoteCount', { count: canvas.notes.length }) }}</span>
          </div>

          <div class="bb-idea-canvas-viewport">
            <div ref="surfaceRef" class="bb-idea-canvas-surface" @dblclick="createNoteAt">
              <article v-for="note in canvas.notes" :key="note.id" :class="noteClass(note)" :style="noteStyle(note)">
                <div class="bb-sticky-note-drag" @pointerdown.stop="startDrag($event, note)">
                  <GripHorizontal aria-hidden="true" />
                </div>
                <button
                  class="bb-sticky-note-delete"
                  type="button"
                  :title="t('ideaCanvasDeleteNote')"
                  :disabled="deletingNoteId === note.id"
                  @pointerdown.stop
                  @click.stop="removeNote(note)"
                >
                  <Trash2 aria-hidden="true" />
                </button>
                <textarea
                  :data-note-editor="note.id"
                  :value="note.text"
                  :placeholder="t('ideaCanvasNotePlaceholder')"
                  @pointerdown.stop
                  @input="onNoteInput(note, $event)"
                  @blur="flushNoteText(note)"
                />
              </article>
            </div>
          </div>
        </template>
      </main>
    </div>

    <Teleport to="body">
      <div
        v-if="pendingDeleteCanvas"
        class="bb-idea-canvas-confirm-backdrop"
        @click.self="cancelCanvasDelete"
      >
        <section
          class="bb-idea-canvas-confirm-dialog"
          role="alertdialog"
          aria-modal="true"
          aria-labelledby="bb-idea-canvas-delete-title"
        >
          <header>
            <Trash2 class="bb-top-action-svg" aria-hidden="true" />
            <div>
              <h3 id="bb-idea-canvas-delete-title">{{ t('ideaCanvasDeleteTitle') }}</h3>
              <p>{{ pendingDeleteCanvas.title }}</p>
            </div>
          </header>
          <p>{{ t('ideaCanvasDeleteConfirm', { title: pendingDeleteCanvas.title }) }}</p>
          <p class="bb-idea-canvas-confirm-hint">{{ t('ideaCanvasDeleteHint') }}</p>
          <footer>
            <BbActionGroup>
              <BbButton
                variant="secondary"
                :disabled="Boolean(deletingCanvasId)"
                @click="cancelCanvasDelete"
              >
                {{ t('cancel') }}
              </BbButton>
              <BbButton
                class="bb-idea-canvas-confirm-submit"
                variant="danger"
                :disabled="Boolean(deletingCanvasId)"
                @click="confirmCanvasDelete"
              >
                <template #leading>
                  <Trash2 aria-hidden="true" />
                </template>
                {{ deletingCanvasId ? t('ideaCanvasDeletingCanvas') : t('ideaCanvasDeleteCanvas') }}
              </BbButton>
            </BbActionGroup>
          </footer>
        </section>
      </div>
    </Teleport>
  </section>
</template>
