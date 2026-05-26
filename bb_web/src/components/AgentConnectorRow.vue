<script setup lang="ts">
// Per-connector row: status light + path + Connect / Disconnect buttons.
// Stays "dumb"—all state mutations are forwarded to the parent panel via
// emit, so this component never owns connector state itself.

import { computed, ref } from 'vue'
import { ChevronDown } from 'lucide-vue-next'
import { BbInfoGrid, BbInfoItem } from '@/components/common'
import { t } from '@/i18n'
import type { AgentProfile } from '@/data/agents'
import type {
  AgentConnector,
  AgentConnectorState,
  AgentConnectorTarget,
} from '@/data/agentConnectors'
import type { AgentTool, AgentToolStatus } from '@/data/agentTools'

const props = defineProps<{
  connector: AgentConnector
  /** Connector id currently being mutated, or null when idle. */
  busyId: string | null
  /** Whether the source-of-truth itself is missing (disables Connect). */
  sourceMissing: boolean
  agents: AgentProfile[]
  registrySourcePath: string
  tool: AgentTool | null
  toolBusyId: string | null
}>()

const emit = defineEmits<{
  (event: 'connect', id: string): void
  (event: 'disconnect', id: string): void
  (event: 'install-tool', id: string): void
}>()

function stateMetaFor(state: AgentConnectorState) {
  switch (state) {
    case 'synced':
      return { label: t('connectorSynced'), color: '#16a34a', tone: 'green' as const }
    case 'drift':
      return { label: t('connectorDrift'), color: '#eab308', tone: 'yellow' as const }
    case 'missing':
      return { label: t('connectorMissing'), color: '#dc2626', tone: 'red' as const }
    case 'unreachable':
      return { label: t('connectorUnreachable'), color: '#9ca3af', tone: 'grey' as const }
    case 'source_missing':
      return { label: t('connectorSourceMissingLabel'), color: '#9ca3af', tone: 'grey' as const }
    default:
      return { label: t('connectorUnknown'), color: '#9ca3af', tone: 'grey' as const }
  }
}

function targetLabel(label: string) {
  if (label.startsWith('Agent: ')) return t('agentConnectorAgentPrefix') + label.slice(7)
  return label
}

const stateMeta = computed(() => stateMetaFor(props.connector.state))

function toolStatusLabel(status: AgentToolStatus) {
  switch (status) {
    case 'missing':
      return t('agentToolMissing')
    case 'installed':
      return t('agentToolInstalled')
    case 'incomplete':
      return t('agentToolIncomplete')
    case 'version_mismatch':
      return t('agentToolVersionMismatch')
    case 'npm_missing':
      return t('agentToolNpmMissing')
    case 'check_failed':
      return t('agentToolCheckFailed')
    case 'external_install':
      return t('agentToolExternalInstall')
    default:
      return t('connectorUnknown')
  }
}

function toolPrimaryStatusLabel(tool: AgentTool) {
  if (tool.status === 'installed' && tool.install_source === 'brew') return t('agentToolBrewInstall')
  if (tool.status === 'installed' && tool.install_source === 'npm') return t('agentToolNpmInstall')
  return toolStatusLabel(tool.status)
}

function toolTone(status: AgentToolStatus) {
  switch (status) {
    case 'installed':
      return 'green'
    case 'missing':
    case 'incomplete':
    case 'version_mismatch':
    case 'external_install':
      return 'yellow'
    case 'npm_missing':
    case 'check_failed':
      return 'red'
    default:
      return 'grey'
  }
}

const isBusy = computed(() => props.busyId === props.connector.id)
const anyBusy = computed(() => props.busyId !== null)
const isToolBusy = computed(() => props.toolBusyId === props.connector.id)
const anyToolBusy = computed(() => props.toolBusyId !== null)
const connectorTargets = computed<AgentConnectorTarget[]>(() => {
  if (props.connector.targets?.length) return props.connector.targets
  return [
    {
      label: t('connectorTarget'),
      target_template: props.connector.target_template,
      target_path: props.connector.target_path,
      connector_type: props.connector.connector_type,
      state: props.connector.state,
      target_sha256_short: props.connector.target_sha256_short,
      target_mtime: props.connector.target_mtime,
      is_symlink: props.connector.is_symlink,
    },
  ]
})
const selectedAgentId = ref<string | null>(null)

const canConnect = computed(
  () => !isBusy.value && !anyBusy.value && !anyToolBusy.value && !props.sourceMissing,
)
const canDisconnect = computed(() => {
  if (isBusy.value || anyBusy.value || anyToolBusy.value) return false
  return (
    props.connector.state === 'synced' ||
    props.connector.state === 'drift'
  )
})

const connectLabel = computed(() => {
  if (isBusy.value) return t('connectorBusy')
  if (props.connector.state === 'synced') return t('connectorResync')
  return t('connectorConnect')
})

const disconnectLabel = computed(() => (isBusy.value ? t('connectorBusy') : t('connectorDisconnect')))
const lockedToolIds = new Set(['opencode', 'pi'])
const toolActionLabel = computed(() => {
  if (isToolBusy.value) return t('connectorBusy')
  const tool = props.tool
  if (!tool) return t('agentToolInstall')
  if (tool.install_source === 'brew') return t('agentToolUpdate')
  if (tool.status === 'version_mismatch' && lockedToolIds.has(tool.id)) return t('agentToolLockOpenCode')
  if (tool.status === 'installed' && lockedToolIds.has(tool.id)) return t('agentToolLocked')
  if (tool.status === 'incomplete') return t('agentToolRepair')
  if (tool.status === 'installed') return t('agentToolUpdate')
  if (tool.status === 'external_install') return t('agentToolRepair')
  return t('agentToolInstall')
})
const canInstallTool = computed(() => {
  if (!props.tool || isToolBusy.value || anyToolBusy.value || anyBusy.value) return false
  if (props.tool.status === 'npm_missing') return false
  if (props.tool.install_source === 'brew') return true
  if (props.tool.status === 'installed' && lockedToolIds.has(props.tool.id)) return false
  return true
})
const visibleToolComponents = computed(() =>
  props.tool?.components?.filter((component) => component.status !== 'installed') ?? [],
)
const managedAgents = computed(() =>
  props.agents
    .filter((agent) => agent.runtime === props.connector.id)
    .filter((agent) => props.connector.id !== 'opencode' || agent.distribute)
    .sort((a, b) => a.id.localeCompare(b.id)),
)
const registrationTargets = computed(() =>
  connectorTargets.value.filter((target) => !target.label.startsWith('Agent: ')),
)
const rowCountLabel = computed(() => `${managedAgents.value.length}${t('connectorRowAgents')}`)
const registrationCountLabel = computed(() => `${registrationTargets.value.length}${t('connectorRowFiles')}`)

function selectAgent(agent: AgentProfile) {
  selectedAgentId.value = agent.id
}

const selectedAgent = computed(() => {
  if (!managedAgents.value.length) return null
  return (
    managedAgents.value.find((agent) => agent.id === selectedAgentId.value) ??
    managedAgents.value[0]
  )
})

function stateLabelForTarget(target: AgentConnectorTarget) {
  switch (target.state) {
    case 'synced':
      return t('connectorSynced')
    case 'drift':
      return t('connectorDrift')
    case 'missing':
      return t('connectorMissing')
    case 'unreachable':
      return t('connectorUnreachable')
    case 'source_missing':
      return t('connectorSourceMissingLabel')
    default:
      return t('connectorUnknown')
  }
}

function targetForAgent(agent: AgentProfile | null) {
  if (!agent) return null
  if (props.connector.id !== 'opencode') return null
  return (
    connectorTargets.value.find((target) => target.label === `Agent: ${agent.id}`) ??
    connectorTargets.value.find((target) => target.source_path?.endsWith(`/${agent.id}.md`)) ??
    null
  )
}

function sourcePathForAgent(agent: AgentProfile | null) {
  if (!agent) return '-'
  return agent.source_path ?? props.registrySourcePath ?? '-'
}

const selectedAgentTarget = computed(() => targetForAgent(selectedAgent.value))

const tooltip = computed(() => {
  const lines = connectorTargets.value.flatMap((target) => {
    const label = targetLabel(target.label)
    const targetLines = [`${label}：${target.target_path}`]
    if (target.source_path) {
      targetLines.push(`${label} ${t('connectorSource')}: ${target.source_path}`)
    }
    if (target.source_sha256_short) {
      targetLines.push(`${label} ${t('connectorSourceSha256')}: ${target.source_sha256_short}`)
    }
    if (target.target_sha256_short) {
      targetLines.push(`${label} ${t('connectorTargetSha256')}: ${target.target_sha256_short}`)
    }
    if (target.target_mtime) {
      targetLines.push(`${label} ${t('connectorMtime')}: ${target.target_mtime}`)
    }
    if (target.is_symlink) {
      targetLines.push(`${label} ${t('connectorTargetIsSymlink')}`)
    }
    if (target.error) {
      targetLines.push(`${label} ${t('connectorError')}: ${target.error}`)
    }
    return targetLines
  })
  if (props.connector.source_sha256_short) {
    lines.push(`${t('connectorSourceSha256')}: ${props.connector.source_sha256_short}`)
  }
  return lines.join('\n')
})

async function handleCopyPath() {
  try {
    const agent = selectedAgent.value
    const target = selectedAgentTarget.value
    const paths = [sourcePathForAgent(agent), target?.target_path].filter(Boolean)
    await navigator.clipboard.writeText(paths.join('\n'))
  } catch (err) {
    console.warn('clipboard write failed', err)
  }
}

async function handleCopyRegistrationPath(target: AgentConnectorTarget) {
  try {
    await navigator.clipboard.writeText(target.target_path)
  } catch (err) {
    console.warn('clipboard write failed', err)
  }
}

function onConnect() {
  if (!canConnect.value) return
  emit('connect', props.connector.id)
}

function onDisconnect() {
  if (!canDisconnect.value) return
  if (
    !window.confirm(
      `${t('connectorConfirmDelete')}\n${connectorTargets.value
        .map((target) => `${targetLabel(target.label)}: ${target.target_path}`)
        .join('\n')}\n\n${t('connectorDeleteOnlyTargets')}`,
    )
  ) {
    return
  }
  emit('disconnect', props.connector.id)
}

function onInstallTool() {
  if (!canInstallTool.value) return
  emit('install-tool', props.connector.id)
}
</script>

<template>
  <div class="connector-row">
    <div class="connector-summary" :title="tooltip">
      <span
        class="state-light"
        :style="{ background: stateMeta.color }"
        :data-tone="stateMeta.tone"
      />
      <div class="connector-info">
        <div class="connector-headline">
          <span class="connector-name">{{ connector.display_name }}</span>
          <span class="connector-id">{{ connector.id }}</span>
          <span class="state-label">{{ stateMeta.label }}</span>
          <span class="target-count">{{ rowCountLabel }}</span>
          <span class="registration-count">{{ registrationCountLabel }}</span>
          <span v-if="connector.is_symlink" class="symlink-badge" :title="t('connectorTargetIsSymlink')">
            ⤴ {{ t('connectorSymlinkBadge') }}
          </span>
        </div>
        <div class="registration-target-list">
          <button
            v-for="target in registrationTargets"
            :key="target.target_template"
            type="button"
            class="registration-target"
            @click="handleCopyRegistrationPath(target)"
          >
            <span
              class="target-state-dot"
              :style="{ background: stateMetaFor(target.state).color }"
              :data-tone="stateMetaFor(target.state).tone"
              :title="`${targetLabel(target.label)}: ${stateMetaFor(target.state).label}`"
            />
            <span class="target-label">{{ targetLabel(target.label) }}</span>
            <code>{{ target.target_path }}</code>
            <span class="target-state-text">{{ stateMetaFor(target.state).label }}</span>
            <span v-if="target.error" class="target-error">{{ target.error }}</span>
          </button>
        </div>
        <div v-if="tool" class="tool-status" :data-tone="toolTone(tool.status)">
          <div class="tool-status-main">
            <span class="tool-status-label">{{ toolPrimaryStatusLabel(tool) }}</span>
            <span class="tool-version">{{ t('agentToolCurrent') }} {{ tool.current_version ?? '-' }}</span>
            <span class="tool-version">{{ t('agentToolTarget') }} {{ tool.target_version }}</span>
          </div>
          <div v-if="tool.last_error" class="tool-error">{{ tool.last_error }}</div>
          <div v-if="visibleToolComponents.length" class="tool-component-list">
            <div
              v-for="component in visibleToolComponents"
              :key="component.id"
              class="tool-component"
              :data-tone="toolTone(component.status)"
            >
              <span class="component-name">{{ component.display_name }}</span>
              <span class="component-status">{{ toolStatusLabel(component.status) }}</span>
            </div>
          </div>
        </div>
      </div>
      <div class="connector-actions">
        <button
          v-if="tool"
          type="button"
          class="btn btn-ghost"
          :disabled="!canInstallTool"
          @click="onInstallTool"
        >
          {{ toolActionLabel }}
        </button>
        <button
          type="button"
          class="btn btn-primary"
          :disabled="!canConnect"
          @click="onConnect"
        >
          {{ connectLabel }}
        </button>
        <button
          type="button"
          class="btn btn-danger"
          :disabled="!canDisconnect"
          @click="onDisconnect"
        >
          {{ disconnectLabel }}
        </button>
      </div>
    </div>

    <details class="connector-details" open>
      <summary>
        <ChevronDown class="connector-details-chevron" aria-hidden="true" />
        <span>{{ t('connectorAgentManager') }}</span>
        <span>{{ managedAgents.length }} {{ t('connectorRows') }}</span>
      </summary>

      <div class="connector-detail-body">
        <div class="target-rows">
          <button
            v-for="agent in managedAgents"
            :key="agent.id"
            type="button"
            class="target-row-item"
            :class="{ selected: selectedAgent?.id === agent.id }"
            @click="selectAgent(agent)"
          >
            <span class="target-row-main">
              <strong>{{ agent.display_name }}</strong>
              <small>{{ agent.kind }}</small>
            </span>
            <span class="target-row-path">{{ sourcePathForAgent(agent) }}</span>
          </button>
        </div>

        <aside v-if="selectedAgent" class="target-preview">
          <div class="target-preview-head">
            <div>
              <h4>{{ selectedAgent.display_name }}</h4>
              <p>{{ selectedAgent.id }}</p>
            </div>
            <button type="button" class="btn btn-ghost" @click="handleCopyPath">{{ t('connectorCopiedPath') }}</button>
          </div>

          <BbInfoGrid class="target-meta-grid" columns="repeat(2, minmax(0, 1fr))">
            <BbInfoItem :label="t('connectorMetaKind')" :value="selectedAgent.kind" />
            <BbInfoItem :label="t('connectorMetaRuntime')" :value="selectedAgent.runtime ?? connector.id" />
            <BbInfoItem
              :label="t('connectorMetaAssignable')"
              :value="selectedAgent.assignable ? t('connectorMetaYes') : t('connectorMetaNo')"
            />
            <BbInfoItem :label="t('connectorMetaRoles')" :value="selectedAgent.roles?.join(' / ') || '-'" />
            <BbInfoItem
              :label="t('connectorMetaDefinition')"
              :value="selectedAgentTarget ? stateLabelForTarget(selectedAgentTarget) : 'metadata'"
            />
            <BbInfoItem :label="t('connectorMetaStatus')" :value="selectedAgent.status" />
          </BbInfoGrid>

          <div class="target-path-preview">
            <span>{{ selectedAgent.source_path ? t('connectorSource') : t('connectorRegistry') }}</span>
            <code>{{ sourcePathForAgent(selectedAgent) }}</code>
          </div>
          <div v-if="selectedAgentTarget" class="target-path-preview">
            <span>{{ t('connectorTarget') }}</span>
            <code>{{ selectedAgentTarget.target_path }}</code>
          </div>
          <div v-if="selectedAgentTarget?.error" class="target-error-panel">
            {{ selectedAgentTarget.error }}
          </div>
        </aside>
      </div>
    </details>
  </div>
</template>

<style scoped>
.connector-row {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px 16px;
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  background: var(--bb-surface);
}

.connector-summary {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
}

.state-light {
  align-self: center;
  width: 12px;
  height: 12px;
  border-radius: 999px;
  box-shadow: 0 0 0 2px var(--bb-hairline);
}

.state-light[data-tone='yellow'] {
  box-shadow: 0 0 8px var(--bb-warning);
}
.state-light[data-tone='red'] {
  box-shadow: 0 0 8px var(--bb-error);
}
.state-light[data-tone='green'] {
  box-shadow: 0 0 6px var(--bb-success);
}

.connector-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.connector-headline {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 8px;
}

.connector-name {
  font-weight: 600;
  font-size: 14px;
  color: var(--bb-text-strong);
}

.connector-id {
  font-size: 12px;
  color: var(--bb-text-muted);
  font-family: var(--bb-font-mono);
}

.state-label {
  font-size: 12px;
  color: var(--bb-text-muted);
}

.target-count {
  font-size: 11px;
  color: var(--bb-text);
  background: var(--bb-surface-muted);
  border-radius: 999px;
  padding: 2px 7px;
}

.registration-count {
  font-size: 11px;
  color: var(--bb-project-blackboard-fg);
  background: var(--bb-project-blackboard-bg);
  border-radius: 999px;
  padding: 2px 7px;
}

.symlink-badge {
  font-size: 11px;
  color: var(--bb-focus);
  background: var(--bb-accent-soft);
  padding: 2px 6px;
  border-radius: 999px;
}

.registration-target-list {
  display: grid;
  gap: 4px;
  max-width: 720px;
}

.registration-target {
  min-width: 0;
  display: grid;
  grid-template-columns: 10px minmax(72px, auto) minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--bb-text-muted);
  font-family: var(--bb-font-mono);
  font-size: 12px;
  text-align: left;
  cursor: pointer;
}

.target-state-dot {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  box-shadow: 0 0 0 1px var(--bb-hairline);
}

.target-state-dot[data-tone='yellow'] {
  box-shadow: 0 0 4px var(--bb-warning);
}

.target-state-dot[data-tone='red'] {
  box-shadow: 0 0 4px var(--bb-error);
}

.target-state-dot[data-tone='green'] {
  box-shadow: 0 0 3px var(--bb-success);
}

.target-label {
  color: var(--bb-text-muted);
  font-weight: 650;
}

.target-state-text {
  color: var(--bb-text-muted);
  font-size: 11px;
  white-space: nowrap;
}

.registration-target span {
  color: var(--bb-text-muted);
  font-weight: 650;
}

.registration-target code {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--bb-text);
  font-family: var(--bb-font-mono);
}

.target-error {
  grid-column: 3 / -1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--bb-error);
  font-size: 11px;
  font-weight: 500;
}

.registration-target:hover code {
  color: var(--bb-text-strong);
  text-decoration: underline;
}

.connector-details {
  border-top: 1px solid var(--bb-hairline);
  padding-top: 8px;
}

.connector-details summary {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  color: var(--bb-text);
  font-size: 12px;
  font-weight: 650;
  list-style: none;
}

.connector-details summary::-webkit-details-marker {
  display: none;
}

.connector-details-chevron {
  width: 14px;
  height: 14px;
  color: var(--bb-text-faint);
  transition: transform 0.16s ease;
}

.connector-details:not([open]) .connector-details-chevron {
  transform: rotate(-90deg);
}

.connector-detail-body {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(280px, 360px);
  gap: 12px;
  padding-top: 10px;
}

.target-rows {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 360px;
  overflow: auto;
  padding-right: 4px;
}

.target-row-item {
  width: 100%;
  border: 1px solid var(--bb-hairline);
  background: var(--bb-surface);
  border-radius: 6px;
  padding: 8px 10px;
  gap: 2px;
  display: grid;
  font-family: var(--bb-font-mono);
  font-size: 12px;
  color: var(--bb-text);
  text-align: left;
  cursor: pointer;
}

.target-row-item:hover,
.target-row-item.selected {
  border-color: color-mix(in srgb, var(--bb-project-blackboard-bg) 28%, var(--bb-hairline));
  background: color-mix(in srgb, var(--bb-project-blackboard-bg) 6%, var(--bb-surface));
}

.target-row-main {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
}

.target-row-main strong {
  color: var(--bb-text-strong);
}

.target-row-main small {
  color: var(--bb-text-muted);
}

.target-row-path {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--bb-text-muted);
  font-family: var(--bb-font-mono);
}

.target-preview {
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  background: var(--bb-surface-soft);
  padding: 12px;
  min-width: 0;
}

.target-preview-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
}

.target-preview h4 {
  margin: 0 0 2px;
  color: var(--bb-text-strong);
  font-size: 14px;
}

.target-preview p {
  margin: 0;
  color: var(--bb-text-muted);
  font-family: var(--bb-font-mono);
}

.target-meta-grid {
  margin: 12px 0;
  --bb-info-grid-gap: 8px;
}

.target-path-preview {
  display: grid;
  gap: 3px;
  margin-top: 8px;
}

.target-path-preview span {
  color: var(--bb-text-muted);
  font-size: 11px;
}

.target-path-preview code {
  padding: 6px 8px;
  border-radius: 6px;
  background: var(--bb-surface);
  border: 1px solid var(--bb-hairline);
  color: var(--bb-text);
  font-family: var(--bb-font-mono);
  font-size: 11px;
  overflow-wrap: anywhere;
}

.target-error-panel {
  margin-top: 8px;
  padding: 8px 10px;
  border-radius: 6px;
  border: 1px solid var(--bb-error);
  background: var(--bb-md-error-bg);
  color: var(--bb-md-error-text);
  font-size: 12px;
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.tool-status {
  display: grid;
  gap: 4px;
  max-width: 720px;
  padding: 7px 9px;
  border: 1px solid var(--bb-hairline);
  border-radius: 6px;
  background: var(--bb-surface-soft);
  font-size: 12px;
}

.tool-status[data-tone='green'] {
  border-color: color-mix(in srgb, var(--bb-success) 32%, var(--bb-hairline));
}

.tool-status[data-tone='yellow'] {
  border-color: color-mix(in srgb, var(--bb-warning) 36%, var(--bb-hairline));
}

.tool-status[data-tone='red'] {
  border-color: color-mix(in srgb, var(--bb-error) 36%, var(--bb-hairline));
}

.tool-status-main {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
  min-width: 0;
}

.tool-status-label {
  color: var(--bb-text-strong);
  font-weight: 700;
}

.tool-version {
  color: var(--bb-text-muted);
}

.tool-error {
  color: var(--bb-error);
  overflow-wrap: anywhere;
}

.tool-component-list {
  display: grid;
  gap: 5px;
  padding-top: 4px;
  border-top: 1px solid var(--bb-hairline);
}

.tool-component {
  display: grid;
  grid-template-columns: minmax(120px, auto) auto;
  gap: 6px 8px;
  align-items: center;
  min-width: 0;
  color: var(--bb-text-muted);
}

.tool-component[data-tone='green'] .component-status {
  color: var(--bb-success);
}

.tool-component[data-tone='yellow'] .component-status {
  color: var(--bb-warning);
}

.tool-component[data-tone='red'] .component-status {
  color: var(--bb-error);
}

.component-name {
  color: var(--bb-text);
  font-weight: 650;
}

.component-status {
  font-weight: 650;
  white-space: nowrap;
}

.connector-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.btn {
  font-size: 13px;
  padding: 6px 12px;
  border-radius: 6px;
  border: 1px solid transparent;
  cursor: pointer;
  transition: background 0.15s ease, border-color 0.15s ease;
}

.btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.btn-primary {
  background: var(--bb-project-blackboard-bg);
  color: var(--bb-project-blackboard-fg);
}

.btn-primary:hover:not(:disabled) {
  background: color-mix(in srgb, var(--bb-project-blackboard-bg) 86%, var(--bb-surface));
}

.btn-danger {
  background: var(--bb-surface);
  color: var(--bb-error);
  border-color: var(--bb-error);
}

.btn-danger:hover:not(:disabled) {
  background: var(--bb-md-error-bg);
  border-color: var(--bb-error);
}

.btn-ghost {
  background: var(--bb-surface);
  color: var(--bb-text);
  border-color: var(--bb-hairline);
}

.btn-ghost:hover:not(:disabled) {
  background: var(--bb-surface-soft);
}

:global(:root[data-theme='dark']) .connector-details {
  border-color: var(--bb-hairline);
}

:global(:root[data-theme='dark']) .target-row-item:hover,
:global(:root[data-theme='dark']) .target-row-item.selected {
  border-color: color-mix(in srgb, var(--bb-project-blackboard-bg) 28%, var(--bb-hairline));
  background: color-mix(in srgb, var(--bb-project-blackboard-bg) 8%, var(--bb-surface));
}

:global(:root[data-theme='dark']) .btn-ghost:hover:not(:disabled) {
  background: var(--bb-surface-muted);
}

@media (max-width: 920px) {
  .connector-summary,
  .connector-detail-body {
    grid-template-columns: 1fr;
  }

  .state-light {
    grid-row: 1;
  }

  .connector-actions {
    justify-self: start;
  }

  .registration-target {
    grid-template-columns: 10px 1fr auto;
    gap: 6px 8px;
  }

  .registration-target code {
    grid-column: 1 / -1;
  }
}
</style>
