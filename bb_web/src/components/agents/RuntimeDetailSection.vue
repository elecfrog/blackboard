<script setup lang="ts">
import { computed } from 'vue'
import type { RuntimeProfile, AgentProfile } from '@/data/agents'
import { BbInfoGrid, BbInfoItem, BbSectionHeader } from '@/components/common'
import { t } from '@/i18n'

const props = defineProps<{
  runtime: RuntimeProfile
  agents: AgentProfile[]
}>()

const managedAgents = computed(() =>
  props.agents.filter((a) => a.runtime === props.runtime.id),
)

function runtimeColor(id: string): string {
  let hash = 0
  for (const ch of id) hash = ((hash << 5) - hash + ch.charCodeAt(0)) | 0
  const hue = Math.abs(hash) % 360
  return `hsl(${hue}, 65%, 42%)`
}

const initial = computed(() =>
  (props.runtime.display_name || props.runtime.id || '?').slice(0, 1).toUpperCase(),
)
</script>

<template>
  <section class="aw-profile">
    <nav class="aw-breadcrumb">
      <span>Runtimes</span>
      <span class="aw-breadcrumb-sep">›</span>
      <span class="aw-breadcrumb-current">{{ runtime.display_name }}</span>
    </nav>

    <div class="aw-profile-summary">
      <div class="aw-profile-header">
        <div class="aw-profile-left">
          <div class="aw-profile-avatar runtime-avatar" :style="{ background: runtimeColor(runtime.id) }">
            {{ initial }}
          </div>
          <div class="aw-profile-fields">
            <h2 class="aw-profile-name">{{ runtime.display_name }}</h2>
            <p class="aw-profile-desc">{{ runtime.description || t('runtimeUnset') }}</p>
          </div>
        </div>
      </div>
    </div>

    <div class="aw-section">
      <BbSectionHeader :title="t('runtimeDetailTitle')" title-tag="h4" :divider="false" />
      <BbInfoGrid columns="repeat(2, minmax(0, 1fr))">
        <BbInfoItem label="ID" :value="runtime.id" />
        <BbInfoItem :label="t('runtimeAssignable')" :value="runtime.assignable ? t('connectorMetaYes') : t('connectorMetaNo')" />
        <BbInfoItem :label="t('runtimeCommand')" :value="runtime.command || '-'" />
        <BbInfoItem :label="t('connectorMetaRoles')" :value="runtime.roles?.join(' / ') || '-'" />
      </BbInfoGrid>
    </div>

    <div v-if="managedAgents.length > 0" class="aw-section">
      <BbSectionHeader :title="t('runtimeManagedAgents')" title-tag="h4" :divider="false" />
      <div class="runtime-agents-list">
        <div v-for="agent in managedAgents" :key="agent.id" class="runtime-agent-item">
          <strong>{{ agent.display_name }}</strong>
          <span class="runtime-agent-id">{{ agent.id }}</span>
          <span v-if="agent.variant" class="runtime-agent-variant">{{ agent.variant }}</span>
        </div>
      </div>
    </div>
    <div v-else class="aw-section">
      <BbSectionHeader :title="t('runtimeManagedAgents')" title-tag="h4" :divider="false" />
      <p class="runtime-no-agents">{{ t('runtimeNoAgents') }}</p>
    </div>
  </section>
</template>

<style scoped>
.runtime-avatar {
  border-radius: 6px;
}

.runtime-agents-list {
  display: grid;
  gap: 6px;
}

.runtime-agent-item {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 8px 10px;
  border: 1px solid var(--bb-hairline);
  border-radius: 6px;
  background: var(--bb-surface);
  font-size: 13px;
}

.runtime-agent-item strong {
  color: var(--bb-text-strong);
}

.runtime-agent-id {
  color: var(--bb-text-muted);
  font-family: var(--bb-font-mono);
  font-size: 12px;
}

.runtime-agent-variant {
  color: var(--bb-text-muted);
  font-size: 12px;
  margin-left: auto;
}

.runtime-no-agents {
  color: var(--bb-text-muted);
  font-size: 13px;
  margin: 0;
}
</style>
