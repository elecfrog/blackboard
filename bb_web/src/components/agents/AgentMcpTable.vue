<script setup lang="ts">
import { computed, ref } from 'vue'
import type { McpServerConfig } from '@/data/agents'
import { BbButton, BbSectionHeader, BbStatusPill } from '@/components/common'
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
    <BbSectionHeader class="aw-section-header" :title="t('mcpServersTitle')" title-tag="h4" :divider="false">
      <template #actions>
        <BbButton v-if="editMode" size="sm" variant="secondary" @click="addServer">
          + {{ t('mcpServerAdd') }}
        </BbButton>
      </template>
    </BbSectionHeader>
    <template v-if="servers.length > 0">
      <table class="aw-table">
        <thead>
          <tr>
            <th>{{ t('mcpServerName') }}</th>
            <th>{{ t('mcpServerType') }}</th>
            <th v-if="editMode" />
          </tr>
        </thead>
        <tbody>
          <tr v-for="(server, idx) in visibleServers" :key="idx">
            <td>
              <template v-if="!editMode">{{ server.name || t('runtimeUnset') }}</template>
              <input v-else :value="server.name" type="text" class="aw-table-input" :placeholder="t('mcpServerNamePlaceholder')" @input="updateField(idx, 'name', ($event.target as HTMLInputElement).value)" />
            </td>
            <td>
              <template v-if="!editMode">
                <BbStatusPill :status="server.transport" :label="server.transport" />
              </template>
              <select v-else :value="server.transport" class="aw-table-select" @change="updateField(idx, 'transport', ($event.target as HTMLSelectElement).value)">
                <option value="stdio">stdio</option>
                <option value="sse">sse</option>
              </select>
            </td>
            <td v-if="editMode">
              <BbButton size="mini" variant="danger" icon-only :aria-label="t('mcpServerRemove')" @click="removeServer(idx)">
                ✕
              </BbButton>
            </td>
          </tr>
        </tbody>
      </table>
    </template>
    <p v-else class="aw-empty-line">{{ t('mcpServersNoneConfigured') }}</p>
    <BbButton
      v-if="!showAll && servers.length > maxVisible"
      size="sm"
      variant="ghost"
      @click="showAll = true"
    >
      {{ t('mcpServersViewAll') }} ({{ servers.length }})
    </BbButton>
  </div>
</template>
