<script setup lang="ts">
import { computed, ref } from 'vue'
import type { AgentProfile } from '@/data/agents'
import { upsertAgent } from '@/data/agents'
import { BbButton, BbField } from '@/components/common'
import { t } from '@/i18n'

const props = defineProps<{
  agent: AgentProfile | null
  readonly?: boolean
}>()

const emit = defineEmits<{
  updated: [agent: AgentProfile]
}>()

const editMode = ref(false)
const editForm = ref<Partial<AgentProfile>>({})
const saving = ref(false)
const saveStatus = ref<'' | 'saved' | 'error'>('')

const agentInitial = computed(() =>
  (props.agent?.display_name || props.agent?.id || '?').slice(0, 1).toUpperCase(),
)

function agentColor(id: string): string {
  let hash = 0
  for (const ch of id) hash = ((hash << 5) - hash + ch.charCodeAt(0)) | 0
  const hue = Math.abs(hash) % 360
  return `hsl(${hue}, 55%, 50%)`
}

function openEdit() {
  if (!props.agent) return
  editForm.value = JSON.parse(JSON.stringify(props.agent))
  if (!editForm.value.mcp_servers) editForm.value.mcp_servers = []
  saveStatus.value = ''
  editMode.value = true
}

function cancelEdit() {
  editMode.value = false
  saveStatus.value = ''
}

async function saveEdit() {
  if (!editForm.value.id) return
  saving.value = true
  saveStatus.value = ''
  try {
    const updated = await upsertAgent(editForm.value as AgentProfile)
    emit('updated', updated)
    saveStatus.value = 'saved'
    setTimeout(() => {
      editMode.value = false
      saveStatus.value = ''
    }, 600)
  } catch {
    saveStatus.value = 'error'
  } finally {
    saving.value = false
  }
}

const descriptionLength = computed(() => (editForm.value.description || '').length)
</script>

<template>
  <section v-if="agent" class="aw-profile">
    <!-- Breadcrumb -->
    <nav class="aw-breadcrumb">
      <span>{{ t('agentProfileBreadcrumb') }}</span>
      <span class="aw-breadcrumb-sep">›</span>
      <span class="aw-breadcrumb-current">{{ agent.display_name }}</span>
    </nav>

    <div class="aw-profile-summary">
      <div class="aw-profile-header">
        <div class="aw-profile-left">
          <div class="aw-profile-avatar" :style="{ background: agentColor(agent.id) }">
            {{ agentInitial }}
          </div>
          <div class="aw-profile-fields">
            <template v-if="!editMode">
              <h2 class="aw-profile-name">{{ agent.display_name }}</h2>
              <p class="aw-profile-desc">{{ agent.description || t('runtimeUnset') }}</p>
            </template>
            <template v-else>
              <BbField>
                <template #label>{{ t('agentProfileName') }} <em>*</em></template>
                <input v-model="editForm.display_name" type="text" required />
              </BbField>
              <BbField>
                <template #label>{{ t('agentProfileDescription') }} <small>{{ descriptionLength }}/500</small></template>
                <textarea v-model="editForm.description" maxlength="500" rows="3" />
              </BbField>
            </template>
          </div>
        </div>
        <div class="aw-profile-actions">
          <span v-if="readonly" class="aw-system-badge">System</span>
          <template v-if="!editMode && !readonly">
            <button type="button" class="aw-edit-link" @click="openEdit">{{ t('agentProfileEditInline') }}</button>
          </template>
          <div v-else class="aw-edit-actions">
            <span v-if="saveStatus === 'saved'" class="aw-save-ok">{{ t('agentProfileSaveOk') }}</span>
            <span v-if="saveStatus === 'error'" class="aw-save-err">{{ t('agentProfileSaveErr') }}</span>
            <BbButton variant="secondary" @click="cancelEdit">{{ t('agentEditCancel') }}</BbButton>
            <BbButton variant="primary" :disabled="saving" @click="saveEdit">
              {{ saving ? t('agentEditSaving') : t('agentEditSave') }}
            </BbButton>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.aw-system-badge {
  display: inline-block;
  padding: 3px 8px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.03em;
  color: var(--bb-text-muted);
  background: var(--bb-surface-muted);
}
</style>
