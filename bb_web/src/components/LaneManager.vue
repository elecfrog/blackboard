<script setup lang="ts">
import { computed, ref } from 'vue'
import type { LaneDef } from '@/data/tickets'
import { archiveLane, patchLane, upsertLane } from '@/data/tickets'
import { BbButton, BbEmptyState, BbInlineAlert } from '@/components/common'
import { t } from '@/i18n'

const props = defineProps<{
  project: string
  lanes: LaneDef[]
  resolveMeta: (laneId: string) => { label: string; color: string; description: string }
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'changed'): void
}>()

// `editing` is null when the form is in "create" mode and a lane id when
// editing an existing entry. The form fields are bound to a local draft so
// users can cancel without mutating the parent prop.
const editing = ref<string | null>(null)
const draft = ref<LaneDef>({
  id: '',
  label: '',
  color: '#64748b',
  description: '',
  status: 'active',
})
const busy = ref(false)
const errorMsg = ref('')

const sortedLanes = computed(() =>
  [...props.lanes].sort((a, b) => {
    if (a.status !== b.status) return a.status === 'active' ? -1 : 1
    return a.id.localeCompare(b.id)
  }),
)

function startCreate() {
  editing.value = null
  draft.value = {
    id: '',
    label: '',
    color: '#64748b',
    description: '',
    status: 'active',
  }
  errorMsg.value = ''
}

function startEdit(lane: LaneDef) {
  editing.value = lane.id
  draft.value = { ...lane }
  errorMsg.value = ''
}

async function save() {
  errorMsg.value = ''
  busy.value = true
  try {
    if (editing.value) {
      // PATCH only the fields the user can edit; id is fixed once a lane
      // exists (the backend rejects id changes).
      await patchLane(props.project, editing.value, {
        label: draft.value.label,
        color: draft.value.color,
        description: draft.value.description,
        status: draft.value.status,
      })
    } else {
      await upsertLane(props.project, draft.value)
    }
    emit('changed')
    editing.value = null
    startCreate()
  } catch (err) {
    errorMsg.value = err instanceof Error ? err.message : String(err)
  } finally {
    busy.value = false
  }
}

async function archive(lane: LaneDef) {
  if (!confirm(t('laneArchiveConfirm', { id: lane.id }))) return
  busy.value = true
  errorMsg.value = ''
  try {
    const result = await archiveLane(props.project, lane.id)
    if (result.affected_ticket_count > 0) {
      alert(
        t('laneArchiveWarning', { count: result.affected_ticket_count, id: lane.id }),
      )
    }
    emit('changed')
  } catch (err) {
    errorMsg.value = err instanceof Error ? err.message : String(err)
  } finally {
    busy.value = false
  }
}

async function reactivate(lane: LaneDef) {
  busy.value = true
  errorMsg.value = ''
  try {
    await patchLane(props.project, lane.id, { status: 'active' })
    emit('changed')
  } catch (err) {
    errorMsg.value = err instanceof Error ? err.message : String(err)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="lane-manager-overlay" @click.self="emit('close')">
    <div class="lane-manager">
      <header>
        <h2>{{ t('manageLanes') }} · {{ project }}</h2>
        <BbButton variant="secondary" @click="emit('close')">{{ t('close') }}</BbButton>
      </header>

      <section class="lane-manager-list">
        <div
          v-for="lane in sortedLanes"
          :key="lane.id"
          class="lane-row"
          :class="{ archived: lane.status !== 'active' }"
        >
          <span class="lane-swatch" :style="{ background: lane.color || '#94a3b8' }" />
          <div class="lane-row-main">
            <strong>{{ lane.id }} · {{ lane.label }}</strong>
            <p v-if="lane.description">{{ lane.description }}</p>
          </div>
          <div class="lane-row-actions">
            <span v-if="lane.status !== 'active'" class="lane-archived-pill">{{ t('laneArchived') }}</span>
            <BbButton size="sm" variant="secondary" :disabled="busy" @click="startEdit(lane)">{{ t('edit') }}</BbButton>
            <BbButton
              v-if="lane.status === 'active'"
              size="sm"
              variant="secondary"
              :disabled="busy"
              @click="archive(lane)"
            >
              {{ t('laneArchive') }}
            </BbButton>
            <BbButton
              v-else
              size="sm"
              variant="secondary"
              :disabled="busy"
              @click="reactivate(lane)"
            >
              {{ t('laneReactivate') }}
            </BbButton>
          </div>
        </div>
        <BbEmptyState v-if="sortedLanes.length === 0" :message="t('laneEmptyShort')" />
      </section>

      <section class="lane-form">
        <h3>{{ editing ? t('laneEdit', { id: editing }) : t('laneCreate') }}</h3>
        <label>
          <span>{{ t('laneFormId') }}</span>
          <input v-model="draft.id" :disabled="!!editing" :placeholder="t('lanePlaceholderId')" />
        </label>
        <label>
          <span>{{ t('label') }}</span>
          <input v-model="draft.label" :placeholder="t('lanePlaceholderLabel')" />
        </label>
        <label>
          <span>{{ t('laneFormColor') }}</span>
          <input v-model="draft.color" placeholder="#0ea5e9" />
        </label>
        <label>
          <span>{{ t('description') }}</span>
          <input v-model="draft.description" />
        </label>
        <label>
          <span>{{ t('laneFormStatus') }}</span>
          <select v-model="draft.status">
            <option value="active">{{ t('laneStatusActive') }}</option>
            <option value="archived">{{ t('laneStatusArchived') }}</option>
          </select>
        </label>
        <BbInlineAlert v-if="errorMsg" tone="error">{{ errorMsg }}</BbInlineAlert>
        <div class="lane-form-actions">
          <BbButton variant="secondary" :disabled="busy" @click="startCreate">{{ t('laneReset') }}</BbButton>
          <BbButton variant="primary" :disabled="busy" @click="save">
            {{ editing ? t('save') : t('create') }}
          </BbButton>
        </div>
      </section>
    </div>
  </div>
</template>
