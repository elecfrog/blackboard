<script setup lang="ts">
import { computed, ref } from 'vue'
import type { AgentProfile } from '@/data/agents'
import { t } from '@/i18n'

const props = defineProps<{
  agents: AgentProfile[]
  selectedId: string
  assignmentCounts: Map<string, number>
}>()

const emit = defineEmits<{
  select: [id: string]
}>()

const searchQuery = ref('')
const currentPage = ref(1)
const pageSize = 8

const filteredAgents = computed(() => {
  const q = searchQuery.value.toLowerCase().trim()
  if (!q) return props.agents
  return props.agents.filter(
    (a) =>
      a.id.toLowerCase().includes(q) ||
      a.display_name.toLowerCase().includes(q),
  )
})

const totalPages = computed(() => Math.max(1, Math.ceil(filteredAgents.value.length / pageSize)))

const pagedAgents = computed(() => {
  const start = (currentPage.value - 1) * pageSize
  return filteredAgents.value.slice(start, start + pageSize)
})

function statusClass(agent: AgentProfile): string {
  if (agent.status === 'active') return 'online'
  if (agent.status === 'inactive') return 'idle'
  return 'offline'
}

function agentInitial(agent: AgentProfile): string {
  return (agent.display_name || agent.id || '?').slice(0, 1).toUpperCase()
}

function agentColor(id: string): string {
  let hash = 0
  for (const ch of id) hash = ((hash << 5) - hash + ch.charCodeAt(0)) | 0
  const hue = Math.abs(hash) % 360
  return `hsl(${hue}, 55%, 50%)`
}

function selectAgent(id: string) {
  emit('select', id)
}

function goPage(page: number) {
  if (page >= 1 && page <= totalPages.value) currentPage.value = page
}
</script>

<template>
  <aside class="aw-list-panel">
    <header class="aw-list-header">
      <h3 class="aw-list-title">{{ t('agentListTitle') }} ({{ filteredAgents.length }})</h3>
    </header>
    <div class="aw-list-search">
      <input
        v-model="searchQuery"
        type="text"
        class="aw-search-input"
        :placeholder="t('agentSearchPlaceholder')"
        @input="currentPage = 1"
      />
    </div>
    <div class="aw-list-items">
      <button
        v-for="agent in pagedAgents"
        :key="agent.id"
        type="button"
        :class="['aw-list-item', { active: selectedId === agent.id }]"
        @click="selectAgent(agent.id)"
      >
        <span class="aw-list-avatar" :style="{ background: agentColor(agent.id) }">
          {{ agentInitial(agent) }}
        </span>
        <span class="aw-list-info">
          <strong>{{ agent.display_name }}</strong>
          <small>{{ agent.description || agent.kind }}</small>
        </span>
        <span class="aw-list-meta">
          <span :class="['aw-status-dot', statusClass(agent)]" />
          <em v-if="assignmentCounts.get(agent.id)">{{ assignmentCounts.get(agent.id) }}</em>
        </span>
      </button>
      <div v-if="pagedAgents.length === 0" class="aw-list-empty">{{ t('agentNoResults') }}</div>
    </div>
    <footer v-if="totalPages > 1" class="aw-list-pagination">
      <button type="button" :disabled="currentPage <= 1" @click="goPage(currentPage - 1)">‹</button>
      <span>{{ currentPage }} / {{ totalPages }}</span>
      <button type="button" :disabled="currentPage >= totalPages" @click="goPage(currentPage + 1)">›</button>
    </footer>
  </aside>
</template>
