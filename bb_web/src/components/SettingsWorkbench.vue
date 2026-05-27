<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { Download, Pencil, RefreshCw, RotateCcw, Save, X } from 'lucide-vue-next'
import {
  BbButton,
  BbInfoGrid,
  BbInfoItem,
  BbInlineAlert,
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
  type PiRuntimeConnector,
  type PiShellPathSource,
  type RuntimeConnector,
  type RuntimeConnectorList,
  type RuntimeConnectorStatus,
} from '@/data/runtimeConnectors'
import {
  loadAgentConnectors,
  syncAgentConnector,
  type AgentConnector,
  type AgentConnectorList,
  type AgentConnectorState,
  type AgentConnectorTarget,
} from '@/data/agentConnectors'
import {
  installAgentTool,
  loadAgentTools,
  type AgentTool,
  type AgentToolList,
  type AgentToolStatus,
} from '@/data/agentTools'

defineProps<{
  project: string
}>()

const loading = ref(true)
const error = ref('')
const savedMessage = ref('')
const registry = ref<AgentRegistryList | null>(null)
const runtimeConnectors = ref<RuntimeConnectorList | null>(null)
const agentConnectorList = ref<AgentConnectorList | null>(null)
const toolList = ref<AgentToolList | null>(null)
const connectorSource = ref<'rest' | 'offline' | 'error'>('rest')
const connectorBackendError = ref('')

const commandDrafts = reactive<Record<string, string>>({})
const concurrencyDrafts = reactive<Record<string, number>>({})
const editingRuntimeIds = reactive<Record<string, boolean>>({})
const savingRuntimeIds = reactive<Record<string, boolean>>({})
const runtimeSaveMessages = reactive<Record<string, string>>({})
const busyConnectorId = ref<string | null>(null)
const toolBusyId = ref<string | null>(null)

const runtimes = computed(() => registry.value?.runtimes ?? [])
const connectors = computed(() => agentConnectorList.value?.connectors ?? [])
const tools = computed(() => toolList.value?.tools ?? [])
const sourceMissing = computed(() => agentConnectorList.value?.source_state === 'missing')

const connectedRuntimeCount = computed(
  () => runtimeConnectors.value?.runtimes.filter((runtime) => runtime.status === 'connected').length ?? 0,
)
const syncedConnectorCount = computed(
  () => connectors.value.filter((connector) => connector.state === 'synced').length,
)
const healthyToolCount = computed(
  () => tools.value.filter(isToolHealthy).length,
)
const connectorSyncTargets = computed(() =>
  connectors.value.filter((connector) => connector.state === 'drift' || connector.state === 'missing'),
)
const toolUpdateTargets = computed(() =>
  tools.value.filter((tool) => canInstallTool(tool) && !isToolHealthy(tool)),
)
const canSyncAll = computed(
  () => !sourceMissing.value && !busyConnectorId.value && connectorSyncTargets.value.length > 0,
)
const canUpdateTools = computed(() => !toolBusyId.value && toolUpdateTargets.value.length > 0)

onMounted(() => {
  void reload()
})

async function reload() {
  loading.value = true
  error.value = ''
  savedMessage.value = ''
  connectorBackendError.value = ''
  try {
    const [reg, runtimeList, connectorResult, toolsResult] = await Promise.all([
      loadAgentRegistry(),
      loadRuntimeConnectors(),
      loadAgentConnectors(),
      loadAgentTools(),
    ])
    registry.value = reg
    applyRuntimeList(runtimeList)
    connectorSource.value = connectorResult.source
    connectorBackendError.value = connectorResult.source === 'error' ? connectorResult.error ?? '' : ''
    agentConnectorList.value = connectorResult.data
    toolList.value = toolsResult
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

function applyRuntimeList(next: RuntimeConnectorList) {
  runtimeConnectors.value = next
  for (const runtime of next.runtimes) {
    commandDrafts[runtime.id] = runtime.command
    concurrencyDrafts[runtime.id] = runtime.max_concurrency ?? 0
  }
}

function runtimeConnector(id: string): RuntimeConnector | null {
  return runtimeConnectors.value?.runtimes.find((runtime) => runtime.id === id) ?? null
}

function agentConnector(id: string): AgentConnector | null {
  return connectors.value.find((connector) => connector.id === id) ?? null
}

function toolForRuntime(id: string): AgentTool | null {
  return tools.value.find((tool) => tool.id === id) ?? null
}

function primaryTargets(connector: AgentConnector | null): AgentConnectorTarget[] {
  if (!connector) return []
  if (connector.targets?.length) return connector.targets
  return [
    {
      label: t('connectorTarget'),
      target_template: connector.target_template,
      target_path: connector.target_path,
      connector_type: connector.connector_type,
      state: connector.state,
      target_sha256_short: connector.target_sha256_short,
      target_mtime: connector.target_mtime,
      is_symlink: connector.is_symlink,
    },
  ]
}

function registrationTargets(connector: AgentConnector | null): AgentConnectorTarget[] {
  return primaryTargets(connector).filter((target) => !target.label.startsWith('Agent: '))
}

function targetLabel(label: string) {
  if (label.startsWith('Agent: ')) return t('agentConnectorAgentPrefix') + label.slice(7)
  return label
}

function normalizedConcurrency(value: number | undefined) {
  const concurrency = Number(value ?? 0)
  return Number.isFinite(concurrency) ? Math.max(0, Math.floor(concurrency)) : 0
}

function resetRuntimeDraft(id: string) {
  const runtime = runtimeConnector(id)
  commandDrafts[id] = runtime?.command ?? ''
  concurrencyDrafts[id] = runtime?.max_concurrency ?? 0
}

function enterRuntimeEdit(id: string) {
  resetRuntimeDraft(id)
  runtimeSaveMessages[id] = ''
  editingRuntimeIds[id] = true
}

function cancelRuntimeEdit(id: string) {
  resetRuntimeDraft(id)
  editingRuntimeIds[id] = false
}

function runtimeConfigDirty(id: string) {
  const runtime = runtimeConnector(id)
  if (!runtime) return false
  return (
    (commandDrafts[id] ?? '') !== runtime.command ||
    normalizedConcurrency(concurrencyDrafts[id]) !== (runtime.max_concurrency ?? 0)
  )
}

async function saveRuntimeConfig(id: string) {
  if (savingRuntimeIds[id]) return
  savingRuntimeIds[id] = true
  runtimeSaveMessages[id] = ''
  error.value = ''
  try {
    const next = await patchRuntimeConnectors({
      commands: { [id]: commandDrafts[id] ?? '' },
      runtime_max_concurrency: {
        [id]: normalizedConcurrency(concurrencyDrafts[id]),
      },
    })
    applyRuntimeList(next)
    editingRuntimeIds[id] = false
    runtimeSaveMessages[id] = t('runtimeConnectorSaved')
    setTimeout(() => {
      runtimeSaveMessages[id] = ''
    }, 2200)
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    savingRuntimeIds[id] = false
  }
}

function replaceConnector(updated: AgentConnector) {
  if (!agentConnectorList.value) return
  const idx = agentConnectorList.value.connectors.findIndex((connector) => connector.id === updated.id)
  if (idx >= 0) agentConnectorList.value.connectors.splice(idx, 1, updated)
}

async function handleSyncConnector(id: string) {
  if (busyConnectorId.value || sourceMissing.value) return
  busyConnectorId.value = id
  error.value = ''
  try {
    replaceConnector(await syncAgentConnector(id))
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busyConnectorId.value = null
  }
}

async function handleSyncAll() {
  if (!canSyncAll.value) return
  savedMessage.value = ''
  for (const connector of connectorSyncTargets.value) {
    busyConnectorId.value = connector.id
    try {
      replaceConnector(await syncAgentConnector(connector.id))
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err)
      break
    }
  }
  busyConnectorId.value = null
  if (!error.value) savedMessage.value = t('runtimeSyncAllDone')
}

function replaceTool(updated: AgentTool) {
  if (!toolList.value) {
    toolList.value = { tools: [updated] }
    return
  }
  const idx = toolList.value.tools.findIndex((tool) => tool.id === updated.id)
  if (idx >= 0) toolList.value.tools.splice(idx, 1, updated)
  else toolList.value.tools.push(updated)
}

async function handleInstallTool(id: string) {
  if (toolBusyId.value) return
  toolBusyId.value = id
  error.value = ''
  try {
    const result = await installAgentTool(id)
    replaceTool(result.tool)
    if (result.exit_code !== undefined && result.exit_code !== 0) {
      const detail = result.stderr_tail || result.stdout_tail || t('agentToolInstallFailed')
      error.value = `${result.command}: ${detail}`
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    toolBusyId.value = null
  }
}

async function handleUpdateTools() {
  if (!canUpdateTools.value) return
  savedMessage.value = ''
  for (const tool of toolUpdateTargets.value) {
    await handleInstallTool(tool.id)
    if (error.value) break
  }
  if (!error.value) savedMessage.value = t('runtimeUpdateAllDone')
}

function runtimeStatusLabel(status?: RuntimeConnectorStatus) {
  if (status === 'connected') return t('runtimeStatusConnected')
  if (status === 'missing') return t('runtimeStatusMissing')
  if (status === 'check_failed') return t('runtimeStatusError')
  return t('connectorUnknown')
}

function runtimeTone(status?: RuntimeConnectorStatus) {
  if (status === 'connected') return 'green'
  if (status === 'missing') return 'red'
  if (status === 'check_failed') return 'yellow'
  return 'grey'
}

function connectorStateLabel(state?: AgentConnectorState) {
  if (state === 'synced') return t('connectorSynced')
  if (state === 'drift') return t('connectorDrift')
  if (state === 'missing') return t('connectorMissing')
  if (state === 'unreachable') return t('connectorUnreachable')
  if (state === 'source_missing') return t('connectorSourceMissingLabel')
  return t('connectorUnknown')
}

function connectorTone(state?: AgentConnectorState) {
  if (state === 'synced') return 'green'
  if (state === 'drift') return 'yellow'
  if (state === 'missing') return 'red'
  return 'grey'
}

function connectorActionLabel(connector: AgentConnector | null) {
  if (connector && busyConnectorId.value === connector.id) return t('connectorBusy')
  if (connector?.state === 'synced') return t('connectorResync')
  return t('runtimeSyncRuntime')
}

function toolStatusLabel(status?: AgentToolStatus) {
  if (status === 'missing') return t('agentToolMissing')
  if (status === 'installed') return t('agentToolInstalled')
  if (status === 'incomplete') return t('agentToolIncomplete')
  if (status === 'version_mismatch') return t('agentToolVersionMismatch')
  if (status === 'npm_missing') return t('agentToolNpmMissing')
  if (status === 'check_failed') return t('agentToolCheckFailed')
  if (status === 'external_install') return t('agentToolExternalInstall')
  return t('connectorUnknown')
}

function toolTone(status?: AgentToolStatus) {
  if (status === 'installed') return 'green'
  if (status === 'missing' || status === 'incomplete' || status === 'version_mismatch' || status === 'external_install') {
    return 'yellow'
  }
  if (status === 'npm_missing' || status === 'check_failed') return 'red'
  return 'grey'
}

const lockedToolIds = new Set(['opencode', 'pi'])

function hasPinnedTargetMismatch(tool: AgentTool | null) {
  if (!tool || tool.target_version === 'latest' || !tool.current_version) return false
  return tool.current_version !== tool.target_version
}

function compareVersionStrings(left?: string, right?: string) {
  const leftParts = left?.match(/\d+/g)?.map(Number) ?? []
  const rightParts = right?.match(/\d+/g)?.map(Number) ?? []
  const length = Math.max(leftParts.length, rightParts.length)
  for (let index = 0; index < length; index += 1) {
    const diff = (leftParts[index] ?? 0) - (rightParts[index] ?? 0)
    if (diff !== 0) return diff > 0 ? 1 : -1
  }
  return 0
}

function versionLockActionLabel(tool: AgentTool) {
  const direction = compareVersionStrings(tool.current_version, tool.target_version)
  if (direction > 0) return t('agentToolDowngradeToLocked')
  if (direction < 0) return t('agentToolUpgradeToLocked')
  return t('agentToolApplyVersionLock')
}

function isToolHealthy(tool: AgentTool) {
  return tool.status === 'installed' && !hasPinnedTargetMismatch(tool)
}

function versionLockLabel(tool: AgentTool | null) {
  if (!tool) return t('connectorUnknown')
  if (tool.target_version === 'latest') return t('runtimeVersionFollowsLatest')
  if (hasPinnedTargetMismatch(tool)) return t('runtimeVersionNeedsTarget', { version: tool.target_version })
  if (tool.current_version === tool.target_version) return t('runtimeVersionLocked')
  return toolStatusLabel(tool.status)
}

function versionLockTone(tool: AgentTool | null) {
  if (!tool) return 'grey'
  if (tool.target_version === 'latest') return toolTone(tool.status)
  if (hasPinnedTargetMismatch(tool)) return 'yellow'
  if (tool.current_version === tool.target_version) return 'green'
  return toolTone(tool.status)
}

function installSourceLabel(tool: AgentTool | null) {
  if (tool?.install_source === 'npm') return t('runtimeInstallMethodNpm')
  if (tool?.install_source === 'brew') return t('runtimeInstallMethodBrew')
  if (tool?.install_source === 'external') return t('runtimeInstallMethodExternal')
  return t('runtimeInstallMethodUnknown')
}

function installSourceTone(tool: AgentTool | null) {
  if (tool?.install_source === 'npm') return 'green'
  if (tool?.install_source === 'brew') return 'yellow'
  if (tool?.install_source === 'external') return 'grey'
  return 'grey'
}

function canInstallTool(tool: AgentTool | null) {
  if (!tool) return false
  if (tool.status === 'npm_missing') return false
  if (hasPinnedTargetMismatch(tool) && lockedToolIds.has(tool.id)) return true
  if (tool.status === 'installed') return false
  return true
}

function shouldShowToolAction(tool: AgentTool | null) {
  return tool !== null
}

function toolActionLabel(tool: AgentTool | null) {
  if (!tool) return t('agentToolInstall')
  if (toolBusyId.value === tool.id) return t('connectorBusy')
  if (hasPinnedTargetMismatch(tool) && lockedToolIds.has(tool.id)) return versionLockActionLabel(tool)
  if (tool.status === 'version_mismatch' && lockedToolIds.has(tool.id)) return versionLockActionLabel(tool)
  if (tool.status === 'installed') return t('agentToolUpdate')
  if (tool.install_source === 'brew') return t('agentToolUpdate')
  if (tool.status === 'incomplete' || tool.status === 'external_install') return t('agentToolRepair')
  return t('agentToolInstall')
}

function toolActionTitle(tool: AgentTool | null) {
  if (!tool || !hasPinnedTargetMismatch(tool) || !lockedToolIds.has(tool.id)) return undefined
  if (tool.install_source === 'brew') return t('runtimePinnedNpmLockHint')
  return t('runtimePinnedMismatch')
}

function shellSourceLabel(source: PiShellPathSource) {
  if (source === 'default') return t('runtimeShellDefault')
  if (source === 'settings') return t('runtimeShellSettingsJson')
  if (source === 'git_bash_default') return t('runtimeShellGitBashDefault')
  if (source === 'path') return t('runtimeShellPath')
  return t('runtimeShellNotFound')
}

function hasPiModelConfig(pi: PiRuntimeConnector) {
  return Boolean(pi.default_provider || pi.default_model)
}

function hasPiSettingsDetails(pi: PiRuntimeConnector) {
  return pi.shell_resolution_required || hasPiModelConfig(pi) || Boolean(pi.settings_error)
}
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
          <BbButton size="lg" variant="secondary" :disabled="loading" @click="reload">
            <template #leading><RefreshCw /></template>
            {{ t('refresh') }}
          </BbButton>
          <BbButton size="lg" variant="secondary" :disabled="!canUpdateTools" @click="handleUpdateTools">
            <template #leading><Download /></template>
            {{ t('runtimeUpdateTools') }}
          </BbButton>
          <BbButton size="lg" variant="primary" :disabled="!canSyncAll" @click="handleSyncAll">
            <template #leading><RotateCcw /></template>
            {{ t('runtimeSyncAll') }}
          </BbButton>
        </template>
      </BbToolbar>
    </header>

    <div v-if="loading" class="bb-state-panel">{{ t('connectorLoading') }}</div>
    <div v-else-if="error" class="bb-state-panel bb-error">{{ error }}</div>

    <div v-else class="runtime-workspace">
      <BbInlineAlert v-if="savedMessage" tone="success">{{ savedMessage }}</BbInlineAlert>
      <BbInlineAlert v-if="connectorSource === 'offline'" tone="warning">
        {{ t('connectorOffline') }}
      </BbInlineAlert>
      <BbInlineAlert v-else-if="connectorBackendError" tone="error">
        {{ t('connectorBackendResponded') }} {{ connectorBackendError }}
      </BbInlineAlert>
      <BbInlineAlert v-else-if="sourceMissing" tone="error">
        {{ t('connectorSourceMissing') }}
      </BbInlineAlert>

      <section class="runtime-summary" :aria-label="t('runtimeConfigOverview')">
        <article>
          <span>{{ t('runtimeAvailable') }}</span>
          <strong>{{ connectedRuntimeCount }} / {{ runtimes.length }}</strong>
        </article>
        <article>
          <span>{{ t('runtimeRuleSync') }}</span>
          <strong>{{ syncedConnectorCount }} / {{ connectors.length }}</strong>
        </article>
        <article>
          <span>{{ t('runtimeTooling') }}</span>
          <strong>{{ healthyToolCount }} / {{ tools.length }}</strong>
        </article>
        <article>
          <span>{{ t('runtimeConfigFile') }}</span>
          <strong class="runtime-path">{{ runtimeConnectors?.config_path || '-' }}</strong>
        </article>
      </section>

      <div class="runtime-list">
        <article
          v-for="runtime in runtimes"
          :key="runtime.id"
          class="runtime-card"
          :data-tone="runtimeTone(runtimeConnector(runtime.id)?.status)"
        >
          <header class="runtime-card-head">
            <span
              class="runtime-state-light"
              :data-tone="runtimeTone(runtimeConnector(runtime.id)?.status)"
              aria-hidden="true"
            />
            <div class="runtime-title">
              <div class="runtime-title-line">
                <h3>{{ runtime.display_name }}</h3>
              </div>
            </div>
            <div class="runtime-card-actions">
              <span class="runtime-status-pill" :data-tone="runtimeTone(runtimeConnector(runtime.id)?.status)">
                {{ runtimeStatusLabel(runtimeConnector(runtime.id)?.status) }}
              </span>
              <BbButton
                v-if="shouldShowToolAction(toolForRuntime(runtime.id))"
                size="sm"
                variant="secondary"
                :disabled="!canInstallTool(toolForRuntime(runtime.id)) || toolBusyId !== null || busyConnectorId !== null"
                :title="toolActionTitle(toolForRuntime(runtime.id))"
                @click="handleInstallTool(runtime.id)"
              >
                <template #leading><Download /></template>
                {{ toolActionLabel(toolForRuntime(runtime.id)) }}
              </BbButton>
              <BbButton
                v-if="agentConnector(runtime.id)"
                size="sm"
                variant="primary"
                :disabled="sourceMissing || busyConnectorId !== null || toolBusyId !== null"
                @click="handleSyncConnector(runtime.id)"
              >
                <template #leading><RotateCcw /></template>
                {{ connectorActionLabel(agentConnector(runtime.id)) }}
              </BbButton>
            </div>
          </header>

          <section class="runtime-facts">
            <BbInfoItem :label="t('runtimeVersion')" :value="runtimeConnector(runtime.id)?.version || '-'" variant="mono" overflow="truncate" />
            <BbInfoItem :label="t('runtimeRuleSync')" variant="mono" overflow="truncate">
              <span class="inline-state" :data-tone="connectorTone(agentConnector(runtime.id)?.state)" />
              {{ connectorStateLabel(agentConnector(runtime.id)?.state) }}
            </BbInfoItem>
            <BbInfoItem :label="t('runtimeVersionLock')" variant="mono" overflow="truncate">
              <span class="inline-state" :data-tone="versionLockTone(toolForRuntime(runtime.id))" />
              {{ versionLockLabel(toolForRuntime(runtime.id)) }}
            </BbInfoItem>
            <BbInfoItem :label="t('runtimeInstallMethod')" variant="mono" overflow="truncate">
              <span class="inline-state" :data-tone="installSourceTone(toolForRuntime(runtime.id))" />
              {{ installSourceLabel(toolForRuntime(runtime.id)) }}
            </BbInfoItem>
            <BbInfoItem :label="t('runtimeConcurrency')" :value="runtimeConnector(runtime.id)?.max_concurrency ?? 0" variant="mono" />
          </section>

          <div class="runtime-detail-grid">
            <section class="runtime-block runtime-block-command">
              <div class="runtime-block-head">
                <div class="runtime-block-title">
                  <h4>{{ t('runtimeCommandConfig') }}</h4>
                  <span class="runtime-status-pill" :data-tone="editingRuntimeIds[runtime.id] ? 'yellow' : 'grey'">
                    {{ editingRuntimeIds[runtime.id] ? t('runtimeEditMode') : t('runtimeViewMode') }}
                  </span>
                </div>
                <div class="runtime-edit-actions">
                  <BbButton
                    v-if="editingRuntimeIds[runtime.id]"
                    size="sm"
                    variant="secondary"
                    :disabled="savingRuntimeIds[runtime.id] || !runtimeConfigDirty(runtime.id)"
                    @click="saveRuntimeConfig(runtime.id)"
                  >
                    <template #leading><Save /></template>
                    {{ savingRuntimeIds[runtime.id] ? t('runtimeSaving') : t('runtimeSaveRuntime') }}
                  </BbButton>
                  <BbButton
                    v-if="editingRuntimeIds[runtime.id]"
                    size="sm"
                    variant="ghost"
                    :disabled="savingRuntimeIds[runtime.id]"
                    @click="cancelRuntimeEdit(runtime.id)"
                  >
                    <template #leading><X /></template>
                    {{ t('cancel') }}
                  </BbButton>
                  <BbButton
                    v-else
                    size="sm"
                    variant="secondary"
                    :disabled="savingRuntimeIds[runtime.id]"
                    @click="enterRuntimeEdit(runtime.id)"
                  >
                    <template #leading><Pencil /></template>
                    {{ t('edit') }}
                  </BbButton>
                </div>
              </div>
              <div v-if="editingRuntimeIds[runtime.id]" class="runtime-form">
                <label class="runtime-field">
                  <span>{{ t('runtimeCommand') }}</span>
                  <input v-model="commandDrafts[runtime.id]" spellcheck="false" />
                </label>
                <label class="runtime-field runtime-field-small">
                  <span>{{ t('runtimeMaxConcurrency') }}</span>
                  <input v-model.number="concurrencyDrafts[runtime.id]" type="number" min="0" step="1" />
                </label>
              </div>
              <BbInfoGrid v-else class="runtime-view-grid" variant="rows">
                <BbInfoItem :label="t('runtimeCommand')" :value="runtimeConnector(runtime.id)?.command || '-'" variant="mono" overflow="truncate" />
                <BbInfoItem :label="t('runtimeMaxConcurrency')" :value="runtimeConnector(runtime.id)?.max_concurrency ?? 0" variant="mono" />
              </BbInfoGrid>
              <BbInlineAlert v-if="runtimeSaveMessages[runtime.id]" tone="success">
                {{ runtimeSaveMessages[runtime.id] }}
              </BbInlineAlert>
              <BbInlineAlert v-if="runtimeConnector(runtime.id)?.error" tone="warning">
                {{ runtimeConnector(runtime.id)?.error }}
              </BbInlineAlert>
            </section>

            <section class="runtime-block">
              <div class="runtime-block-head">
                <h4>{{ t('runtimeRuleSync') }}</h4>
                <span class="runtime-status-pill" :data-tone="connectorTone(agentConnector(runtime.id)?.state)">
                  {{ connectorStateLabel(agentConnector(runtime.id)?.state) }}
                </span>
              </div>
              <template v-if="agentConnector(runtime.id)">
                <div class="runtime-target-list">
                  <div
                    v-for="target in registrationTargets(agentConnector(runtime.id))"
                    :key="target.target_template"
                    class="runtime-target"
                  >
                    <span class="inline-state" :data-tone="connectorTone(target.state)" />
                    <span>{{ targetLabel(target.label) }}</span>
                    <code>{{ target.target_path }}</code>
                    <small>{{ connectorStateLabel(target.state) }}</small>
                  </div>
                </div>
              </template>
              <p v-else class="runtime-muted">{{ t('runtimeNoConnector') }}</p>
            </section>
          </div>

          <details
            v-if="runtime.id === 'pi' && runtimeConnectors?.pi && hasPiSettingsDetails(runtimeConnectors.pi)"
            class="runtime-pi-details"
          >
            <summary>{{ t('runtimePiSettings') }}</summary>
            <BbInfoGrid class="runtime-meta" columns="repeat(2, minmax(0, 1fr))">
              <template v-if="runtimeConnectors.pi.shell_resolution_required">
                <BbInfoItem :label="t('runtimeSource')" :value="shellSourceLabel(runtimeConnectors.pi.shell_path_source)" />
                <BbInfoItem :label="t('runtimeEffectiveShell')" :value="runtimeConnectors.pi.effective_shell_path || '-'" variant="mono" overflow="truncate" />
                <BbInfoItem :label="t('runtimeRecommendedGitBash')" :value="runtimeConnectors.pi.recommended_shell_path || t('runtimeRecommendedGitBashNotFound')" variant="mono" overflow="truncate" />
              </template>
              <BbInfoItem v-else :label="t('runtimeShellMode')" :value="t('runtimeShellDefault')" />
              <BbInfoItem v-if="hasPiModelConfig(runtimeConnectors.pi)" :label="t('runtimeModel')" variant="mono" overflow="truncate">
                {{ runtimeConnectors.pi.default_provider || '-' }} / {{ runtimeConnectors.pi.default_model || '-' }}
              </BbInfoItem>
              <BbInfoItem v-if="runtimeConnectors.pi.settings_error" :label="t('runtimePiSettings')" :value="runtimeConnectors.pi.settings_path || '-'" variant="mono" overflow="truncate" />
              <BbInfoItem v-if="runtimeConnectors.pi.settings_error" :label="t('runtimeConfigError')" :value="runtimeConnectors.pi.settings_error" />
            </BbInfoGrid>
          </details>
        </article>
      </div>
    </div>
  </section>
</template>

<style scoped>
.settings-workbench {
  display: flex;
  flex: 1 1 0;
  min-height: 0;
  flex-direction: column;
  overflow-y: auto;
}

.settings-workbench :deep(.bb-button-icon svg) {
  width: 15px;
  height: 15px;
}

.runtime-workspace {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 14px 20px 40px;
}

.runtime-summary {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
}

.runtime-summary article {
  min-width: 0;
  padding: 10px 12px;
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  background: var(--bb-surface);
}

.runtime-summary span {
  display: block;
  margin-bottom: 4px;
  color: var(--bb-text-muted);
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
}

.runtime-summary strong {
  display: block;
  min-width: 0;
  overflow: hidden;
  color: var(--bb-text-strong);
  font-size: 17px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.runtime-summary .runtime-path {
  font-family: var(--bb-font-mono);
  font-size: 12px;
}

.runtime-list {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.runtime-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 14px 16px;
  border: 1px solid var(--bb-hairline);
  border-left-width: 3px;
  border-radius: 8px;
  background: var(--bb-surface);
}

.runtime-card[data-tone='green'] {
  border-left-color: var(--bb-success);
}

.runtime-card[data-tone='yellow'] {
  border-left-color: var(--bb-warning);
}

.runtime-card[data-tone='red'] {
  border-left-color: var(--bb-error);
}

.runtime-card[data-tone='grey'] {
  border-left-color: var(--bb-text-muted);
}

.runtime-card-head {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr) auto;
  gap: 10px;
  align-items: flex-start;
}

.runtime-state-light,
.inline-state {
  display: inline-block;
  flex: 0 0 auto;
  border-radius: 999px;
  box-shadow: 0 0 0 1px var(--bb-hairline);
}

.runtime-state-light {
  width: 12px;
  height: 12px;
  margin-top: 6px;
}

.inline-state {
  width: 8px;
  height: 8px;
  margin-right: 6px;
}

.runtime-state-light[data-tone='green'],
.inline-state[data-tone='green'] {
  background: var(--bb-success);
  box-shadow: 0 0 6px color-mix(in srgb, var(--bb-success) 45%, transparent);
}

.runtime-state-light[data-tone='yellow'],
.inline-state[data-tone='yellow'] {
  background: var(--bb-warning);
  box-shadow: 0 0 7px color-mix(in srgb, var(--bb-warning) 45%, transparent);
}

.runtime-state-light[data-tone='red'],
.inline-state[data-tone='red'] {
  background: var(--bb-error);
  box-shadow: 0 0 7px color-mix(in srgb, var(--bb-error) 40%, transparent);
}

.runtime-state-light[data-tone='grey'],
.inline-state[data-tone='grey'] {
  background: var(--bb-text-muted);
}

.runtime-title {
  min-width: 0;
}

.runtime-title-line,
.runtime-block-head {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
}

.runtime-title h3,
.runtime-block h4 {
  margin: 0;
  color: var(--bb-text-strong);
}

.runtime-title h3 {
  font-size: 16px;
}

.runtime-block h4 {
  font-size: 13px;
}

.runtime-target code {
  color: var(--bb-text-muted);
  font-family: var(--bb-font-mono);
  font-size: 12px;
}

.runtime-muted {
  margin: 4px 0 0;
  color: var(--bb-text-muted);
  font-size: 12px;
  line-height: 1.5;
}

.runtime-card-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
}

.runtime-status-pill,
.runtime-count,
.runtime-component {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 700;
  white-space: nowrap;
}

.runtime-status-pill[data-tone='green'],
.runtime-component[data-tone='green'] {
  color: var(--bb-success);
  background: color-mix(in srgb, var(--bb-success) 10%, var(--bb-surface));
}

.runtime-status-pill[data-tone='yellow'],
.runtime-component[data-tone='yellow'] {
  color: var(--bb-warning);
  background: color-mix(in srgb, var(--bb-warning) 12%, var(--bb-surface));
}

.runtime-status-pill[data-tone='red'],
.runtime-component[data-tone='red'] {
  color: var(--bb-error);
  background: color-mix(in srgb, var(--bb-error) 10%, var(--bb-surface));
}

.runtime-status-pill[data-tone='grey'],
.runtime-component[data-tone='grey'],
.runtime-count {
  color: var(--bb-text-muted);
  background: var(--bb-surface-muted);
}

.runtime-facts {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 8px;
}

.runtime-detail-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 14px 18px;
}

.runtime-block {
  min-width: 0;
  padding-top: 12px;
  border-top: 1px solid var(--bb-hairline);
}

.runtime-block-head {
  justify-content: space-between;
  margin-bottom: 10px;
}

.runtime-block-title,
.runtime-edit-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
  min-width: 0;
}

.runtime-block-title {
  flex: 1 1 auto;
}

.runtime-edit-actions {
  justify-content: flex-end;
}

.runtime-pill-group {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 6px;
}

.runtime-view-grid {
  margin-bottom: 10px;
}

.runtime-form {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(96px, 140px);
  gap: 10px;
  margin-bottom: 10px;
}

.runtime-field {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 4px;
}

.runtime-field span {
  color: var(--bb-text-muted);
  font-size: 11px;
  font-weight: 700;
}

.runtime-field input {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  padding: 7px 8px;
  border: 1px solid var(--bb-hairline);
  border-radius: 6px;
  background: var(--bb-surface);
  color: var(--bb-text-strong);
  font: 12px/1.4 var(--bb-font-mono);
}

.runtime-field input:focus {
  border-color: var(--bb-hairline-strong);
  outline: 2px solid color-mix(in srgb, var(--bb-focus) 20%, transparent);
}

.runtime-meta {
  margin-top: 8px;
}

.runtime-target-list,
.runtime-component-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.runtime-target {
  display: grid;
  grid-template-columns: 12px minmax(70px, max-content) minmax(0, 1fr) auto;
  gap: 8px;
  align-items: center;
  min-width: 0;
  color: var(--bb-text-muted);
  font-size: 12px;
}

.runtime-target code {
  min-width: 0;
  overflow: hidden;
  color: var(--bb-text);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.runtime-target small {
  color: var(--bb-text-muted);
  font-size: 11px;
  white-space: nowrap;
}

.runtime-component-list {
  flex-direction: row;
  flex-wrap: wrap;
  margin-top: 8px;
}

.runtime-pi-details {
  padding-top: 10px;
  border-top: 1px solid var(--bb-hairline);
}

.runtime-pi-details summary {
  cursor: pointer;
  color: var(--bb-text-strong);
  font-size: 13px;
  font-weight: 700;
}

@media (max-width: 980px) {
  .runtime-summary,
  .runtime-facts,
  .runtime-detail-grid {
    grid-template-columns: 1fr;
  }

  .runtime-card-head {
    grid-template-columns: 16px minmax(0, 1fr);
  }

  .runtime-card-actions {
    grid-column: 2;
    justify-content: flex-start;
  }
}

@media (max-width: 640px) {
  .runtime-workspace {
    padding: 12px;
  }

  .runtime-form,
  .runtime-target {
    grid-template-columns: 1fr;
  }

  .runtime-target .inline-state {
    display: none;
  }
}
</style>
