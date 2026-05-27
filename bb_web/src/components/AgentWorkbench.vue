<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { AgentProfile, RuntimeProfile } from '@/data/agents'
import type { SkillInfo } from '@/data/agents'
import { loadAgentRegistry, loadAgentSkills, upsertAgent } from '@/data/agents'
import type { BlackboardTicket, ProjectEntry } from '@/data/tickets'
import { isOpenTicketStatus, loadBlackboardData, loadProjects } from '@/data/tickets'
import { BbButton, BbDialog, BbField, BbToolbar } from '@/components/common'
import { t } from '@/i18n'
import AgentListPanel from './agents/AgentListPanel.vue'
import type { SelectionKind } from './agents/AgentListPanel.vue'
import AgentProfileSection from './agents/AgentProfileSection.vue'
import AgentRuntimeConfig from './agents/AgentRuntimeConfig.vue'
import AgentMcpTable from './agents/AgentMcpTable.vue'
import AgentSkillsTable from './agents/AgentSkillsTable.vue'
import AgentAssignments from './agents/AgentAssignments.vue'

interface ProjectTickets {
  project: ProjectEntry
  tickets: BlackboardTicket[]
}

const props = defineProps<{
  currentProject: string
}>()

const loading = ref(true)
const error = ref('')
const runtimes = ref<RuntimeProfile[]>([])
const agents = ref<AgentProfile[]>([])
const availableSkills = ref<SkillInfo[]>([])
const projects = ref<ProjectTickets[]>([])
const selectedId = ref('')
const skillSaving = ref(false)
const skillSaveError = ref('')

// New agent modal
const showNewAgent = ref(false)
const newAgentForm = ref({
  id: '',
  display_name: '',
  runtime: 'opencode',
  scope: 'global',
  status: 'active',
  assignable: true,
  distribute: false,
  roles: [] as string[],
  variant: '',
})
const newAgentSaving = ref(false)

const selectedAgentProfile = computed(() =>
  agents.value.find((agent) => agent.id === selectedId.value) ?? null,
)

const isSystemAgent = computed(() =>
  selectedAgentProfile.value?.scope === 'system',
)

const assignedProjects = computed(() =>
  projects.value
    .map((entry) => ({
      project: entry.project,
      tickets: entry.tickets.filter(
        (ticket) => ticket.extra.assignee === selectedId.value && isOpenTicketStatus(ticket.status),
      ),
    }))
    .filter((entry) => entry.tickets.length > 0),
)

const assignmentCountByAgent = computed(() => {
  const counts = new Map<string, number>()
  for (const agent of agents.value) counts.set(agent.id, 0)
  for (const project of projects.value) {
    for (const ticket of project.tickets) {
      if (!isOpenTicketStatus(ticket.status)) continue
      const assignee = ticket.extra.assignee
      if (assignee) counts.set(assignee, (counts.get(assignee) ?? 0) + 1)
    }
  }
  return counts
})

function onSelect(id: string, _kind: SelectionKind) {
  selectedId.value = id
  skillSaveError.value = ''
}

function onAgentUpdated(updated: AgentProfile) {
  const idx = agents.value.findIndex((a) => a.id === updated.id)
  if (idx >= 0) agents.value[idx] = updated
}

async function createNewAgent() {
  if (!newAgentForm.value.id || !newAgentForm.value.display_name) return
  newAgentSaving.value = true
  try {
    const created = await upsertAgent(newAgentForm.value as AgentProfile)
    agents.value.push(created)
    selectedId.value = created.id
    showNewAgent.value = false
    newAgentForm.value = {
      id: '',
      display_name: '',
      runtime: 'opencode',
      scope: 'global',
      status: 'active',
      assignable: true,
      distribute: false,
      roles: [],
      variant: '',
    }
  } catch (err) {
    console.error('Failed to create agent:', err)
  } finally {
    newAgentSaving.value = false
  }
}

async function updateAgentSkills(skills: string[]) {
  const agent = selectedAgentProfile.value
  if (!agent) return
  skillSaving.value = true
  skillSaveError.value = ''
  const nextSkills = Array.from(
    new Set(skills.map((skill) => skill.trim()).filter(Boolean)),
  )
  try {
    const updated = await upsertAgent({ ...agent, skills: nextSkills })
    onAgentUpdated(updated)
  } catch (err) {
    skillSaveError.value = err instanceof Error ? err.message : String(err)
  } finally {
    skillSaving.value = false
  }
}

function chooseDefault() {
  if (selectedId.value) return
  if (agents.value.length > 0) {
    selectedId.value = agents.value[0].id
  }
}

async function reload() {
  loading.value = true
  error.value = ''
  try {
    const [registry, projectList] = await Promise.all([loadAgentRegistry(), loadProjects()])
    runtimes.value = registry.runtimes
    agents.value = registry.agents.filter((agent) => agent.status === 'active' && agent.assignable)
    chooseDefault()
    try {
      availableSkills.value = (await loadAgentSkills()).skills
    } catch (err) {
      console.error('Failed to load registered skills:', err)
      availableSkills.value = []
    }
    const payloads = await Promise.all(
      projectList.projects.map(async (project) => ({
        project,
        tickets: (await loadBlackboardData(project.name)).tickets,
      })),
    )
    projects.value = payloads
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

onMounted(reload)
</script>

<template>
  <section class="agent-workbench">
    <header class="bb-workspace-head">
      <div class="bb-workspace-head-main">
        <h2>{{ t('agents') }}</h2>
        <p>{{ t('agentsSubtitle') }}</p>
      </div>
      <BbToolbar class="bb-workspace-head-actions" variant="inline">
        <template #actions>
          <BbButton size="lg" @click="showNewAgent = true">+ {{ t('newAgent') }}</BbButton>
          <BbButton size="lg" @click="reload">{{ t('refresh') }}</BbButton>
        </template>
      </BbToolbar>
    </header>

    <div v-if="loading" class="bb-state-panel">{{ t('loadingAgents') }}</div>
    <div v-else-if="error" class="bb-state-panel bb-error">{{ error }}</div>
    <template v-else>
      <div class="aw-layout">
        <!-- Left: System + Project Agent list -->
        <AgentListPanel
          :agents="agents"
          :selected-id="selectedId"
          :assignment-counts="assignmentCountByAgent"
          @select="onSelect"
        />

        <!-- Right: Content area -->
        <main class="aw-content">
          <!-- Agent detail view -->
          <template v-if="selectedAgentProfile">
            <AgentProfileSection
              :agent="selectedAgentProfile"
              :readonly="isSystemAgent"
              @updated="onAgentUpdated"
            />

            <AgentRuntimeConfig
              :agent="selectedAgentProfile"
              :readonly="isSystemAgent"
              @updated="onAgentUpdated"
            />

            <AgentMcpTable
              :servers="selectedAgentProfile.mcp_servers ?? []"
              :edit-mode="false"
            />

            <AgentSkillsTable
              :skills="selectedAgentProfile.skills ?? []"
              :available-skills="availableSkills"
              :saving="skillSaving"
              :save-error="skillSaveError"
              :readonly="isSystemAgent"
              @update:skills="updateAgentSkills"
            />

            <AgentAssignments
              :projects="assignedProjects"
            />
          </template>
        </main>
      </div>
    </template>

    <BbDialog
      v-if="showNewAgent"
      :title="t('newAgentTitle')"
      :close-label="t('close')"
      @close="showNewAgent = false"
    >
      <BbField>
        <template #label>{{ t('agentIdLabel') }} <em>*</em></template>
        <input v-model="newAgentForm.id" type="text" :placeholder="t('agentIdPlaceholder')" />
      </BbField>
      <BbField>
        <template #label>{{ t('agentDisplayNameLabel') }} <em>*</em></template>
        <input v-model="newAgentForm.display_name" type="text" :placeholder="t('agentDisplayNamePlaceholder')" />
      </BbField>
      <BbField :label="t('runtimeConfigRuntimeLabel')">
        <select v-model="newAgentForm.runtime">
          <option v-for="rt in runtimes" :key="rt.id" :value="rt.id">{{ rt.display_name }}</option>
        </select>
      </BbField>
      <BbField :label="t('agentVariantLabel')">
        <input v-model="newAgentForm.variant" type="text" :placeholder="t('agentVariantPlaceholder')" />
      </BbField>
      <template #footer>
        <BbButton variant="secondary" @click="showNewAgent = false">{{ t('cancel') }}</BbButton>
        <BbButton
          variant="primary"
          :disabled="newAgentSaving || !newAgentForm.id || !newAgentForm.display_name"
          @click="createNewAgent"
        >
          {{ newAgentSaving ? t('creating') : t('create') }}
        </BbButton>
      </template>
    </BbDialog>
  </section>
</template>