<script setup lang="ts">
import { computed, ref } from 'vue'
import BbObjectItem from '@/components/common/BbObjectItem.vue'
import type { AgentProfile } from '@/data/agents'
import { t } from '@/i18n'

export type SelectionKind = 'agent'

const props = defineProps<{
  agents: AgentProfile[]
  selectedId: string
  assignmentCounts: Map<string, number>
}>()

const emit = defineEmits<{
  select: [id: string, kind: SelectionKind]
}>()

const searchQuery = ref('')

const systemAgents = computed(() => {
  const q = searchQuery.value.toLowerCase().trim()
  const list = props.agents.filter((a) => a.scope === 'system')
  if (!q) return list
  return list.filter(
    (a) =>
      a.id.toLowerCase().includes(q) ||
      a.display_name.toLowerCase().includes(q),
  )
})

const projectAgents = computed(() => {
  const q = searchQuery.value.toLowerCase().trim()
  const list = props.agents.filter((a) => a.scope !== 'system')
  if (!q) return list
  return list.filter(
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

function systemColor(id: string): string {
  let hash = 0
  for (const ch of id) hash = ((hash << 5) - hash + ch.charCodeAt(0)) | 0
  const hue = Math.abs(hash) % 360
  return `hsl(${hue}, 40%, 45%)`
}

function selectAgent(id: string) {
  emit('select', id, 'agent')
}

function isActive(id: string) {
  return props.selectedId === id
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
      <!-- System Agents group -->
      <div v-if="systemAgents.length > 0" class="aw-list-group">
        <div class="aw-list-group-label">System Agents ({{ systemAgents.length }})</div>
        <BbObjectItem
          v-for="agent in systemAgents"
          :key="`sys-${agent.id}`"
          class="aw-list-item"
          :title="agent.display_name"
          :active="isActive(agent.id)"
          @select="selectAgent(agent.id)"
        >
          <template #leading>
            <span class="aw-list-avatar aw-list-avatar--system" :style="{ background: systemColor(agent.id) }">
              {{ itemInitial(agent.display_name, agent.id) }}
            </span>
          </template>
          <template #trailing>
            <span class="aw-scope-badge aw-scope-badge--system">SYS</span>
          </template>
        </BbObjectItem>
      </div>

      <!-- Project Agents group -->
      <div v-if="projectAgents.length > 0" class="aw-list-group">
        <div class="aw-list-group-label">Project Agents ({{ projectAgents.length }})</div>
        <BbObjectItem
          v-for="agent in projectAgents"
          :key="`prj-${agent.id}`"
          class="aw-list-item"
          :title="agent.display_name"
          :active="isActive(agent.id)"
          @select="selectAgent(agent.id)"
        >
          <template #leading>
            <span class="aw-list-avatar" :style="{ background: itemColor(agent.id) }">
              {{ itemInitial(agent.display_name, agent.id) }}
            </span>
          </template>
        </BbObjectItem>
      </div>

      <div v-if="systemAgents.length === 0 && projectAgents.length === 0" class="aw-list-empty">
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

.aw-list-avatar--system {
  border-radius: 4px;
}

.aw-scope-badge {
  display: inline-block;
  padding: 1px 5px;
  border-radius: 3px;
  font-size: 9px;
  font-weight: 700;
  letter-spacing: 0.03em;
}

.aw-scope-badge--system {
  color: var(--bb-text-muted);
  background: var(--bb-surface-muted);
}
</style>
