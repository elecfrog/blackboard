<script setup lang="ts">
/**
 * SettingsWorkbench — Settings workspace.
 * Left: Runtime / Agent grouped list (title-only BbObjectItem).
 * Right: Detail panel for selected item.
 */

import { computed, onMounted, ref } from 'vue'
import {
  BbButton,
  BbInfoGrid,
  BbInfoItem,
  BbSectionHeader,
  BbToolbar,
  BbInlineAlert,
} from '@/components/common'
import BbObjectItem from '@/components/common/BbObjectItem.vue'
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
function runtimeConnectorStatus(id: string): string | undefined {
  return runtimeConnectors.value?.runtimes.find((r) => r.id === id)?.status
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
      <div class="aw-layout">
        <!-- Left: grouped list -->
        <aside class="aw-list-panel">
          <div class="bb-object-list aw-list-items">
            <div v-if="runtimes.length > 0" class="aw-list-group">
              <div class="aw-list-group-label">Runtimes ({{ runtimes.length }})</div>
              <BbObjectItem
                v-for="rt in runtimes"
                :key="`rt-${rt.id}`"
                :title="rt.display_name"
                :active="selectedId === rt.id && selectedKind === 'runtime'"
                @select="selectItem(rt.id, 'runtime')"
              >
                <template #trailing>
                  <span
                    class="aw-status-dot"
                    :data-status="runtimeConnectorStatus(rt.id) ?? 'unknown'"
                  />
                </template>
              </BbObjectItem>
            </div>

            <div v-if="agents.length > 0" class="aw-list-group">
              <div class="aw-list-group-label">Agents ({{ agents.length }})</div>
              <BbObjectItem
                v-for="agent in agents"
                :key="`ag-${agent.id}`"
                :title="agent.display_name"
                :active="selectedId === agent.id && selectedKind === 'agent'"
                @select="selectItem(agent.id, 'agent')"
              />
            </div>
          </div>
        </aside>

        <!-- Right: detail -->
        <main class="aw-content">
          <!-- Runtime detail -->
          <template v-if="selectedRuntime">
            <div class="aw-section">
              <BbSectionHeader :title="selectedRuntime.display_name" title-tag="h3" :divider="false">
                <template #actions>
                  <span
                    v-if="selectedRuntimeConnector"
                    class="aw-status-pill"
                    :data-status="selectedRuntimeConnector.status"
                  >
                    {{ statusLabel(selectedRuntimeConnector.status) }}
                  </span>
                </template>
              </BbSectionHeader>
              <BbInfoGrid columns="repeat(2, minmax(0, 1fr))">
                <BbInfoItem label="ID" :value="selectedRuntime.id" variant="mono" />
                <BbInfoItem :label="t('runtimeAssignable')" :value="selectedRuntime.assignable ? t('connectorMetaYes') : t('connectorMetaNo')" />
                <BbInfoItem :label="t('connectorMetaRoles')" :value="selectedRuntime.roles?.join(', ') || '—'" />
                <BbInfoItem v-if="selectedRuntime.description" label="Description" :value="selectedRuntime.description" />
              </BbInfoGrid>
            </div>

            <!-- Runtime connector config -->
            <div v-if="selectedRuntimeConnector" class="aw-section">
              <BbSectionHeader :title="t('runtimeConnectorTitle')" title-tag="h4" :divider="false" />
              <BbInfoGrid columns="repeat(2, minmax(0, 1fr))">
                <BbInfoItem :label="t('runtimeVersion')" :value="selectedRuntimeConnector.version || '—'" variant="mono" />
                <BbInfoItem :label="t('runtimeConcurrency')" :value="String(selectedRuntimeConnector.max_concurrency ?? 0)" variant="mono" />
                <BbInfoItem :label="t('runtimeActualLaunch')" :value="selectedRuntimeConnector.resolved_program" variant="mono" />
                <BbInfoItem v-if="selectedRuntimeConnector.error" label="Error" :value="selectedRuntimeConnector.error" />
              </BbInfoGrid>

              <div class="aw-dense-form">
                <label class="aw-dense-field">
                  <span class="aw-dense-label">{{ t('runtimeCommand') }}</span>
                  <input v-model="commandDraft" type="text" spellcheck="false" />
                </label>
                <label class="aw-dense-field aw-dense-field--narrow">
                  <span class="aw-dense-label">{{ t('runtimeMaxConcurrency') }}</span>
                  <input v-model.number="concurrencyDraft" type="number" min="0" step="1" />
                </label>
                <BbButton variant="primary" :disabled="saving" @click="saveRuntimeConfig">
                  {{ saving ? t('runtimeSaving') : t('runtimeSaveSettings') }}
                </BbButton>
              </div>
              <BbInlineAlert v-if="saveMessage" tone="success">{{ saveMessage }}</BbInlineAlert>
            </div>

            <!-- CLI Tool -->
            <div v-if="toolForRuntime" class="aw-section">
              <BbSectionHeader title="CLI Tool" title-tag="h4" :divider="false">
                <template #actions>
                  <span class="aw-status-pill" :data-status="toolForRuntime.status === 'installed' ? 'connected' : 'missing'">
                    {{ toolForRuntime.status }}
                  </span>
                </template>
              </BbSectionHeader>
              <BbInfoGrid columns="repeat(2, minmax(0, 1fr))">
                <BbInfoItem :label="t('agentToolCurrent')" :value="toolForRuntime.current_version || '—'" variant="mono" />
                <BbInfoItem :label="t('agentToolTarget')" :value="toolForRuntime.target_version" variant="mono" />
                <BbInfoItem v-if="toolForRuntime.install_command" label="Install" :value="toolForRuntime.install_command" variant="mono" />
              </BbInfoGrid>
              <BbButton
                :variant="toolForRuntime.status === 'missing' ? 'primary' : 'secondary'"
                :disabled="toolBusyId !== null"
                @click="handleInstallTool(toolForRuntime!.id)"
              >
                {{ toolBusyId === selectedRuntime!.id ? t('connectorBusy') : toolForRuntime.status === 'missing' ? t('agentToolInstall') : t('agentToolUpdate') }}
              </BbButton>
            </div>

            <!-- Agent connector sync -->
            <div v-if="connectorForRuntime" class="aw-section">
              <BbSectionHeader title="Agent Connector" title-tag="h4" :divider="false">
                <template #actions>
                  <span class="aw-status-pill" :data-status="connectorForRuntime.state === 'synced' ? 'connected' : connectorForRuntime.state">
                    {{ statusLabel(connectorForRuntime.state) }}
                  </span>
                </template>
              </BbSectionHeader>
              <div class="aw-action-row">
                <BbButton
                  variant="primary"
                  :disabled="busyConnectorId !== null"
                  @click="handleSyncConnector(connectorForRuntime!.id)"
                >
                  {{ busyConnectorId === connectorForRuntime!.id ? t('connectorBusy') : t('connectorConnect') }}
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

            <!-- Managed agents -->
            <div class="aw-section">
              <BbSectionHeader :title="t('runtimeManagedAgents')" title-tag="h4" :divider="false" />
              <div v-if="managedAgentsForRuntime.length > 0" class="aw-managed-list">
                <BbInfoGrid v-for="agent in managedAgentsForRuntime" :key="agent.id" columns="repeat(3, minmax(0, 1fr))">
                  <BbInfoItem label="Agent" :value="agent.display_name" />
                  <BbInfoItem label="ID" :value="agent.id" variant="mono" />
                  <BbInfoItem label="Variant" :value="agent.variant || '—'" variant="mono" />
                </BbInfoGrid>
              </div>
              <p v-else class="aw-empty-hint">{{ t('runtimeNoAgents') }}</p>
            </div>
          </template>

          <!-- Agent detail -->
          <template v-if="selectedAgent">
            <div class="aw-section">
              <BbSectionHeader :title="selectedAgent.display_name" title-tag="h3" :divider="false" />
              <BbInfoGrid columns="repeat(2, minmax(0, 1fr))">
                <BbInfoItem label="ID" :value="selectedAgent.id" variant="mono" />
                <BbInfoItem label="Runtime" :value="selectedAgent.runtime || '—'" variant="mono" />
                <BbInfoItem label="Scope" :value="selectedAgent.scope" />
                <BbInfoItem label="Status" :value="selectedAgent.status" />
                <BbInfoItem :label="t('runtimeAssignable')" :value="selectedAgent.assignable ? t('connectorMetaYes') : t('connectorMetaNo')" />
                <BbInfoItem label="Distribute" :value="selectedAgent.distribute ? t('connectorMetaYes') : t('connectorMetaNo')" />
                <BbInfoItem :label="t('connectorMetaRoles')" :value="selectedAgent.roles?.join(', ') || '—'" />
              </BbInfoGrid>
            </div>

            <div v-if="selectedAgentConnector" class="aw-section">
              <BbSectionHeader title="Connector Sync" title-tag="h4" :divider="false">
                <template #actions>
                  <span class="aw-status-pill" :data-status="selectedAgentConnector.state === 'synced' ? 'connected' : selectedAgentConnector.state">
                    {{ statusLabel(selectedAgentConnector.state) }}
                  </span>
                </template>
              </BbSectionHeader>
              <BbInfoGrid columns="1fr">
                <BbInfoItem label="via" :value="selectedAgentConnector.display_name" />
              </BbInfoGrid>
            </div>
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

/* Reuse workbench two-column layout from styles.css (.aw-layout) */

.aw-list-group + .aw-list-group {
  margin-top: 12px;
}

.aw-list-group-label {
  padding: 4px 10px 6px;
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--bb-text-muted);
}

.aw-status-dot {
  display: block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--bb-text-faint);
}

.aw-status-dot[data-status='connected'] { background: var(--bb-success); }
.aw-status-dot[data-status='missing'] { background: var(--bb-text-faint); }
.aw-status-dot[data-status='check_failed'] { background: var(--bb-error); }

.aw-status-pill {
  display: inline-block;
  padding: 3px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 700;
}

.aw-status-pill[data-status='connected'] {
  color: var(--bb-success);
  background: color-mix(in srgb, var(--bb-success) 10%, var(--bb-surface));
}

.aw-status-pill[data-status='missing'] {
  color: var(--bb-text-muted);
  background: var(--bb-surface-muted);
}

.aw-status-pill[data-status='drift'] {
  color: var(--bb-warning);
  background: color-mix(in srgb, var(--bb-warning) 10%, var(--bb-surface));
}

/* Dense form — follows box model rules from DESIGN.md */
.aw-dense-form {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 100px auto;
  gap: 10px;
  align-items: end;
}

.aw-dense-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.aw-dense-field--narrow {
  max-width: 100px;
}

.aw-dense-label {
  color: var(--bb-text-muted);
  font-size: 11px;
  font-weight: 600;
}

.aw-dense-field input {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  padding: 7px 9px;
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  color: var(--bb-text-strong);
  background: var(--bb-surface);
  font: 13px/1.3 var(--bb-font-mono);
}

.aw-dense-field input:focus {
  outline: 2px solid color-mix(in srgb, var(--bb-focus) 20%, transparent);
  border-color: var(--bb-hairline-strong);
}

.aw-action-row {
  display: flex;
  gap: 8px;
}

.aw-managed-list {
  display: grid;
  gap: 8px;
}

.aw-empty-hint {
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 13px;
}
</style>
