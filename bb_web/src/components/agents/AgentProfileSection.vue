<script setup lang="ts">
import { computed, ref } from 'vue'
import type { AgentProfile } from '@/data/agents'
import { upsertAgent } from '@/data/agents'
import { t } from '@/i18n'

const props = defineProps<{
  agent: AgentProfile | null
  allAgents: AgentProfile[]
}>()

const emit = defineEmits<{
  updated: [agent: AgentProfile]
  selectAgent: [id: string]
}>()

const editMode = ref(false)
const editForm = ref<Partial<AgentProfile>>({})
const saving = ref(false)
const saveStatus = ref<'' | 'saved' | 'error'>('')

// Org relations add dialog
const showOrgAdd = ref(false)
const orgAddTarget = ref('')

const agentInitial = computed(() =>
  (props.agent?.display_name || props.agent?.id || '?').slice(0, 1).toUpperCase(),
)

function agentColor(id: string): string {
  let hash = 0
  for (const ch of id) hash = ((hash << 5) - hash + ch.charCodeAt(0)) | 0
  const hue = Math.abs(hash) % 360
  return `hsl(${hue}, 55%, 50%)`
}

function statusLabel(status?: string): string {
  if (status === 'active') return t('agentProfileStatusOnline')
  if (status === 'inactive') return t('agentProfileStatusIdle')
  return t('agentProfileStatusOffline')
}

function statusClass(status?: string): string {
  if (status === 'active') return 'online'
  if (status === 'inactive') return 'idle'
  return 'offline'
}

function openEdit() {
  if (!props.agent) return
  editForm.value = JSON.parse(JSON.stringify(props.agent))
  if (!editForm.value.mcp_servers) editForm.value.mcp_servers = []
  if (!editForm.value.skills) editForm.value.skills = []
  if (!editForm.value.workers) editForm.value.workers = []
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

function addOrgRelation() {
  if (!orgAddTarget.value) return
  // Add as worker if current agent is coordinator
  if (editForm.value.org_role === 'coordinator') {
    if (!editForm.value.workers) editForm.value.workers = []
    if (!editForm.value.workers.includes(orgAddTarget.value)) {
      editForm.value.workers.push(orgAddTarget.value)
    }
  } else {
    editForm.value.coordinator = orgAddTarget.value
  }
  orgAddTarget.value = ''
  showOrgAdd.value = false
}

function removeWorker(wid: string) {
  if (editForm.value.workers) {
    editForm.value.workers = editForm.value.workers.filter((w) => w !== wid)
  }
}

const availableAgentsForOrg = computed(() =>
  props.allAgents.filter((a) => a.id !== props.agent?.id),
)

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

    <!-- Profile card -->
    <div class="aw-profile-card">
      <div class="aw-profile-header">
        <div class="aw-profile-left">
          <div class="aw-profile-avatar" :style="{ background: agentColor(agent.id) }">
            {{ agentInitial }}
          </div>
          <div class="aw-profile-fields">
            <template v-if="!editMode">
              <h2 class="aw-profile-name">{{ agent.display_name }}</h2>
              <p class="aw-profile-desc">{{ agent.description || '—' }}</p>
              <div class="aw-profile-tags">
                <span v-for="role in agent.roles" :key="role" class="aw-tag">{{ role }}</span>
                <span v-if="agent.org_role" class="aw-tag org">{{ agent.org_role }}</span>
              </div>
            </template>
            <template v-else>
              <label class="aw-field">
                <span>{{ t('agentProfileName') }} <em>*</em></span>
                <input v-model="editForm.display_name" type="text" required />
              </label>
              <label class="aw-field">
                <span>{{ t('agentProfileDescription') }} <small>{{ descriptionLength }}/500</small></span>
                <textarea v-model="editForm.description" maxlength="500" rows="3" />
              </label>
              <label class="aw-field">
                <span>{{ t('agentProfileSkillsField') }}</span>
                <input
                  :value="(editForm.skills ?? []).join(', ')"
                  type="text"
                  @blur="editForm.skills = ($event.target as HTMLInputElement).value.split(',').map(s => s.trim()).filter(Boolean)"
                />
              </label>
            </template>
          </div>
        </div>
        <div class="aw-profile-actions">
          <template v-if="!editMode">
            <span class="aw-meta-pill">
              <span :class="['aw-status-dot', statusClass(agent.status)]" />
              {{ statusLabel(agent.status) }}
            </span>
            <button type="button" class="aw-edit-link" @click="openEdit">{{ t('agentProfileEditInline') }}</button>
          </template>
          <div v-else class="aw-edit-actions">
            <span v-if="saveStatus === 'saved'" class="aw-save-ok">{{ t('agentProfileSaveOk') }}</span>
            <span v-if="saveStatus === 'error'" class="aw-save-err">{{ t('agentProfileSaveErr') }}</span>
            <button type="button" class="aw-btn secondary" @click="cancelEdit">{{ t('agentEditCancel') }}</button>
            <button type="button" class="aw-btn primary" :disabled="saving" @click="saveEdit">
              {{ saving ? t('agentEditSaving') : t('agentEditSave') }}
            </button>
          </div>
        </div>
      </div>
      <div class="aw-profile-meta-card">
        <div class="aw-meta-grid">
          <div class="aw-meta-cell">
            <span class="aw-meta-label">{{ t('agentProfileRuntime') }}</span>
            <span class="aw-meta-value">
              <template v-if="!editMode">{{ agent.runtime || agent.kind || '—' }}</template>
              <input
                v-else
                v-model="editForm.runtime"
                type="text"
                class="aw-meta-input"
                placeholder="opencode"
              />
            </span>
          </div>
          <div class="aw-meta-cell">
            <span class="aw-meta-label">{{ t('agentProfileModel') }}</span>
            <span class="aw-meta-value">
              <template v-if="!editMode">{{ agent.model || '—' }}</template>
              <input v-else v-model="editForm.model" type="text" class="aw-meta-input" placeholder="e.g. gpt-4o" />
            </span>
          </div>
          <div class="aw-meta-cell">
            <span class="aw-meta-label">{{ t('agentProfileVariant') }}</span>
            <span class="aw-meta-value">
              <template v-if="!editMode">{{ agent.variant || '—' }}</template>
              <input
                v-else
                v-model="editForm.variant"
                type="text"
                class="aw-meta-input"
                placeholder="xhigh / high"
              />
            </span>
          </div>
        </div>
        <div class="aw-meta-strip">
          <div class="aw-meta-stat">
            <span>{{ t('agentProfileRoles') }}</span>
            <strong>{{ agent.roles.length }}</strong>
          </div>
          <div class="aw-meta-stat">
            <span>{{ t('agentSkills') }}</span>
            <strong>{{ agent.skills?.length ?? 0 }}</strong>
          </div>
          <div class="aw-meta-stat">
            <span>{{ t('agentProfileMCP') }}</span>
            <strong>{{ agent.mcp_servers?.length ?? 0 }}</strong>
          </div>
          <div class="aw-meta-stat">
            <span>{{ t('agentProfileWorkers') }}</span>
            <strong>{{ agent.workers?.length ?? 0 }}</strong>
          </div>
        </div>
      </div>
    </div>

    <!-- Org Relations -->
    <div class="aw-section">
      <div class="aw-section-header">
        <h4>{{ t('agentProfileOrganisation') }}</h4>
        <button v-if="editMode" type="button" class="aw-add-btn" @click="showOrgAdd = true">+ {{ t('agentProfileAdd') }}</button>
      </div>
      <div class="aw-org-chips">
        <template v-if="agent.org_role === 'coordinator' && agent.workers?.length">
          <span
            v-for="wid in (editMode ? editForm.workers : agent.workers)"
            :key="wid"
            class="aw-chip worker"
          >
            <span class="aw-chip-icon">👤</span>
            <button type="button" class="aw-chip-label" @click="$emit('selectAgent', wid)">{{ wid }}</button>
            <button v-if="editMode" type="button" class="aw-chip-remove" @click="removeWorker(wid)">✕</button>
          </span>
        </template>
        <template v-if="agent.org_role === 'worker' && agent.coordinator">
          <span class="aw-chip coordinator">
            <span class="aw-chip-icon">👑</span>
            <button type="button" class="aw-chip-label" @click="$emit('selectAgent', editMode ? editForm.coordinator! : agent.coordinator!)">
              {{ editMode ? editForm.coordinator : agent.coordinator }}
            </button>
          </span>
        </template>
        <span v-if="!agent.org_role && !editMode" class="aw-org-empty">{{ t('agentProfileNoOrgRelations') }}</span>
      </div>
      <!-- Add org dialog -->
      <div v-if="showOrgAdd" class="aw-org-add-dialog">
        <select v-model="orgAddTarget">
          <option value="">{{ t('agentProfileSelectAgent') }}</option>
          <option v-for="a in availableAgentsForOrg" :key="a.id" :value="a.id">{{ a.display_name }} ({{ a.id }})</option>
        </select>
        <button type="button" class="aw-btn primary" @click="addOrgRelation">{{ t('agentProfileAdd') }}</button>
        <button type="button" class="aw-btn secondary" @click="showOrgAdd = false">{{ t('agentProfileCancel') }}</button>
      </div>
    </div>

    <!-- Footer timestamps -->
    <div class="aw-profile-footer">
      <span>{{ t('agentProfileCreated') }}: —</span>
      <span>{{ t('agentProfileUpdated') }}: —</span>
    </div>
  </section>
</template>
