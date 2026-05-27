<script setup lang="ts">
/**
 * SettingsWorkbench — Runtime management workspace.
 * Flat sheet layout: all runtimes displayed as cards in a single scrollable column.
 */

import { computed, onMounted, ref, reactive } from 'vue'
import {
  BbButton,
  BbToolbar,
} from '@/components/common'
import { t } from '@/i18n'
import {
  loadAgentRegistry,
  type AgentRegistryList,
} from '@/data/agents'
import {
  loadRuntimeConnectors,
  patchRuntimeConnectors,
  type RuntimeConnector,
  type RuntimeConnectorList,
} from '@/data/runtimeConnectors'
import {
  loadAgentConnectors,
  syncAgentConnector,
  disconnectAgentConnector,
  type AgentConnector,
  type AgentConnectorList,
} from '@/data/agentConnectors'
import {
  installAgentTool,
  loadAgentTools,
  type AgentTool,
  type AgentToolList,
} from '@/data/agentTools'

const props = defineProps<{
  project: string
}>()

// ── State ──
const loading = ref(true)
const error = ref('')
const registry = ref<AgentRegistryList | null>(null)
const runtimeConnectors = ref<RuntimeConnectorList | null>(null)
const agentConnectorList = ref<AgentConnectorList | null>(null)
const toolList = ref<AgentToolList | null>(null)

// Per-runtime edit drafts
const commandDrafts = reactive<Record<string, string>>({})
const concurrencyDrafts = reactive<Record<string, number>>({})
const savingIds = reactive<Record<string, boolean>>({})
const saveMessages = reactive<Record<string, string>>({})

// Connector busy state
const busyConnectorId = ref<string | null>(null)
const toolBusyId = ref<string | null>(null)



// ── Computed ──
const runtimes = computed(() => registry.value?.runtimes ?? [])

function getRuntimeConnector(id: string): RuntimeConnector | null {
  return runtimeConnectors.value?.runtimes.find((rc) => rc.id === id) ?? null
}

function getAgentConnector(id: string): AgentConnector | null {
  return agentConnectorList.value?.connectors.find((c) => c.id === id) ?? null
}

function getTool(id: string): AgentTool | null {
  return toolList.value?.tools.find((tool) => tool.id === id) ?? null
}

// ── Actions ──
function initDrafts() {
  for (const rt of runtimes.value) {
    const rc = getRuntimeConnector(rt.id)
    commandDrafts[rt.id] = rc?.command ?? ''
    concurrencyDrafts[rt.id] = rc?.max_concurrency ?? 0
  }
}

async function reload() {
  loading.value = true
  error.value = ''
  try {
    const [reg, rtConns, agConns, tools] = await Promise.all([
      loadAgentRegistry(),
      loadRuntimeConnectors(),
      loadAgentConnectors(),
      loadAgentTools(),
    ])
    registry.value = reg
    runtimeConnectors.value = rtConns
    agentConnectorList.value = agConns.data
    toolList.value = tools
    initDrafts()
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

async function saveRuntimeConfig(id: string) {
  if (savingIds[id]) return
  savingIds[id] = true
  saveMessages[id] = ''
  try {
    const next = await patchRuntimeConnectors({
      commands: { [id]: commandDrafts[id] },
      runtime_max_concurrency: { [id]: Math.max(0, Math.floor(concurrencyDrafts[id])) },
    })
    runtimeConnectors.value = next
    saveMessages[id] = t('runtimeConnectorSaved')
    setTimeout(() => { saveMessages[id] = '' }, 2000)
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    savingIds[id] = false
  }
}

async function handleSyncConnector(id: string) {
  if (busyConnectorId.value) return
  busyConnectorId.value = id
  try {
    const updated = await syncAgentConnector(id)
    if (agentConnectorList.value) {
      const idx = agentConnectorList.value.connectors.findIndex((c) => c.id === updated.id)
      if (idx >= 0) agentConnectorList.value.connectors.splice(idx, 1, updated)
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busyConnectorId.value = null
  }
}

async function handleDisconnectConnector(id: string) {
  if (busyConnectorId.value) return
  busyConnectorId.value = id
  try {
    const updated = await disconnectAgentConnector(id)
    if (agentConnectorList.value) {
      const idx = agentConnectorList.value.connectors.findIndex((c) => c.id === updated.id)
      if (idx >= 0) agentConnectorList.value.connectors.splice(idx, 1, updated)
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busyConnectorId.value = null
  }
}

async function handleInstallTool(id: string) {
  if (toolBusyId.value) return
  toolBusyId.value = id
  try {
    const result = await installAgentTool(id)
    if (toolList.value) {
      const idx = toolList.value.tools.findIndex((tool) => tool.id === result.tool.id)
      if (idx >= 0) toolList.value.tools.splice(idx, 1, result.tool)
      else toolList.value.tools.push(result.tool)
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    toolBusyId.value = null
  }
}

// ── Helpers ──
function statusLabel(status: string) {
  if (status === 'connected') return t('runtimeStatusConnected')
  if (status === 'synced') return t('connectorSynced')
  if (status === 'drift') return t('connectorDrift')
  if (status === 'missing') return t('runtimeStatusMissing')
  if (status === 'unreachable') return t('connectorUnreachable')
  return status
}

onMounted(reload)
</script>

<template>
  <section class="settings-workbench">
    <header class="bb-workspace-head">
      <div class="bb-workspace-head-main">
        <h2>{{ t('settings') }}</h2>
        <p>{{ t('settingsSubtitle') }}</p>
      </div>
      <BbToolbar class="bb-workspace-head-actions" variant="inline">
        <template #actions>
          <BbButton size="lg" @click="reload">{{ t('refresh') }}</BbButton>
        </template>
      </BbToolbar>
    </header>

    <div v-if="loading" class="bb-state-panel">{{ t('connectorLoading') }}</div>
    <div v-else-if="error" class="bb-state-panel bb-error">{{ error }}</div>

    <div v-else class="sw-body">
      <h3 class="sw-heading">Runtimes</h3>
      <div class="sw-list">
        <div
          v-for="rt in runtimes"
          :key="rt.id"
          class="sw-item"
        >
          <!-- 第一行：名称 状态 版本 connector状态 roles 操作 -->
          <div class="sw-line sw-line-primary">
            <span class="sw-name">{{ rt.display_name }}</span>
            <span
              v-if="getRuntimeConnector(rt.id)"
              class="sw-pill"
              :data-status="getRuntimeConnector(rt.id)!.status"
            >{{ statusLabel(getRuntimeConnector(rt.id)!.status) }}</span>
            <span class="sw-ver">{{ getRuntimeConnector(rt.id)?.version || '—' }}</span>
            <span
              v-if="getAgentConnector(rt.id)"
              class="sw-pill"
              :data-status="getAgentConnector(rt.id)!.state === 'synced' ? 'connected' : getAgentConnector(rt.id)!.state"
            >{{ statusLabel(getAgentConnector(rt.id)!.state) }}</span>
            <span class="sw-roles">{{ rt.roles?.join(', ') || '' }}</span>
            <div class="sw-actions">
              <BbButton
                v-if="getAgentConnector(rt.id) && getAgentConnector(rt.id)!.state !== 'synced'"
                size="sm"
                variant="primary"
                :disabled="busyConnectorId !== null"
                @click="handleSyncConnector(rt.id)"
              >{{ busyConnectorId === rt.id ? '…' : 'Sync' }}</BbButton>
              <BbButton
                v-if="getAgentConnector(rt.id) && (getAgentConnector(rt.id)!.state === 'synced' || getAgentConnector(rt.id)!.state === 'drift')"
                size="sm"
                variant="secondary"
                :disabled="busyConnectorId !== null"
                @click="handleDisconnectConnector(rt.id)"
              >{{ t('connectorDisconnect') }}</BbButton>
            </div>
          </div>

          <!-- 配置 -->
          <div v-if="getRuntimeConnector(rt.id)" class="sw-line sw-line-config">
            <span class="sw-config-label">max</span>
            <input v-model.number="concurrencyDrafts[rt.id]" type="number" class="sw-input sw-input--num" min="0" step="1" />
            <BbButton size="sm" variant="primary" :disabled="savingIds[rt.id]" @click="saveRuntimeConfig(rt.id)">
              {{ savingIds[rt.id] ? '…' : 'Save' }}
            </BbButton>
            <span v-if="saveMessages[rt.id]" class="sw-ok">✓</span>
          </div>

        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.settings-workbench {
  display: flex;
  flex-direction: column;
  flex: 1 1 0;
  min-height: 0;
  overflow-y: auto;
}

.sw-body {
  padding: 14px 20px 40px;
}

.sw-heading {
  margin: 0 0 8px;
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--bb-text-muted);
}

/* ── 列表 ── */
.sw-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.sw-item {
  display: flex;
  flex-direction: column;
  padding: 12px 16px;
  gap: 6px;
  border: 1px solid var(--bb-hairline);
  border-radius: 10px;
  background: var(--bb-surface);
}

/* ── 行 ── */
.sw-line {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 24px;
}

/* 第一行：主信息 */
.sw-name {
  font-size: 13px;
  font-weight: 700;
  color: var(--bb-text-strong);
  min-width: 80px;
}

.sw-ver {
  font-size: 12px;
  font-family: var(--bb-font-mono);
  color: var(--bb-text-muted);
}

.sw-roles {
  font-size: 11px;
  color: var(--bb-text-muted);
  margin-left: auto;
}

.sw-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

/* 第二行：次要信息 */
.sw-line-secondary {
  font-size: 11px;
  color: var(--bb-text-muted);
}

.sw-bin {
  font-family: var(--bb-font-mono);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 400px;
}

.sw-sep {
  flex-shrink: 0;
}

.sw-cli-info {
  font-size: 11px;
  white-space: nowrap;
}

/* 第三行：配置 */
.sw-line-config {
  margin-top: 2px;
}

.sw-config-label {
  font-size: 11px;
  font-weight: 600;
  color: var(--bb-text-muted);
}

.sw-input {
  box-sizing: border-box;
  padding: 3px 6px;
  border: 1px solid var(--bb-hairline);
  border-radius: 5px;
  font: 11px/1.3 var(--bb-font-mono);
  color: var(--bb-text-strong);
  background: var(--bb-surface);
  min-width: 0;
  width: 160px;
}

.sw-input--num {
  width: 44px;
}

.sw-input:focus {
  outline: 2px solid color-mix(in srgb, var(--bb-focus) 20%, transparent);
  border-color: var(--bb-hairline-strong);
}

.sw-ok {
  color: var(--bb-success);
  font-weight: 700;
  font-size: 11px;
}

/* ── Pill ── */
.sw-pill {
  display: inline-block;
  padding: 2px 7px;
  border-radius: 999px;
  font-size: 10px;
  font-weight: 700;
  line-height: 1.4;
  white-space: nowrap;
  flex-shrink: 0;
}

.sw-pill--sm {
  padding: 1px 5px;
  font-size: 9px;
}

.sw-pill[data-status='connected'] {
  color: var(--bb-success);
  background: color-mix(in srgb, var(--bb-success) 10%, var(--bb-surface));
}

.sw-pill[data-status='missing'] {
  color: var(--bb-text-muted);
  background: var(--bb-surface-muted);
}

.sw-pill[data-status='drift'] {
  color: var(--bb-warning);
  background: color-mix(in srgb, var(--bb-warning) 10%, var(--bb-surface));
}

.sw-pill[data-status='check_failed'] {
  color: var(--bb-error);
  background: color-mix(in srgb, var(--bb-error) 10%, var(--bb-surface));
}
</style>