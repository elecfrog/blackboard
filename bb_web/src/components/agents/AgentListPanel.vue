<script setup lang="ts">
import { computed, ref } from 'vue'
import BbObjectItem from '@/components/common/BbObjectItem.vue'
import type { AgentProfile, RuntimeProfile } from '@/data/agents'
import { t } from '@/i18n'

export type SelectionKind = 'runtime' | 'agent'

const props = defineProps<{
  runtimes: RuntimeProfile[]
  agents: AgentProfile[]
  selectedId: string
  selectedKind: SelectionKind
  assignmentCounts: Map<string, number>
}>()

const emit = defineEmits<{
  select: [id: string, kind: SelectionKind]
}>()

const searchQuery = ref('')

const filteredRuntimes = computed(() => {
  const q = searchQuery.value.toLowerCase().trim()
  if (!q) return props.runtimes
  return props.runtimes.filter(
    (rt) =>
      rt.id.toLowerCase().includes(q) ||
      rt.display_name.toLowerCase().includes(q),
  )
})

const filteredAgents = computed(() => {
  const q = searchQuery.value.toLowerCase().trim()
  if (!q) return props.agents
  return props.agents.filter(
    (a) =>
      a.id.toLowerCase().includes(q) ||
      a.display_name.toLowerCase().includes(q),
  )
})

function itemInitial(name: string, id: string): string {
  return (name || id || '?').slice(0, 1).toUpperCase()
}

function itemColor(id: string): string {
  let hash = 0
  for (const ch of id) hash = ((hash << 5) - hash + ch.charCodeAt(0)) | 0
  const hue = Math.abs(hash) % 360
  return `hsl(${hue}, 55%, 50%)`
}

function runtimeColor(id: string): string {
  let hash = 0
  for (const ch of id) hash = ((hash << 5) - hash + ch.charCodeAt(0)) | 0
  const hue = Math.abs(hash) % 360
  return `hsl(${hue}, 65%, 42%)`
}

function selectRuntime(id: string) {
  emit('select', id, 'runtime')
}

function selectAgent(id: string) {
  emit('select', id, 'agent')
}

function isActive(id: string, kind: SelectionKind) {
  return props.selectedId === id && props.selectedKind === kind
}
</script>

<template>
  <aside class="aw-list-panel">
    <header class="aw-list-header">
      <h3 class="aw-list-title">{{ t('agentListTitle') }}</h3>
    </header>
    <div class="aw-list-search">
      <input
        v-model="searchQuery"
        type="text"
        class="aw-search-input"
        :placeholder="t('agentSearchPlaceholder')"
      />
    </div>
    <div class="bb-object-list aw-list-items">
      <!-- Runtimes group -->
      <div v-if="filteredRuntimes.length > 0" class="aw-list-group">
        <div class="aw-list-group-label">Runtimes ({{ filteredRuntimes.length }})</div>
        <BbObjectItem
          v-for="rt in filteredRuntimes"
          :key="`rt-${rt.id}`"
          class="aw-list-item"
          :title="rt.display_name"
          :active="isActive(rt.id, 'runtime')"
          @select="selectRuntime(rt.id)"
        >
          <template #leading>
            <span class="aw-list-avatar aw-list-avatar--runtime" :style="{ background: runtimeColor(rt.id) }">
              {{ itemInitial(rt.display_name, rt.id) }}
            </span>
          </template>
        </BbObjectItem>
      </div>

      <!-- Agents group -->
      <div v-if="filteredAgents.length > 0" class="aw-list-group">
        <div class="aw-list-group-label">Agents ({{ filteredAgents.length }})</div>
        <BbObjectItem
          v-for="agent in filteredAgents"
          :key="`ag-${agent.id}`"
          class="aw-list-item"
          :title="agent.display_name"
          :active="isActive(agent.id, 'agent')"
          @select="selectAgent(agent.id)"
        >
          <template #leading>
            <span class="aw-list-avatar" :style="{ background: itemColor(agent.id) }">
              {{ itemInitial(agent.display_name, agent.id) }}
            </span>
          </template>
        </BbObjectItem>
      </div>

      <div v-if="filteredRuntimes.length === 0 && filteredAgents.length === 0" class="aw-list-empty">
        {{ t('agentNoResults') }}
      </div>
    </div>
  </aside>
</template>

<style scoped>
.aw-list-group {
  display: flex;
  flex-direction: column;
}

.aw-list-group + .aw-list-group {
  margin-top: 12px;
}

.aw-list-group-label {
  padding: 4px 12px 6px;
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--bb-text-muted);
}

.aw-list-avatar--runtime {
  border-radius: 4px;
}
</style>
