<script setup lang="ts">
// Owner of all Agent connector state. Loads the catalog once on mount,
// then keeps it in sync with the backend after every Connect / Disconnect /
// Sync All call.

import { computed, onMounted, ref } from 'vue'
import {
  loadAgentRegistry,
  loadProjectAgents,
  type AgentRegistryList,
  type ProjectAgentList,
} from '@/data/agents'
import {
  disconnectAgentConnector,
  loadAgentConnectors,
  syncAgentConnector,
  type AgentConnector,
  type AgentConnectorList,
} from '@/data/agentConnectors'
import {
  installAgentTool,
  loadAgentTools,
  type AgentTool,
  type AgentToolList,
} from '@/data/agentTools'
import { BbButton, BbInfoGrid, BbInfoItem } from '@/components/common'
import { t } from '@/i18n'
import AgentConnectorRow from './AgentConnectorRow.vue'

const props = defineProps<{
  project: string
}>()

const list = ref<AgentConnectorList | null>(null)
const toolList = ref<AgentToolList | null>(null)
const registry = ref<AgentRegistryList | null>(null)
const projectRegistry = ref<ProjectAgentList | null>(null)
const offline = ref(false)
const loading = ref(false)
const error = ref<string | null>(null)
const backendError = ref<string | null>(null)
const busyId = ref<string | null>(null)
const toolBusyId = ref<string | null>(null)

onMounted(async () => {
  await refresh()
})

async function refresh() {
  loading.value = true
  error.value = null
  backendError.value = null
  try {
    const result = await loadAgentConnectors()
    list.value = result.data
    offline.value = result.source === 'offline'
    backendError.value = result.source === 'error' ? result.error ?? t('connectorBackendError') : null
    if (result.source === 'rest') {
      const [registryResult, projectResult, toolsResult] = await Promise.all([
        loadAgentRegistry(),
        loadProjectAgents(props.project),
        loadAgentTools(),
      ])
      registry.value = registryResult
      projectRegistry.value = projectResult
      toolList.value = toolsResult
    }
  } catch (err) {
    console.error('refresh agent connectors', err)
    error.value = err instanceof Error ? err.message : String(err)
    offline.value = true
  } finally {
    loading.value = false
  }
}

const sourceMissing = computed(
  () => list.value?.source_state === 'missing',
)

const sourcePath = computed(() => list.value?.source_path ?? '')

const connectors = computed(() => list.value?.connectors ?? [])
const tools = computed(() => toolList.value?.tools ?? [])
const agents = computed(() => registry.value?.agents ?? [])
const assignableAgents = computed(() => projectRegistry.value?.agents ?? [])
const distributedCount = computed(
  () => agents.value.filter((agent) => agent.runtime === 'opencode' && agent.distribute).length,
)

const driftCount = computed(
  () =>
    connectors.value.filter(
      (c) =>
        c.state === 'drift' ||
        c.state === 'missing' ||
        c.state === 'unreachable',
    ).length,
)

const canSyncAll = computed(
  () =>
    !offline.value &&
    !sourceMissing.value &&
    !busyId.value &&
    connectors.value.some((c) => c.state === 'drift' || c.state === 'missing'),
)

function replaceConnector(updated: AgentConnector) {
  if (!list.value) return
  const idx = list.value.connectors.findIndex((c) => c.id === updated.id)
  if (idx >= 0) {
    list.value.connectors.splice(idx, 1, updated)
  }
}

function replaceTool(updated: AgentTool) {
  if (!toolList.value) {
    toolList.value = { tools: [updated] }
    return
  }
  const idx = toolList.value.tools.findIndex((tool) => tool.id === updated.id)
  if (idx >= 0) {
    toolList.value.tools.splice(idx, 1, updated)
  } else {
    toolList.value.tools.push(updated)
  }
}

function toolForConnector(id: string) {
  return tools.value.find((tool) => tool.id === id) ?? null
}

async function handleConnect(id: string) {
  if (busyId.value) return
  busyId.value = id
  error.value = null
  try {
    const updated = await syncAgentConnector(id)
    replaceConnector(updated)
  } catch (err) {
    console.error('sync connector', id, err)
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busyId.value = null
  }
}

async function handleDisconnect(id: string) {
  if (busyId.value) return
  busyId.value = id
  error.value = null
  try {
    const updated = await disconnectAgentConnector(id)
    replaceConnector(updated)
  } catch (err) {
    console.error('disconnect connector', id, err)
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busyId.value = null
  }
}

async function handleSyncAll() {
  if (!canSyncAll.value || !list.value) return
  // Sync only entries that need it; skip 'unreachable' because creating the
  // parent dir there would be surprising (it usually means the tool is not
  // installed).
  const targets = list.value.connectors
    .filter((c) => c.state === 'drift' || c.state === 'missing')
    .map((c) => c.id)
  for (const id of targets) {
    busyId.value = id
    try {
      const updated = await syncAgentConnector(id)
      replaceConnector(updated)
    } catch (err) {
      console.error('sync connector during sync-all', id, err)
      error.value = err instanceof Error ? err.message : String(err)
      break
    }
  }
  busyId.value = null
}

async function handleInstallTool(id: string) {
  if (busyId.value || toolBusyId.value) return
  toolBusyId.value = id
  error.value = null
  try {
    const result = await installAgentTool(id)
    replaceTool(result.tool)
    if (result.exit_code !== undefined && result.exit_code !== 0) {
      const detail = result.stderr_tail || result.stdout_tail || t('agentToolInstallFailed')
      error.value = `${result.command}: ${detail}`
    }
    const connectorsResult = await loadAgentConnectors()
    if (connectorsResult.data) {
      list.value = connectorsResult.data
      backendError.value =
        connectorsResult.source === 'error' ? connectorsResult.error ?? t('connectorBackendError') : null
    }
  } catch (err) {
    console.error('install agent tool', id, err)
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    toolBusyId.value = null
  }
}
</script>

<template>
  <section class="agent-panel">
    <header class="agent-panel-header">
      <div>
        <h2>{{ t('agentConnectors') }}</h2>
        <p class="agent-panel-subtitle">{{ t('agentConnectorSubtitle') }}</p>
      </div>
      <BbButton
        variant="primary"
        :disabled="!canSyncAll"
        @click="handleSyncAll"
      >
        {{ t('connectorSyncAll') }}
        <span v-if="driftCount > 0" class="badge">{{ driftCount }}</span>
      </BbButton>
    </header>

    <p v-if="loading" class="banner banner-info">{{ t('connectorLoading') }}</p>

    <p v-else-if="offline" class="banner banner-warn">
      {{ t('connectorOffline') }}
    </p>

    <p v-else-if="backendError" class="banner banner-error">
      {{ t('connectorBackendResponded') }} {{ backendError }}
    </p>

    <p v-else-if="sourceMissing" class="banner banner-error">
      {{ t('connectorSourceMissing') }}
    </p>

    <p v-if="error" class="banner banner-error">{{ error }}</p>

    <section v-if="registry" class="agent-registry-card">
      <div>
        <h3>{{ t('agentRegistry') }}</h3>
        <p>{{ t('agentRegistryDescription') }}</p>
      </div>
      <BbInfoGrid class="agent-registry-facts" columns="repeat(3, minmax(96px, 1fr))">
        <BbInfoItem :label="t('connectorAgents')" :value="agents.length" variant="metric" />
        <BbInfoItem
          :label="`${project} ${t('connectorAssignable')}`"
          :value="assignableAgents.length"
          variant="metric"
        />
        <BbInfoItem :label="t('connectorOpenCodeDistributed')" :value="distributedCount" variant="metric" />
      </BbInfoGrid>
    </section>

    <div v-if="!offline && !backendError && connectors.length > 0" class="connector-list">
      <AgentConnectorRow
        v-for="connector in connectors"
        :key="connector.id"
        :connector="connector"
        :busy-id="busyId"
        :source-missing="sourceMissing"
        :agents="agents"
        :registry-source-path="registry?.source_path ?? ''"
        :tool="toolForConnector(connector.id)"
        :tool-busy-id="toolBusyId"
        @connect="handleConnect"
        @disconnect="handleDisconnect"
        @install-tool="handleInstallTool"
      />
    </div>
  </section>
</template>

<style scoped>
.agent-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 24px;
  border-radius: 12px;
  background: var(--bb-surface-soft);
  border: 1px solid var(--bb-hairline);
  overflow: hidden;
}

.agent-panel-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.agent-panel-header h2 {
  margin: 0 0 4px 0;
  font-size: 18px;
  color: var(--bb-text-strong);
}

.agent-panel-subtitle {
  margin: 0;
  font-size: 13px;
  color: var(--bb-text-muted);
  max-width: 60ch;
}

.agent-panel-subtitle code {
  font-family: var(--bb-font-mono);
  font-size: 12px;
  overflow-wrap: anywhere;
}

.badge {
  background: var(--bb-project-blackboard-fg);
  color: var(--bb-project-blackboard-bg);
  font-size: 11px;
  padding: 2px 6px;
  border-radius: 999px;
  font-weight: 600;
}

.banner {
  padding: 10px 14px;
  border-radius: 6px;
  font-size: 13px;
  margin: 0;
}

.banner code {
  font-family: var(--bb-font-mono);
}

.banner-info {
  background: var(--bb-accent-soft);
  color: var(--bb-focus);
}

.banner-warn {
  background: color-mix(in srgb, var(--bb-warning) 12%, var(--bb-surface));
  color: var(--bb-warning);
  border: 1px solid color-mix(in srgb, var(--bb-warning) 30%, var(--bb-hairline));
}

.banner-error {
  background: var(--bb-md-error-bg);
  color: var(--bb-error);
  border: 1px solid var(--bb-md-error-border);
}

.connector-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.agent-registry-card {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 16px;
  padding: 14px 16px;
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  background: var(--bb-surface);
}

.agent-registry-card h3 {
  margin: 0 0 4px;
  font-size: 15px;
  color: var(--bb-text-strong);
}

.agent-registry-card p {
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 13px;
  overflow-wrap: anywhere;
}

.agent-registry-facts {
  --bb-info-grid-gap: 10px;
  --bb-info-item-padding: 8px 10px;
  --bb-info-item-border: 1px solid var(--bb-hairline);
}

:global(:root[data-theme='dark']) .agent-registry-facts {
  --bb-info-item-bg: var(--bb-surface-muted);
}

@media (max-width: 860px) {
  .agent-registry-card,
  .agent-panel-header {
    grid-template-columns: 1fr;
    flex-direction: column;
  }

  .agent-registry-facts {
    grid-template-columns: repeat(auto-fit, minmax(112px, 1fr));
    width: 100%;
  }
}
</style>
