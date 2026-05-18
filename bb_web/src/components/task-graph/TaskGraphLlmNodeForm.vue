<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Plus, RefreshCw, X } from 'lucide-vue-next'
import { BbButton, BbCheckboxField, BbDenseRow, BbField, BbSectionHeader, BbSegmentedControl } from '@/components/common'
import { t } from '@/i18n'
import { type TaskGraphNode } from '@/data/taskGraphs'
import { loadRuntimeModels, type McpServerConfig, type ProjectAgentProfile } from '@/data/agents'

const props = defineProps<{
  node: TaskGraphNode
  readonly: boolean
  projectAgents: ProjectAgentProfile[]
  projectAgentsError: string
  promptFileContent: string
  graphScope: string
  graphId: string
  project: string
}>()

const emit = defineEmits<{
  'update-config': [patch: Record<string, unknown>]
  'switch-mode': [mode: 'llm' | 'agent']
  'load-prompt-file': []
  'update-prompt-template': [value: string]
  'update-prompt-mode': [mode: string]
  'save-prompt-file': []
}>()

function inputValue(event: Event) {
  return event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLSelectElement
    ? event.target.value
    : ''
}

function configString(node: TaskGraphNode, key: string) {
  const value = node.config[key]
  return typeof value === 'string' ? value : ''
}

const llmRunAs = computed(() => props.node.config.run_as === 'agent' ? 'agent' : 'llm')
const llmModeOptions = computed(() => [
  { value: 'llm', label: t('taskGraphLlmModeLlm') },
  { value: 'agent', label: t('taskGraphLlmModeAgent') },
])

function switchMode(value: string) {
  if (value === 'llm' || value === 'agent') emit('switch-mode', value)
}

const selectedAgentProfile = computed(() => {
  const node = props.node
  if (!node || llmRunAs.value !== 'agent') return null
  const profileId = configString(node, 'agent_profile')
  return props.projectAgents.find((agent) => agent.id === profileId) ?? null
})

function agentLabel(agent: ProjectAgentProfile) {
  return agent.display_name ? `${agent.display_name} (${agent.id})` : agent.id
}

function profileRuntimeSummary(profile: ProjectAgentProfile | null) {
  if (!profile) return ''
  return [
    profile.runtime ? `runtime ${profile.runtime}` : t('taskGraphRuntimeUnset'),
    profile.model ? `model ${profile.model}` : t('taskGraphModelUnset'),
    `${profile.skills?.length ?? 0} skills`,
    `${profile.mcp_servers?.length ?? 0} MCP`,
  ].join(' · ')
}

function agentTaskPromptHint(profile: ProjectAgentProfile | null) {
  return profile?.instructions_path
    ? t('taskGraphEmptyPromptDefault', { path: profile.instructions_path })
    : t('taskGraphPromptRequired')
}

const llmRuntimes = ['codex', 'opencode', 'codebuddy', 'pi']
const runtimeModels = ref<string[]>([])
const runtimeModelsError = ref('')
const runtimeModelsSource = ref('')
const mcpEnvErrors = ref<Record<number, string>>({})
const llmContractJsonErrors = ref<Record<string, string>>({})
const runtimeModelsLoading = ref(false)
let runtimeModelRequestId = 0

const selectedRuntime = computed(() => configString(props.node, 'runtime') || 'codex')
const selectedModel = computed(() => configString(props.node, 'model'))
const modelDatalistId = computed(() => `task-graph-models-${props.node.id.replace(/[^a-z0-9_-]/gi, '-')}`)
const modelOptions = computed(() => {
  const current = selectedModel.value.trim()
  const values = current ? [current, ...runtimeModels.value] : [...runtimeModels.value]
  return [...new Set(values.filter(Boolean))]
})
const llmToolkitOptions = computed(() => [
  {
    value: 'blackboard_mcp',
    label: t('taskGraphLlmNodeToolkitBlackboardMcp'),
    description: t('taskGraphLlmNodeToolkitBlackboardMcpHint'),
  },
  {
    value: 'browser_use',
    label: t('taskGraphLlmNodeToolkitBrowserUse'),
    description: t('taskGraphLlmNodeToolkitBrowserUseHint'),
  },
])

watch(
  [() => props.project, selectedRuntime, llmRunAs],
  () => {
    if (llmRunAs.value === 'llm') void refreshRuntimeModels()
  },
  { immediate: true },
)

async function refreshRuntimeModels() {
  const requestId = ++runtimeModelRequestId
  const runtime = selectedRuntime.value
  runtimeModelsLoading.value = true
  runtimeModelsError.value = ''
  runtimeModelsSource.value = ''
  try {
    const catalog = await loadRuntimeModels(props.project, runtime)
    if (requestId !== runtimeModelRequestId) return
    runtimeModels.value = catalog.models ?? []
    runtimeModelsSource.value = catalog.source || ''
    runtimeModelsError.value = catalog.error ?? ''
  } catch (error) {
    if (requestId !== runtimeModelRequestId) return
    runtimeModels.value = []
    runtimeModelsError.value = error instanceof Error ? error.message : String(error)
  } finally {
    if (requestId === runtimeModelRequestId) runtimeModelsLoading.value = false
  }
}

function promptTemplate(node: TaskGraphNode) {
  const prompt = node.config.prompt
  return prompt && typeof prompt === 'object' && 'template' in prompt
    ? String((prompt as Record<string, unknown>).template ?? '')
    : ''
}

function promptMode(node: TaskGraphNode) {
  const prompt = node.config.prompt
  return prompt && typeof prompt === 'object' && 'mode' in prompt
    ? String((prompt as Record<string, unknown>).mode ?? 'inline')
    : 'inline'
}

function llmSkills(node: TaskGraphNode) {
  const value = node.config.skills
  return Array.isArray(value) ? value.map(String) : []
}

function llmToolkits(node: TaskGraphNode) {
  const value = node.config.toolkits
  if (!Array.isArray(value)) return []
  return value.map((item) => {
    if (typeof item === 'string') return item
    if (item && typeof item === 'object' && !Array.isArray(item)) {
      const id = (item as Record<string, unknown>).id
      return typeof id === 'string' ? id : ''
    }
    return ''
  }).filter(Boolean)
}

function updateToolkit(toolkit: string, enabled: boolean) {
  const current = new Set(llmToolkits(props.node))
  if (enabled) current.add(toolkit)
  else current.delete(toolkit)
  emit('update-config', { toolkits: [...current] })
}

function addSkill() {
  emit('update-config', { skills: [...llmSkills(props.node), ''] })
}

function updateSkill(index: number, value: string) {
  const skills = [...llmSkills(props.node)]
  skills[index] = value
  emit('update-config', { skills })
}

function removeSkill(index: number) {
  emit('update-config', { skills: llmSkills(props.node).filter((_, itemIndex) => itemIndex !== index) })
}

function llmMcpServers(node: TaskGraphNode): McpServerConfig[] {
  const value = node.config.mcp_servers
  return Array.isArray(value)
    ? value.map((item) => {
        const server = item as Partial<McpServerConfig>
        return {
          name: typeof server.name === 'string' ? server.name : '',
          transport: server.transport === 'sse' ? 'sse' : 'stdio',
          command: typeof server.command === 'string' ? server.command : '',
          args: Array.isArray(server.args) ? server.args.map(String) : [],
          url: typeof server.url === 'string' ? server.url : '',
          env: server.env && typeof server.env === 'object' && !Array.isArray(server.env)
            ? server.env as Record<string, string>
            : {},
        }
      })
    : []
}

function addMcpServer() {
  emit('update-config', {
    mcp_servers: [
      ...llmMcpServers(props.node),
      { name: '', transport: 'stdio', command: '', args: [], env: {} },
    ],
  })
}

function updateMcpServer(index: number, patch: Partial<McpServerConfig>) {
  const servers = llmMcpServers(props.node)
  servers[index] = { ...servers[index], ...patch }
  emit('update-config', { mcp_servers: servers })
}

function mcpTransport(value: string): 'stdio' | 'sse' {
  return value === 'sse' ? 'sse' : 'stdio'
}

function removeMcpServer(index: number) {
  emit('update-config', { mcp_servers: llmMcpServers(props.node).filter((_, itemIndex) => itemIndex !== index) })
}

function mcpArgsText(server: McpServerConfig) {
  return (server.args ?? []).join('\n')
}

function updateMcpArgs(index: number, value: string) {
  updateMcpServer(index, { args: value.split(/\r?\n/).map((item) => item.trim()).filter(Boolean) })
}

function mcpEnvText(server: McpServerConfig) {
  return JSON.stringify(server.env ?? {}, null, 2)
}

function updateMcpEnv(index: number, value: string) {
  try {
    const parsed = JSON.parse(value) as Record<string, string>
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      updateMcpServer(index, { env: parsed })
      const errors = { ...mcpEnvErrors.value }
      delete errors[index]
      mcpEnvErrors.value = errors
      return
    }
  } catch { /* fall through to error */ }
  mcpEnvErrors.value = { ...mcpEnvErrors.value, [index]: t('mcpEnvInvalidJson') }
}

function llmCustomEnv(node: TaskGraphNode): Record<string, string> {
  const value = node.config.custom_env
  return value && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, string>
    : {}
}

function customEnvEntries(node: TaskGraphNode) {
  return Object.entries(llmCustomEnv(node))
}

function addCustomEnv() {
  const env = { ...llmCustomEnv(props.node) }
  let index = Object.keys(env).length + 1
  let key = `KEY_${index}`
  while (env[key] !== undefined) {
    index += 1
    key = `KEY_${index}`
  }
  env[key] = ''
  emit('update-config', { custom_env: env })
}

function updateCustomEnvKey(oldKey: string, newKey: string) {
  if (!newKey || oldKey === newKey) return
  const env = { ...llmCustomEnv(props.node) }
  const value = env[oldKey] ?? ''
  delete env[oldKey]
  env[newKey] = value
  emit('update-config', { custom_env: env })
}

function updateCustomEnvValue(key: string, value: string) {
  emit('update-config', { custom_env: { ...llmCustomEnv(props.node), [key]: value } })
}

function removeCustomEnv(key: string) {
  const env = { ...llmCustomEnv(props.node) }
  delete env[key]
  emit('update-config', { custom_env: env })
}

function llmCustomArgs(node: TaskGraphNode) {
  const value = node.config.custom_args
  return Array.isArray(value) ? value.map(String) : []
}

function addCustomArg() {
  emit('update-config', { custom_args: [...llmCustomArgs(props.node), ''] })
}

function updateCustomArg(index: number, value: string) {
  const args = [...llmCustomArgs(props.node)]
  args[index] = value
  emit('update-config', { custom_args: args })
}

function removeCustomArg(index: number) {
  emit('update-config', { custom_args: llmCustomArgs(props.node).filter((_, itemIndex) => itemIndex !== index) })
}

function llmInputs(node: TaskGraphNode) {
  const raw = node.config.inputs
  return raw && typeof raw === 'object' && !Array.isArray(raw)
    ? raw as Record<string, unknown>
    : {}
}

function configObject(node: TaskGraphNode, key: string): Record<string, unknown> {
  const raw = node.config[key]
  return raw && typeof raw === 'object' && !Array.isArray(raw)
    ? raw as Record<string, unknown>
    : {}
}

function resourceBundleInput(node: TaskGraphNode) {
  const value = configObject(node, 'resource_bundle').input
  return typeof value === 'string' ? value : ''
}

function resourceBundleRequired(node: TaskGraphNode) {
  return configObject(node, 'resource_bundle').required === true
}

function updateResourceBundleInput(value: string) {
  const input = value.trim()
  if (!input) {
    emit('update-config', { resource_bundle: null })
    return
  }
  emit('update-config', {
    resource_bundle: {
      ...configObject(props.node, 'resource_bundle'),
      input,
    },
  })
}

function updateResourceBundleRequired(required: boolean) {
  const current = configObject(props.node, 'resource_bundle')
  const input = typeof current.input === 'string' && current.input.trim()
    ? current.input.trim()
    : 'resource_bundle'
  emit('update-config', {
    resource_bundle: {
      ...current,
      input,
      required,
    },
  })
}

function configJsonText(node: TaskGraphNode, key: string) {
  const raw = node.config[key]
  if (raw === undefined || raw === null) return ''
  try {
    return JSON.stringify(raw, null, 2)
  } catch {
    return String(raw)
  }
}

function updateJsonConfigField(key: string, value: string) {
  if (!value.trim()) {
    emit('update-config', { [key]: null })
    const errors = { ...llmContractJsonErrors.value }
    delete errors[key]
    llmContractJsonErrors.value = errors
    return
  }
  try {
    const parsed = JSON.parse(value) as unknown
    emit('update-config', { [key]: parsed })
    const errors = { ...llmContractJsonErrors.value }
    delete errors[key]
    llmContractJsonErrors.value = errors
  } catch {
    llmContractJsonErrors.value = {
      ...llmContractJsonErrors.value,
      [key]: t('taskGraphInvalidJson'),
    }
  }
}

function updateLlmInput(key: string, value: string) {
  const inputs = { ...llmInputs(props.node), [key]: value }
  emit('update-config', { inputs })
}

function renameLlmInput(oldKey: string, newKey: string) {
  if (!newKey || oldKey === newKey) return
  const inputs = { ...llmInputs(props.node) }
  const value = inputs[oldKey]
  delete inputs[oldKey]
  inputs[newKey] = value ?? ''
  emit('update-config', { inputs })
}

function removeLlmInput(key: string) {
  const inputs = { ...llmInputs(props.node) }
  delete inputs[key]
  emit('update-config', { inputs })
}

function addLlmInput() {
  const inputs = { ...llmInputs(props.node) }
  let idx = 1
  while (inputs[`input_${idx}`] !== undefined) idx++
  inputs[`input_${idx}`] = ''
  emit('update-config', { inputs })
}

function promptVars(node: TaskGraphNode) {
  const raw = node.config.prompt_vars
  return raw && typeof raw === 'object' && !Array.isArray(raw)
    ? raw as Record<string, unknown>
    : {}
}

function updatePromptVar(key: string, value: string) {
  const vars = { ...promptVars(props.node), [key]: value }
  emit('update-config', { prompt_vars: vars })
}

function renamePromptVar(oldKey: string, newKey: string) {
  if (!newKey || oldKey === newKey) return
  const vars = { ...promptVars(props.node) }
  const value = vars[oldKey]
  delete vars[oldKey]
  vars[newKey] = value ?? ''
  emit('update-config', { prompt_vars: vars })
}

function removePromptVar(key: string) {
  const vars = { ...promptVars(props.node) }
  delete vars[key]
  emit('update-config', { prompt_vars: vars })
}

function addPromptVar() {
  const vars = { ...promptVars(props.node) }
  let idx = 1
  while (vars[`var_${idx}`] !== undefined) idx++
  vars[`var_${idx}`] = ''
  emit('update-config', { prompt_vars: vars })
}
</script>

<template>
  <div class="task-graph-llm-form">
    <BbSegmentedControl
      :model-value="llmRunAs"
      :options="llmModeOptions"
      :aria-label="t('taskGraphLlmNodeMode')"
      @change="switchMode"
    />

    <template v-if="llmRunAs === 'agent'">
      <BbField :label="t('taskGraphAgentProfile')">
        <select :value="configString(node, 'agent_profile')" @change="emit('update-config', { agent_profile: inputValue($event) })">
          <option value="">{{ t('taskGraphSelectAgent') }}</option>
          <option v-for="agent in projectAgents" :key="agent.id" :value="agent.id">{{ agentLabel(agent) }}</option>
        </select>
      </BbField>
      <div v-if="selectedAgentProfile" class="task-graph-agent-summary">
        <strong>{{ selectedAgentProfile.display_name || selectedAgentProfile.id }}</strong>
        <span>{{ profileRuntimeSummary(selectedAgentProfile) }}</span>
        <span v-if="selectedAgentProfile.instructions_path">{{ t('taskGraphDefaultPrompt', { path: selectedAgentProfile.instructions_path }) }}</span>
      </div>
      <p v-else-if="projectAgentsError" class="task-graph-muted-note">{{ projectAgentsError }}</p>
      <p v-else class="task-graph-muted-note">{{ t('taskGraphNoActiveAgent') }}</p>
      <template v-if="promptMode(node) === 'file'">
        <BbField :label="t('taskGraphLegacyPromptFile')">
          <input :value="promptTemplate(node)" disabled />
        </BbField>
        <BbButton class="task-graph-inline-action" size="sm" variant="secondary" @click="emit('load-prompt-file')">
          {{ t('taskGraphLoadFileContent') }}
        </BbButton>
        <BbField v-if="promptFileContent" :label="t('taskGraphPreview')">
          <textarea class="task-graph-prompt-preview" :value="promptFileContent" disabled />
        </BbField>
        <BbButton v-if="promptFileContent" class="task-graph-inline-action" size="sm" variant="secondary" @click="emit('update-prompt-mode', 'inline'); emit('update-prompt-template', promptFileContent)">
          {{ t('taskGraphLlmNodeUseInlinePrompt') }}
        </BbButton>
      </template>
      <template v-else>
        <BbField :label="t('taskGraphTaskPrompt')">
          <textarea :value="promptTemplate(node)" :placeholder="t('taskGraphPromptPlaceholder')" @input="emit('update-prompt-template', inputValue($event))" />
        </BbField>
        <p class="task-graph-muted-note">{{ agentTaskPromptHint(selectedAgentProfile) }}</p>
      </template>
    </template>

    <template v-else>
      <BbField :label="t('taskGraphRuntimeLabel')">
        <select :value="configString(node, 'runtime')" @change="emit('update-config', { runtime: inputValue($event) })">
          <option v-for="runtime in llmRuntimes" :key="runtime" :value="runtime">{{ runtime }}</option>
        </select>
      </BbField>
      <BbField :label="t('taskGraphProviderAgent')">
        <input :value="configString(node, 'agent')" :placeholder="t('agentChatProfileFallback', { runtime: selectedRuntime })" @input="emit('update-config', { agent: inputValue($event) })" />
      </BbField>
      <BbField :label="t('taskGraphModelLabel')">
        <div class="task-graph-model-picker">
          <input
            :value="selectedModel"
            :list="modelDatalistId"
            :placeholder="t('taskGraphDefaultModel')"
            @input="emit('update-config', { model: inputValue($event) })"
          />
          <BbButton
            size="mini"
            variant="secondary"
            icon-only
            :disabled="readonly || runtimeModelsLoading"
            :title="t('taskGraphRefreshModels')"
            :aria-label="t('taskGraphRefreshModels')"
            @click="refreshRuntimeModels"
          >
            <RefreshCw aria-hidden="true" />
          </BbButton>
        </div>
        <datalist :id="modelDatalistId">
          <option v-for="model in modelOptions" :key="model" :value="model" />
        </datalist>
      </BbField>
      <p v-if="runtimeModelsLoading" class="task-graph-muted-note">{{ t('taskGraphLoadingModels', { runtime: selectedRuntime }) }}</p>
      <p v-else-if="runtimeModelsError" class="task-graph-muted-note">{{ runtimeModelsError }}</p>
      <p v-else-if="runtimeModelsSource && modelOptions.length > 0" class="task-graph-muted-note">{{ t('taskGraphModelsFrom', { count: modelOptions.length, source: runtimeModelsSource }) }}</p>
      <template v-if="promptMode(node) === 'file'">
        <BbField :label="t('taskGraphLegacyPromptFile')">
          <input :value="promptTemplate(node)" disabled />
        </BbField>
        <BbButton class="task-graph-inline-action" size="sm" variant="secondary" @click="emit('load-prompt-file')">
          {{ t('taskGraphLoadFileContent') }}
        </BbButton>
        <BbField v-if="promptFileContent" :label="t('taskGraphPreview')">
          <textarea class="task-graph-prompt-preview" :value="promptFileContent" disabled />
        </BbField>
        <BbButton v-if="promptFileContent" class="task-graph-inline-action" size="sm" variant="secondary" @click="emit('update-prompt-mode', 'inline'); emit('update-prompt-template', promptFileContent)">
          {{ t('taskGraphUseInlinePrompt') }}
        </BbButton>
      </template>
      <template v-else>
        <BbField :label="t('taskGraphLlmNodePrompt')">
          <textarea :value="promptTemplate(node)" @input="emit('update-prompt-template', inputValue($event))" />
        </BbField>
      </template>

      <div class="task-graph-inline-list">
        <div class="task-graph-inline-list-head">
          <strong>{{ t('taskGraphLlmNodeInputBindings') }}</strong>
          <BbButton size="mini" variant="secondary" icon-only @click="addLlmInput">
            <Plus aria-hidden="true" />
          </BbButton>
        </div>
        <BbDenseRow v-for="[key, value] in Object.entries(llmInputs(node))" :key="`in-${key}`" class="task-graph-inline-row">
          <input class="bb-dense-control" :value="key" @change="renameLlmInput(key, inputValue($event))" />
          <input class="bb-dense-control" :value="value" @input="updateLlmInput(key, inputValue($event))" />
          <BbButton size="mini" variant="secondary" icon-only @click="removeLlmInput(key)">
            <X aria-hidden="true" />
          </BbButton>
        </BbDenseRow>
      </div>

      <div class="task-graph-inline-list task-graph-toolkit-list">
        <BbSectionHeader
          :title="t('taskGraphLlmNodeToolkits')"
          as="div"
          title-tag="h4"
          density="compact"
          :divider="false"
        />
        <BbDenseRow
          v-for="option in llmToolkitOptions"
          :key="option.value"
          class="task-graph-toolkit-row"
          columns="minmax(0, 1fr)"
          padding="7px 8px"
        >
          <BbCheckboxField
            :label="option.label"
            :checked="llmToolkits(node).includes(option.value)"
            @change="(checked) => updateToolkit(option.value, checked)"
          />
          <p class="task-graph-muted-note">{{ option.description }}</p>
        </BbDenseRow>
      </div>

      <details class="task-graph-advanced" :open="Boolean(node.config.resource_bundle || node.config.tool_policy || node.config.output_contract || node.config.lifecycle)">
        <summary>{{ t('taskGraphEnhancedLlmContract') }}</summary>
        <BbField :label="t('taskGraphResourceBundleInputPin')">
          <input
            :value="resourceBundleInput(node)"
            :placeholder="t('taskGraphResourceBundlePlaceholder')"
            @input="updateResourceBundleInput(inputValue($event))"
          />
        </BbField>
        <BbCheckboxField
          :label="t('taskGraphResourceBundleRequired')"
          :checked="resourceBundleRequired(node)"
          @change="updateResourceBundleRequired"
        />
        <BbField :label="t('taskGraphOutputContractJson')">
          <textarea
            :value="configJsonText(node, 'output_contract')"
            :placeholder="t('taskGraphOutputContractPlaceholder')"
            @change="updateJsonConfigField('output_contract', inputValue($event))"
          />
        </BbField>
        <p v-if="llmContractJsonErrors.output_contract" class="task-graph-muted-note">{{ llmContractJsonErrors.output_contract }}</p>
        <BbField :label="t('taskGraphToolPolicyJson')">
          <textarea
            :value="configJsonText(node, 'tool_policy')"
            :placeholder="t('taskGraphToolPolicyPlaceholder')"
            @change="updateJsonConfigField('tool_policy', inputValue($event))"
          />
        </BbField>
        <p v-if="llmContractJsonErrors.tool_policy" class="task-graph-muted-note">{{ llmContractJsonErrors.tool_policy }}</p>
        <BbField :label="t('taskGraphLifecycleJson')">
          <textarea
            :value="configJsonText(node, 'lifecycle')"
            :placeholder="t('taskGraphLifecyclePlaceholder')"
            @change="updateJsonConfigField('lifecycle', inputValue($event))"
          />
        </BbField>
        <p v-if="llmContractJsonErrors.lifecycle" class="task-graph-muted-note">{{ llmContractJsonErrors.lifecycle }}</p>
      </details>

      <div class="task-graph-inline-list">
        <div class="task-graph-inline-list-head">
          <strong>{{ t('taskGraphLlmNodePromptVars') }}</strong>
          <BbButton size="mini" variant="secondary" icon-only @click="addPromptVar">
            <Plus aria-hidden="true" />
          </BbButton>
        </div>
        <BbDenseRow v-for="[key, value] in Object.entries(promptVars(node))" :key="`var-${key}`" class="task-graph-inline-row">
          <input class="bb-dense-control" :value="key" @change="renamePromptVar(key, inputValue($event))" />
          <input class="bb-dense-control" :value="value" @input="updatePromptVar(key, inputValue($event))" />
          <BbButton size="mini" variant="secondary" icon-only @click="removePromptVar(key)">
            <X aria-hidden="true" />
          </BbButton>
        </BbDenseRow>
      </div>

      <div class="task-graph-inline-list">
        <div class="task-graph-inline-list-head">
          <strong>{{ t('taskGraphLlmNodeSkills') }}</strong>
          <BbButton size="mini" variant="secondary" icon-only @click="addSkill">
            <Plus aria-hidden="true" />
          </BbButton>
        </div>
        <BbDenseRow v-for="(skill, index) in llmSkills(node)" :key="`skill-${index}`" class="task-graph-inline-row">
          <input class="bb-dense-control" :value="skill" :placeholder="t('taskGraphLlmNodePlaceholderSkill')" @input="updateSkill(index, inputValue($event))" />
          <BbButton size="mini" variant="secondary" icon-only @click="removeSkill(index)">
            <X aria-hidden="true" />
          </BbButton>
        </BbDenseRow>
      </div>

      <div class="task-graph-inline-list">
        <div class="task-graph-inline-list-head">
          <strong>{{ t('taskGraphLlmNodeMcpServers') }}</strong>
          <BbButton size="mini" variant="secondary" icon-only @click="addMcpServer">
            <Plus aria-hidden="true" />
          </BbButton>
        </div>
        <div v-for="(server, index) in llmMcpServers(node)" :key="`mcp-${index}`" class="task-graph-mcp-editor">
          <BbDenseRow class="task-graph-inline-row">
            <input class="bb-dense-control" :value="server.name" :placeholder="t('taskGraphLlmNodePlaceholderServerName')" @input="updateMcpServer(index, { name: inputValue($event) })" />
            <select class="bb-dense-control" :value="server.transport" @change="updateMcpServer(index, { transport: mcpTransport(inputValue($event)) })">
              <option value="stdio">stdio</option>
              <option value="sse">sse</option>
            </select>
            <BbButton size="mini" variant="secondary" icon-only @click="removeMcpServer(index)">
              <X aria-hidden="true" />
            </BbButton>
          </BbDenseRow>
          <BbField v-if="server.transport === 'stdio'" :label="t('taskGraphLlmNodeCommand')">
            <input :value="server.command ?? ''" @input="updateMcpServer(index, { command: inputValue($event) })" />
          </BbField>
          <BbField v-if="server.transport === 'stdio'" :label="t('taskGraphLlmNodeArgs')">
            <textarea :value="mcpArgsText(server)" :placeholder="t('taskGraphLlmNodePlaceholderArgs')" @input="updateMcpArgs(index, inputValue($event))" />
          </BbField>
          <BbField v-if="server.transport === 'sse'" :label="t('taskGraphLlmNodeUrl')">
            <input :value="server.url ?? ''" @input="updateMcpServer(index, { url: inputValue($event) })" />
          </BbField>
          <BbField :label="t('taskGraphLlmNodeEnvJson')">
            <textarea :value="mcpEnvText(server)" @change="updateMcpEnv(index, inputValue($event))" />
          </BbField>
          <p v-if="mcpEnvErrors[index]" class="task-graph-muted-note">{{ mcpEnvErrors[index] }}</p>
        </div>
      </div>

      <details class="task-graph-advanced">
        <summary>{{ t('taskGraphLlmNodeAdvancedEnvArgs') }}</summary>
        <div class="task-graph-inline-list">
          <div class="task-graph-inline-list-head">
            <strong>{{ t('taskGraphLlmNodeCustomEnv') }}</strong>
            <BbButton size="mini" variant="secondary" icon-only @click="addCustomEnv">
              <Plus aria-hidden="true" />
            </BbButton>
          </div>
          <BbDenseRow v-for="[key, value] in customEnvEntries(node)" :key="`env-${key}`" class="task-graph-inline-row">
            <input class="bb-dense-control" :value="key" @change="updateCustomEnvKey(key, inputValue($event))" />
            <input class="bb-dense-control" :value="value" @input="updateCustomEnvValue(key, inputValue($event))" />
            <BbButton size="mini" variant="secondary" icon-only @click="removeCustomEnv(key)">
              <X aria-hidden="true" />
            </BbButton>
          </BbDenseRow>
        </div>
        <div class="task-graph-inline-list">
          <div class="task-graph-inline-list-head">
            <strong>{{ t('taskGraphLlmNodeCustomArgs') }}</strong>
            <BbButton size="mini" variant="secondary" icon-only @click="addCustomArg">
              <Plus aria-hidden="true" />
            </BbButton>
          </div>
          <BbDenseRow v-for="(arg, index) in llmCustomArgs(node)" :key="`arg-${index}`" class="task-graph-inline-row">
            <input class="bb-dense-control" :value="arg" @input="updateCustomArg(index, inputValue($event))" />
            <BbButton size="mini" variant="secondary" icon-only @click="removeCustomArg(index)">
              <X aria-hidden="true" />
            </BbButton>
          </BbDenseRow>
        </div>
      </details>
    </template>
  </div>
</template>

<style scoped>
.task-graph-llm-form {
  display: grid;
  gap: 9px;
  min-width: 0;
}

.task-graph-prompt-preview {
  min-height: 120px;
}

.task-graph-agent-summary,
.task-graph-inline-list,
.task-graph-mcp-editor,
.task-graph-advanced {
  display: grid;
  gap: 7px;
  min-width: 0;
}

.task-graph-agent-summary,
.task-graph-mcp-editor {
  padding: 8px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-agent-summary strong,
.task-graph-inline-list-head strong {
  color: var(--bb-text-strong);
  font-size: 12px;
}

.task-graph-agent-summary span,
.task-graph-muted-note {
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 11px;
  overflow-wrap: anywhere;
}

.task-graph-inline-list-head {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 6px;
}

.task-graph-inline-row {
  --bb-dense-row-columns: minmax(0, 1fr) auto;
  --bb-dense-row-padding: 0;
  --bb-dense-row-border: transparent;
  --bb-dense-row-bg: transparent;
  --bb-dense-control-height: 32px;
}

.task-graph-inline-row:has(input + input) {
  --bb-dense-row-columns: minmax(0, 0.75fr) minmax(0, 1fr) auto;
}

.task-graph-model-picker {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 30px;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.task-graph-mcp-editor > .task-graph-inline-row {
  --bb-dense-row-columns: minmax(0, 1fr) minmax(72px, 0.45fr) auto;
}

.task-graph-toolkit-list {
  --bb-section-header-min-height: 22px;
  --bb-section-header-padding-bottom: 0;
}

.task-graph-toolkit-row {
  --bb-dense-row-gap: 3px;
  align-items: start;
}

.task-graph-advanced summary {
  color: var(--bb-text-muted);
  cursor: pointer;
  font-size: 12px;
  font-weight: 760;
}

.task-graph-inline-action {
  justify-self: start;
}

.task-graph-muted-note {
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 11px;
  overflow-wrap: anywhere;
}
</style>
