<script setup lang="ts">
import { computed, ref } from 'vue'
import type { McpServerConfig } from '@/data/agents'
import { t } from '@/i18n'

const props = defineProps<{
  servers: McpServerConfig[]
  editMode: boolean
}>()

const emit = defineEmits<{
  'update:servers': [servers: McpServerConfig[]]
}>()

const showAll = ref(false)
const maxVisible = 4

const visibleServers = computed(() => {
  if (showAll.value || props.servers.length <= maxVisible) return props.servers
  return props.servers.slice(0, maxVisible)
})

function statusClass(_server: McpServerConfig): string {
  // No real status from backend yet; assume connected if name is present
  return _server.name ? 'connected' : 'disconnected'
}

function statusLabel(server: McpServerConfig): string {
  return server.name ? t('mcpServerConnected') : t('mcpServerDisconnected')
}

function addServer() {
  const updated = [...props.servers, { name: '', transport: 'stdio' as const, command: '' }]
  emit('update:servers', updated)
}

function removeServer(index: number) {
  const updated = props.servers.filter((_, i) => i !== index)
  emit('update:servers', updated)
}

function updateField(index: number, field: keyof McpServerConfig, value: string) {
  const updated = [...props.servers]
  const server = { ...updated[index] }
  if (field === 'transport') {
    server.transport = value as 'stdio' | 'sse'
  } else {
    ;(server as Record<string, unknown>)[field] = value
  }
  updated[index] = server
  emit('update:servers', updated)
}
</script>

<template>
  <div class="aw-section">
    <div class="aw-section-header">
      <h4>{{ t('mcpServersTitle') }}</h4>
      <div class="aw-section-actions">
        <button v-if="editMode" type="button" class="aw-add-btn" @click="addServer">+ {{ t('mcpServerAdd') }}</button>
      </div>
    </div>
    <template v-if="servers.length > 0">
      <table class="aw-table">
        <thead>
          <tr>
            <th>{{ t('mcpServerName') }}</th>
            <th>{{ t('mcpServerType') }}</th>
            <th>{{ t('mcpServerStatus') }}</th>
            <th v-if="editMode" />
          </tr>
        </thead>
        <tbody>
          <tr v-for="(server, idx) in visibleServers" :key="idx">
            <td>
              <template v-if="!editMode">{{ server.name || '—' }}</template>
              <input v-else :value="server.name" type="text" class="aw-table-input" :placeholder="t('mcpServerNamePlaceholder')" @input="updateField(idx, 'name', ($event.target as HTMLInputElement).value)" />
            </td>
            <td>
              <template v-if="!editMode">
                <span class="aw-transport-badge">{{ server.transport }}</span>
              </template>
              <select v-else :value="server.transport" class="aw-table-select" @change="updateField(idx, 'transport', ($event.target as HTMLSelectElement).value)">
                <option value="stdio">stdio</option>
                <option value="sse">sse</option>
              </select>
            </td>
            <td>
              <span :class="['aw-status-dot', statusClass(server)]" />
              {{ statusLabel(server) }}
            </td>
            <td v-if="editMode">
              <button type="button" class="aw-remove-btn" @click="removeServer(idx)">✕</button>
            </td>
          </tr>
        </tbody>
      </table>
    </template>
    <div v-else class="aw-empty-panel">
      <div class="aw-empty-panel-icon">M</div>
      <div class="aw-empty-panel-body">
        <strong>{{ t('mcpServersNoneConfigured') }}</strong>
        <p>{{ t('mcpServersNoneHint') }}</p>
      </div>
    </div>
    <button
      v-if="!showAll && servers.length > maxVisible"
      type="button"
      class="aw-view-all"
      @click="showAll = true"
    >
      {{ t('mcpServersViewAll') }} ({{ servers.length }})
    </button>
  </div>
</template>
