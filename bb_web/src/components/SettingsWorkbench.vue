<script setup lang="ts">
/**
 * SettingsWorkbench — 重写后的 Settings 页面
 * 左侧：Runtime / Agent 分组列表
 * 右侧：根据选中类型展示 Runtime 详情（含 connector 状态、命令配置）或 Agent Connector 详情
 */

import { computed, onMounted, ref } from 'vue'
import { BbButton, BbInfoGrid, BbInfoItem, BbSectionHeader, BbToolbar } from '@/components/common'
import { t } from '@/i18n'
import {
  loadAgentRegistry,
  type AgentProfile,
  type AgentRegistryList,
  type RuntimeProfile,
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

type SelectionKind = 'runtime' | 'agent'

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

const selectedId = ref('')
const selectedKind = ref<SelectionKind>('runtime')
const saving = ref(false)
const saveMessage = ref('')

// Runtime edit drafts
const commandDraft = ref('')
const concurrencyDraft = ref(0)

// Connector busy state
const busyConnectorId = ref<string | null>(null)
const toolBusyId = ref<string | null>(null)

// ── Computed ──
const runtimes = computed(() => registry.value?.runtimes ?? [])
const agents = computed(() => registry.value?.agents ?? [])

const selectedRuntime = computed(() =>
  selectedKind.value === 'runtime'
    ? runtimes.value.find((rt) => rt.id === selectedId.value) ?? null
    : null,
)

const selectedAgent = computed(() =>
  selectedKind.value === 'agent'
    ? agents.value.find((a) => a.id === selectedId.value) ?? null
    : null,
)

const selectedRuntimeConnector = computed<RuntimeConnector | null>(() => {
  if (!selectedRuntime.value || !runtimeConnectors.value) return null
  return runtimeConnectors.value.runtimes.find((rc) => rc.id === selectedRuntime.value!.id) ?? null
})

const selectedAgentConnector = computed<AgentConnector | null>(() => {
  if (!selectedAgent.value || !agentConnectorList.value) return null
  // Agent's connector is identified by its runtime field
  const runtime = selectedAgent.value.runtime
  if (!runtime) return null
  return agentConnectorList.value.connectors.find((c) => c.id === runtime) ?? null
})

const connectorForRuntime = computed<AgentConnector | null>(() => {
  if (!selectedRuntime.value || !agentConnectorList.value) return null
  return agentConnectorList.value.connectors.find((c) => c.id === selectedRuntime.value!.id) ?? null
})

const toolForRuntime = computed<AgentTool | null>(() => {
  if (!selectedRuntime.value || !toolList.value) return null
  return toolList.value.tools.find((tool) => tool.id === selectedRuntime.value!.id) ?? null
})

const managedAgentsForRuntime = computed(() => {
  if (!selectedRuntime.value) return []
  return agents.value.filter((a) => a.runtime === selectedRuntime.value!.id)
})

// ── Actions ──
function selectItem(id: string, kind: SelectionKind) {
  selectedId.value = id
  selectedKind.value = kind
  saveMessage.value = ''
  // Init edit drafts for runtime
  if (kind === 'runtime') {
    const rc = runtimeConnectors.value?.runtimes.find((r) => r.id === id)
    commandDraft.value = rc?.command ?? ''
    concurrencyDraft.value = rc?.max_concurrency ?? 0
  }
}

function chooseDefault() {
  if (selectedId.value) return
  if (runtimes.value.length > 0) {
    selectItem(runtimes.value[0].id, 'runtime')
  } else if (agents.value.length > 0) {
    selectItem(agents.value[0].id, 'agent')
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
    chooseDefault()
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

async function saveRuntimeConfig() {
  if (!selectedRuntime.value || saving.value) return
  saving.value = true
  saveMessage.value = ''
  try {
    const next = await patchRuntimeConnectors({
      commands: { [selectedRuntime.value.id]: commandDraft.value },
      runtime_max_concurrency: { [selectedRuntime.value.id]: Math.max(0, Math.floor(concurrencyDraft.value)) },
    })
    runtimeConnectors.value = next
    saveMessage.value = t('runtimeConnectorSaved')
    setTimeout(() => { saveMessage.value = '' }, 2000)
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    saving.value = false
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
function runtimeColor(id: string): string {
  let hash = 0
  for (const ch of id) hash = ((hash << 5) - hash + ch.charCodeAt(0)) | 0
  const hue = Math.abs(hash) % 360
  return `hsl(${hue}, 65%, 42%)`
}

function agentColor(id: string): string {
  let hash = 0
  for (const ch of id) hash = ((hash << 5) - hash + ch.charCodeAt(0)) | 0
  const hue = Math.abs(hash) % 360
  return `hsl(${hue}, 55%, 50%)`
}

function statusTone(status: string) {
  if (status === 'connected' || status === 'synced') return 'ok'
  if (status === 'missing') return 'missing'
  if (status === 'drift') return 'warn'
  return 'grey'
}

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
    <template v-else>
      <div class="sw-layout">
        <!-- Left: Runtime + Agent list -->
        <aside class="sw-list-panel">
          <div class="sw-list-search">
            <!-- placeholder for future search -->
          </div>
          <div class="sw-list-items">
            <!-- Runtimes group -->
            <div v-if="runtimes.length > 0" class="sw-list-group">
              <div class="sw-list-group-label">Runtimes ({{ runtimes.length }})</div>
              <button
                v-for="rt in runtimes"
                :key="`rt-${rt.id}`"
                type="button"
                class="sw-list-item"
                :class="{ active: selectedId === rt.id && selectedKind === 'runtime' }"
                @click="selectItem(rt.id, 'runtime')"
              >
                <span class="sw-list-avatar sw-list-avatar--runtime" :style="{ background: runtimeColor(rt.id) }">
                  {{ rt.display_name.slice(0, 1).toUpperCase() }}
                </span>
                <span class="sw-list-item-text">
                  <strong>{{ rt.display_name }}</strong>
                  <span
                    v-if="runtimeConnectors?.runtimes.find((r) => r.id === rt.id)"
                    class="sw-status-dot"
                    :data-tone="statusTone(runtimeConnectors!.runtimes.find((r) => r.id === rt.id)!.status)"
                  />
                </span>
              </button>
            </div>

            <!-- Agents group -->
            <div v-if="agents.length > 0" class="sw-list-group">
              <div class="sw-list-group-label">Agents ({{ agents.length }})</div>
              <button
                v-for="agent in agents"
                :key="`ag-${agent.id}`"
                type="button"
                class="sw-list-item"
                :class="{ active: selectedId === agent.id && selectedKind === 'agent' }"
                @click="selectItem(agent.id, 'agent')"
              >
                <span class="sw-list-avatar" :style="{ background: agentColor(agent.id) }">
                  {{ (agent.display_name || agent.id).slice(0, 1).toUpperCase() }}
                </span>
                <span class="sw-list-item-text">
                  <strong>{{ agent.display_name }}</strong>
                  <code>{{ agent.runtime }}</code>
                </span>
              </button>
            </div>
          </div>
        </aside>

        <!-- Right: Detail panel -->
        <main class="sw-content">
          <!-- ═══ Runtime Detail ═══ -->
          <template v-if="selectedRuntime">
            <section class="sw-detail-card">
              <div class="sw-detail-head">
                <span class="sw-detail-avatar sw-detail-avatar--runtime" :style="{ background: runtimeColor(selectedRuntime.id) }">
                  {{ selectedRuntime.display_name.slice(0, 1).toUpperCase() }}
                </span>
                <div>
                  <h3>{{ selectedRuntime.display_name }}</h3>
                  <code>{{ selectedRuntime.id }}</code>
                </div>
                <span
                  v-if="selectedRuntimeConnector"
                  class="sw-status-badge"
                  :data-tone="statusTone(selectedRuntimeConnector.status)"
                >
                  {{ statusLabel(selectedRuntimeConnector.status) }}
                </span>
              </div>

              <!-- Runtime connector info -->
              <div v-if="selectedRuntimeConnector" class="sw-section">
                <BbSectionHeader :title="t('runtimeConnectorTitle')" title-tag="h4" :divider="false" />
                <BbInfoGrid columns="repeat(2, minmax(0, 1fr))">
                  <BbInfoItem :label="t('runtimeVersion')" :value="selectedRuntimeConnector.version || '-'" variant="mono" />
                  <BbInfoItem :label="t('runtimeConcurrency')" :value="String(selectedRuntimeConnector.max_concurrency ?? 0)" variant="mono" />
                  <BbInfoItem :label="t('runtimeActualLaunch')" :value="selectedRuntimeConnector.resolved_program" variant="mono" />
                  <BbInfoItem v-if="selectedRuntimeConnector.error" label="Error" :value="selectedRuntimeConnector.error" variant="mono" />
                </BbInfoGrid>

                <div class="sw-form-row">
                  <label class="sw-field">
                    <span>{{ t('runtimeCommand') }}</span>
                    <input v-model="commandDraft" spellcheck="false" />
                  </label>
                  <label class="sw-field sw-field-small">
                    <span>{{ t('runtimeMaxConcurrency') }}</span>
                    <input v-model.number="concurrencyDraft" type="number" min="0" step="1" />
                  </label>
                  <BbButton variant="primary" :disabled="saving" @click="saveRuntimeConfig">
                    {{ saving ? t('runtimeSaving') : t('runtimeSaveSettings') }}
                  </BbButton>
                </div>
                <p v-if="saveMessage" class="sw-save-ok">{{ saveMessage }}</p>
              </div>

              <!-- Agent connector (sync AGENTS.md) -->
              <div v-if="connectorForRuntime" class="sw-section">
                <BbSectionHeader title="Agent Connector" title-tag="h4" :divider="false" />
                <div class="sw-connector-status">
                  <span class="sw-status-badge" :data-tone="statusTone(connectorForRuntime.state)">
                    {{ statusLabel(connectorForRuntime.state) }}
                  </span>
                  <div class="sw-connector-actions">
                    <BbButton
                      variant="secondary"
                      :disabled="busyConnectorId !== null"
                      @click="handleSyncConnector(connectorForRuntime!.id)"
                    >
                      {{ busyConnectorId === connectorForRuntime.id ? t('connectorBusy') : t('connectorConnect') }}
                    </BbButton>
                    <BbButton
                      v-if="connectorForRuntime.state === 'synced' || connectorForRuntime.state === 'drift'"
                      variant="secondary"
                      :disabled="busyConnectorId !== null"
                      @click="handleDisconnectConnector(connectorForRuntime!.id)"
                    >
                      {{ t('connectorDisconnect') }}
                    </BbButton>
                  </div>
                </div>
              </div>

              <!-- Tool status -->
              <div v-if="toolForRuntime" class="sw-section">
                <BbSectionHeader title="CLI Tool" title-tag="h4" :divider="false" />
                <div class="sw-tool-status" :data-status="toolForRuntime.status">
                  <BbInfoGrid columns="repeat(2, minmax(0, 1fr))">
                    <BbInfoItem label="Status" :value="toolForRuntime.status" />
                    <BbInfoItem :label="t('agentToolCurrent')" :value="toolForRuntime.current_version || '-'" variant="mono" />
                    <BbInfoItem :label="t('agentToolTarget')" :value="toolForRuntime.target_version" variant="mono" />
                    <BbInfoItem v-if="toolForRuntime.install_command" label="Install command" :value="toolForRuntime.install_command" variant="mono" />
                  </BbInfoGrid>
                  <BbButton
                    :variant="toolForRuntime.status === 'missing' ? 'primary' : 'secondary'"
                    :disabled="toolBusyId !== null"
                    @click="handleInstallTool(toolForRuntime!.id)"
                  >
                    {{ toolBusyId === selectedRuntime!.id ? t('connectorBusy') : toolForRuntime.status === 'missing' ? t('agentToolInstall') : t('agentToolUpdate') }}
                  </BbButton>
                </div>
              </div>

              <!-- Managed agents -->
              <div class="sw-section">
                <BbSectionHeader :title="t('runtimeManagedAgents')" title-tag="h4" :divider="false" />
                <div v-if="managedAgentsForRuntime.length > 0" class="sw-agent-list">
                  <div v-for="agent in managedAgentsForRuntime" :key="agent.id" class="sw-agent-item">
                    <strong>{{ agent.display_name }}</strong>
                    <code>{{ agent.id }}</code>
                    <span v-if="agent.variant" class="sw-agent-variant">{{ agent.variant }}</span>
                  </div>
                </div>
                <p v-else class="sw-muted">{{ t('runtimeNoAgents') }}</p>
              </div>
            </section>
          </template>

          <!-- ═══ Agent Detail ═══ -->
          <template v-if="selectedAgent">
            <section class="sw-detail-card">
              <div class="sw-detail-head">
                <span class="sw-detail-avatar" :style="{ background: agentColor(selectedAgent.id) }">
                  {{ (selectedAgent.display_name || selectedAgent.id).slice(0, 1).toUpperCase() }}
                </span>
                <div>
                  <h3>{{ selectedAgent.display_name }}</h3>
                  <code>{{ selectedAgent.id }}</code>
                </div>
              </div>

              <div class="sw-section">
                <BbSectionHeader title="Profile" title-tag="h4" :divider="false" />
                <BbInfoGrid columns="repeat(2, minmax(0, 1fr))">
                  <BbInfoItem label="Runtime" :value="selectedAgent.runtime || '-'" variant="mono" />
                  <BbInfoItem label="Scope" :value="selectedAgent.scope" />
                  <BbInfoItem label="Status" :value="selectedAgent.status" />
                  <BbInfoItem :label="t('runtimeAssignable')" :value="selectedAgent.assignable ? t('connectorMetaYes') : t('connectorMetaNo')" />
                  <BbInfoItem label="Distribute" :value="selectedAgent.distribute ? t('connectorMetaYes') : t('connectorMetaNo')" />
                  <BbInfoItem :label="t('connectorMetaRoles')" :value="selectedAgent.roles?.join(' / ') || '-'" />
                </BbInfoGrid>
              </div>

              <!-- Agent's connector sync status -->
              <div v-if="selectedAgentConnector" class="sw-section">
                <BbSectionHeader title="Connector Sync" title-tag="h4" :divider="false" />
                <div class="sw-connector-status">
                  <span class="sw-status-badge" :data-tone="statusTone(selectedAgentConnector.state)">
                    {{ statusLabel(selectedAgentConnector.state) }}
                  </span>
                  <span class="sw-muted">via {{ selectedAgentConnector.display_name }}</span>
                </div>
              </div>
            </section>
          </template>
        </main>
      </div>
    </template>
  </section>
</template>

<style scoped>
.settings-workbench {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.sw-layout {
  display: grid;
  grid-template-columns: minmax(240px, 280px) minmax(0, 1fr);
  gap: 12px;
  min-height: 0;
}

/* ── Left panel ── */
.sw-list-panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
  border: 1px solid var(--bb-hairline);
  border-radius: 12px;
  background: var(--bb-surface-soft);
  overflow: hidden;
}

.sw-list-items {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
}

.sw-list-group + .sw-list-group {
  margin-top: 12px;
}

.sw-list-group-label {
  padding: 4px 10px 6px;
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--bb-text-muted);
}

.sw-list-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 8px 10px;
  border: 1px solid transparent;
  border-radius: 8px;
  background: transparent;
  cursor: pointer;
  text-align: left;
  transition: background 0.12s, border-color 0.12s;
}

.sw-list-item:hover {
  background: var(--bb-surface);
  border-color: var(--bb-hairline);
}

.sw-list-item.active {
  background: var(--bb-surface);
  border-color: var(--bb-focus);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--bb-focus) 20%, transparent);
}

.sw-list-avatar {
  flex: 0 0 32px;
  width: 32px;
  height: 32px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  font-weight: 700;
  font-size: 13px;
}

.sw-list-avatar--runtime {
  border-radius: 6px;
}

.sw-list-item-text {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.sw-list-item-text strong {
  color: var(--bb-text-strong);
  font-size: 13px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sw-list-item-text code {
  color: var(--bb-text-muted);
  font-family: var(--bb-font-mono);
  font-size: 11px;
}

.sw-status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--bb-text-muted);
}

.sw-status-dot[data-tone='ok'] { background: var(--bb-success); }
.sw-status-dot[data-tone='warn'] { background: var(--bb-warning); }
.sw-status-dot[data-tone='missing'] { background: var(--bb-text-faint); }

/* ── Right content ── */
.sw-content {
  display: flex;
  flex-direction: column;
  gap: 0;
  min-height: 0;
  overflow-y: auto;
}

.sw-detail-card {
  display: flex;
  flex-direction: column;
  gap: 20px;
  padding: 20px;
  border: 1px solid var(--bb-hairline);
  border-radius: 12px;
  background: var(--bb-surface-soft);
}

.sw-detail-head {
  display: flex;
  align-items: center;
  gap: 12px;
}

.sw-detail-avatar {
  flex: 0 0 40px;
  width: 40px;
  height: 40px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  font-weight: 700;
  font-size: 16px;
}

.sw-detail-avatar--runtime {
  border-radius: 8px;
}

.sw-detail-head h3 {
  margin: 0;
  font-size: 18px;
  color: var(--bb-text-strong);
}

.sw-detail-head code {
  color: var(--bb-text-muted);
  font-family: var(--bb-font-mono);
  font-size: 12px;
}

.sw-section {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-top: 12px;
  border-top: 1px solid var(--bb-hairline);
}

.sw-form-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 100px auto;
  gap: 10px;
  align-items: end;
}

.sw-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.sw-field span {
  color: var(--bb-text-muted);
  font-size: 12px;
}

.sw-field input {
  width: 100%;
  padding: 8px 10px;
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  color: var(--bb-text-strong);
  background: var(--bb-surface);
  font: 13px var(--bb-font-mono);
}

.sw-field input:focus {
  outline: 2px solid color-mix(in srgb, var(--bb-focus) 24%, transparent);
  border-color: var(--bb-focus);
}

.sw-field-small input {
  text-align: center;
}

.sw-save-ok {
  color: var(--bb-success);
  font-size: 12px;
  margin: 0;
}

.sw-status-badge {
  padding: 4px 9px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 700;
}

.sw-status-badge[data-tone='ok'] {
  color: var(--bb-success);
  background: color-mix(in srgb, var(--bb-success) 12%, var(--bb-surface));
}

.sw-status-badge[data-tone='warn'] {
  color: var(--bb-warning);
  background: color-mix(in srgb, var(--bb-warning) 12%, var(--bb-surface));
}

.sw-status-badge[data-tone='missing'] {
  color: var(--bb-text-muted);
  background: var(--bb-surface-muted);
}

.sw-status-badge[data-tone='grey'] {
  color: var(--bb-text-muted);
  background: var(--bb-surface-muted);
}

.sw-connector-status {
  display: flex;
  align-items: center;
  gap: 12px;
}

.sw-connector-actions {
  display: flex;
  gap: 8px;
}

.sw-agent-list {
  display: grid;
  gap: 6px;
}

.sw-agent-item {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 8px 10px;
  border: 1px solid var(--bb-hairline);
  border-radius: 6px;
  background: var(--bb-surface);
  font-size: 13px;
}

.sw-agent-item strong {
  color: var(--bb-text-strong);
}

.sw-agent-item code {
  color: var(--bb-text-muted);
  font-family: var(--bb-font-mono);
  font-size: 12px;
}

.sw-agent-variant {
  color: var(--bb-text-muted);
  font-size: 12px;
  margin-left: auto;
}

.sw-muted {
  color: var(--bb-text-muted);
  font-size: 13px;
  margin: 0;
}

.sw-tool-status {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
  border: 1px solid var(--bb-hairline);
  border-radius: 10px;
  background: var(--bb-surface);
}

.sw-tool-status[data-status='missing'] {
  border-color: color-mix(in srgb, var(--bb-warning) 40%, var(--bb-hairline));
  background: color-mix(in srgb, var(--bb-warning) 4%, var(--bb-surface));
}

.sw-tool-status[data-status='installed'] {
  border-color: color-mix(in srgb, var(--bb-success) 30%, var(--bb-hairline));
}

@media (max-width: 860px) {
  .sw-layout {
    grid-template-columns: 1fr;
  }
}
</style>
