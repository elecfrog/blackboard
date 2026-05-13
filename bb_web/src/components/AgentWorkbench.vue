<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { AgentProfile } from '@/data/agents'
import { loadAgentRegistry, upsertAgent } from '@/data/agents'
import type { BlackboardTicket, ProjectEntry } from '@/data/tickets'
import { isOpenTicketStatus, loadBlackboardData, loadProjects } from '@/data/tickets'
import { t } from '@/i18n'
import AgentListPanel from './agents/AgentListPanel.vue'
import AgentProfileSection from './agents/AgentProfileSection.vue'
import AgentMcpTable from './agents/AgentMcpTable.vue'
import AgentSkillsTable from './agents/AgentSkillsTable.vue'
import AgentMetricsCards from './agents/AgentMetricsCards.vue'
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
const agents = ref<AgentProfile[]>([])
const projects = ref<ProjectTickets[]>([])
const selectedAgent = ref('')

// New agent modal
const showNewAgent = ref(false)
const newAgentForm = ref({
  id: '',
  display_name: '',
  kind: 'opencode',
  scope: 'global',
  status: 'active',
  assignable: true,
  distribute: false,
  roles: [] as string[],
  variant: '',
})
const newAgentSaving = ref(false)

// Edit mode state (shared with profile section)
const editMode = ref(false)

const selectedAgentProfile = computed(() =>
  agents.value.find((agent) => agent.id === selectedAgent.value) ?? null,
)

const assignedProjects = computed(() =>
  projects.value
    .map((entry) => ({
      project: entry.project,
      tickets: entry.tickets.filter(
        (ticket) => ticket.extra.assignee === selectedAgent.value && isOpenTicketStatus(ticket.status),
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

function selectAgent(id: string) {
  selectedAgent.value = id
  editMode.value = false
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
    selectedAgent.value = created.id
    showNewAgent.value = false
    newAgentForm.value = {
      id: '',
      display_name: '',
      kind: 'opencode',
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

function chooseDefaultAgent() {
  if (selectedAgent.value) return
  const preferred = agents.value.find((agent) => agent.id === 'codex') ?? agents.value[0]
  selectedAgent.value = preferred?.id ?? ''
}

async function reload() {
  loading.value = true
  error.value = ''
  try {
    const [registry, projectList] = await Promise.all([loadAgentRegistry(), loadProjects()])
    agents.value = registry.agents.filter((agent) => agent.status === 'active' && agent.assignable)
    chooseDefaultAgent()
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
      <div class="bb-workspace-head-actions">
        <button type="button" class="bb-top-action-button" @click="showNewAgent = true">+ {{ t('newAgent') }}</button>
        <button type="button" class="bb-top-action-button" @click="reload">{{ t('refresh') }}</button>
      </div>
    </header>

    <div v-if="loading" class="bb-state-panel">{{ t('loadingAgents') }}</div>
    <div v-else-if="error" class="bb-state-panel bb-error">{{ error }}</div>
    <template v-else>
      <div class="aw-layout">
        <!-- Left: Agent list -->
        <AgentListPanel
          :agents="agents"
          :selected-id="selectedAgent"
          :assignment-counts="assignmentCountByAgent"
          @select="selectAgent"
        />

        <!-- Right: Content area -->
        <main class="aw-content">
          <AgentProfileSection
            :agent="selectedAgentProfile"
            :all-agents="agents"
            @updated="onAgentUpdated"
            @select-agent="selectAgent"
          />

          <AgentMcpTable
            v-if="selectedAgentProfile"
            :servers="selectedAgentProfile.mcp_servers ?? []"
            :edit-mode="false"
          />

          <AgentSkillsTable
            v-if="selectedAgentProfile"
            :skills="selectedAgentProfile.skills ?? []"
            :edit-mode="false"
          />

          <AgentMetricsCards v-if="selectedAgentProfile" />

          <AgentAssignments
            v-if="selectedAgentProfile"
            :projects="assignedProjects"
          />
        </main>
      </div>
    </template>

    <!-- New Agent Modal -->
    <div v-if="showNewAgent" class="aw-modal-overlay" @click.self="showNewAgent = false">
      <div class="aw-modal">
        <header class="aw-modal-header">
          <h3>{{ t('newAgentTitle') }}</h3>
          <button type="button" class="aw-modal-close" @click="showNewAgent = false">✕</button>
        </header>
        <div class="aw-modal-body">
          <label class="aw-field">
            <span>{{ t('agentIdLabel') }} <em>*</em></span>
            <input v-model="newAgentForm.id" type="text" :placeholder="t('agentIdPlaceholder')" />
          </label>
          <label class="aw-field">
            <span>{{ t('agentDisplayNameLabel') }} <em>*</em></span>
            <input v-model="newAgentForm.display_name" type="text" :placeholder="t('agentDisplayNamePlaceholder')" />
          </label>
          <label class="aw-field">
            <span>{{ t('agentKindLabel') }}</span>
            <select v-model="newAgentForm.kind">
              <option value="opencode">opencode</option>
              <option value="codex">codex</option>
              <option value="codebuddy">codebuddy</option>
              <option value="custom">custom</option>
            </select>
          </label>
          <label class="aw-field">
            <span>{{ t('agentVariantLabel') }}</span>
            <input v-model="newAgentForm.variant" type="text" :placeholder="t('agentVariantPlaceholder')" />
          </label>
        </div>
        <footer class="aw-modal-footer">
          <button type="button" class="aw-btn secondary" @click="showNewAgent = false">{{ t('cancel') }}</button>
          <button type="button" class="aw-btn primary" :disabled="newAgentSaving || !newAgentForm.id || !newAgentForm.display_name" @click="createNewAgent">
            {{ newAgentSaving ? t('creating') : t('create') }}
          </button>
        </footer>
      </div>
    </div>
  </section>
</template>
