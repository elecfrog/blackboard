<script setup lang="ts">
import { computed, ref } from 'vue'
import { Plus, X } from 'lucide-vue-next'
import { t } from '@/i18n'
import { type TaskGraphNode } from '@/data/taskGraphs'
import { type McpServerConfig, type ProjectAgentProfile } from '@/data/agents'

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

const selectedAgentProfile = computed(() => {
  const node = props.node
  if (!node || node.type !== 'llm' || llmRunAs.value !== 'agent') return null
  const profileId = configString(node, 'agent_profile')
  return props.projectAgents.find((agent) => agent.id === profileId) ?? null
})

function agentLabel(agent: ProjectAgentProfile) {
  return agent.display_name ? `${agent.display_name} (${agent.id})` : agent.id
}

function profileRuntimeSummary(profile: ProjectAgentProfile | null) {
  if (!profile) return ''
  return [
    profile.runtime ? `runtime ${profile.runtime}` : 'runtime unset',
    profile.model ? `model ${profile.model}` : 'model unset',
    `${profile.skills?.length ?? 0} skills`,
    `${profile.mcp_servers?.length ?? 0} MCP`,
  ].join(' · ')
}

function agentTaskPromptHint(profile: ProjectAgentProfile | null) {
  return profile?.instructions_path
    ? `Empty prompt uses profile default: ${profile.instructions_path}`
    : 'Task prompt is required because this profile has no default prompt.'
}

const llmRuntimes = ['codex', 'opencode', 'codebuddy']

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
    }
  } catch { /* keep last valid env */ }
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
    <div class="task-graph-segmented" role="group" aria-label="LLM node mode">
      <button type="button" :class="{ active: llmRunAs === 'llm' }" @click="emit('switch-mode', 'llm')">LLM</button>
      <button type="button" :class="{ active: llmRunAs === 'agent' }" @click="emit('switch-mode', 'agent')">Agent</button>
    </div>

    <template v-if="llmRunAs === 'agent'">
      <label>
        <span>agent profile</span>
        <select :value="configString(node, 'agent_profile')" @change="emit('update-config', { agent_profile: inputValue($event) })">
          <option value="">Select agent</option>
          <option v-for="agent in projectAgents" :key="agent.id" :value="agent.id">{{ agentLabel(agent) }}</option>
        </select>
      </label>
      <div v-if="selectedAgentProfile" class="task-graph-agent-summary">
        <strong>{{ selectedAgentProfile.display_name || selectedAgentProfile.id }}</strong>
        <span>{{ profileRuntimeSummary(selectedAgentProfile) }}</span>
        <span v-if="selectedAgentProfile.instructions_path">default prompt {{ selectedAgentProfile.instructions_path }}</span>
      </div>
      <p v-else-if="projectAgentsError" class="task-graph-muted-note">{{ projectAgentsError }}</p>
      <p v-else class="task-graph-muted-note">No active project agent selected.</p>
      <template v-if="promptMode(node) === 'file'">
        <label>
          <span>legacy task prompt file</span>
          <input :value="promptTemplate(node)" disabled />
        </label>
        <button type="button" class="task-graph-inline-add" @click="emit('load-prompt-file')">{{ t('taskGraphLoadFileContent') }}</button>
        <label v-if="promptFileContent">
          <span>{{ t('taskGraphPreview') }}</span>
          <textarea :value="promptFileContent" disabled style="min-height: 120px" />
        </label>
        <button v-if="promptFileContent" type="button" class="task-graph-inline-add" @click="emit('update-prompt-mode', 'inline'); emit('update-prompt-template', promptFileContent)">
          Use inline task prompt
        </button>
      </template>
      <template v-else>
        <label>
          <span>task prompt</span>
          <textarea :value="promptTemplate(node)" placeholder="Leave empty to use the profile default prompt." @input="emit('update-prompt-template', inputValue($event))" />
        </label>
        <p class="task-graph-muted-note">{{ agentTaskPromptHint(selectedAgentProfile) }}</p>
      </template>
    </template>

    <template v-else>
      <label>
        <span>runtime</span>
        <select :value="configString(node, 'runtime')" @change="emit('update-config', { runtime: inputValue($event) })">
          <option v-for="runtime in llmRuntimes" :key="runtime" :value="runtime">{{ runtime }}</option>
        </select>
      </label>
      <label>
        <span>provider agent</span>
        <input :value="configString(node, 'agent')" placeholder="native" @input="emit('update-config', { agent: inputValue($event) })" />
      </label>
      <label>
        <span>model</span>
        <input :value="configString(node, 'model')" @input="emit('update-config', { model: inputValue($event) })" />
      </label>
      <template v-if="promptMode(node) === 'file'">
        <label>
          <span>legacy prompt file</span>
          <input :value="promptTemplate(node)" disabled />
        </label>
        <button type="button" class="task-graph-inline-add" @click="emit('load-prompt-file')">{{ t('taskGraphLoadFileContent') }}</button>
        <label v-if="promptFileContent">
          <span>{{ t('taskGraphPreview') }}</span>
          <textarea :value="promptFileContent" disabled style="min-height: 120px" />
        </label>
        <button v-if="promptFileContent" type="button" class="task-graph-inline-add" @click="emit('update-prompt-mode', 'inline'); emit('update-prompt-template', promptFileContent)">
          Use inline prompt
        </button>
      </template>
      <template v-else>
        <label>
          <span>prompt</span>
          <textarea :value="promptTemplate(node)" @input="emit('update-prompt-template', inputValue($event))" />
        </label>
      </template>

      <div class="task-graph-inline-list">
        <div class="task-graph-inline-list-head">
          <strong>input bindings</strong>
          <button type="button" class="task-graph-mini-button" @click="addLlmInput">
            <Plus aria-hidden="true" />
          </button>
        </div>
        <div v-for="[key, value] in Object.entries(llmInputs(node))" :key="`in-${key}`" class="task-graph-inline-row">
          <input :value="key" @change="renameLlmInput(key, inputValue($event))" />
          <input :value="value" @input="updateLlmInput(key, inputValue($event))" />
          <button type="button" class="task-graph-mini-button" @click="removeLlmInput(key)">
            <X aria-hidden="true" />
          </button>
        </div>
      </div>

      <div class="task-graph-inline-list">
        <div class="task-graph-inline-list-head">
          <strong>prompt vars</strong>
          <button type="button" class="task-graph-mini-button" @click="addPromptVar">
            <Plus aria-hidden="true" />
          </button>
        </div>
        <div v-for="[key, value] in Object.entries(promptVars(node))" :key="`var-${key}`" class="task-graph-inline-row">
          <input :value="key" @change="renamePromptVar(key, inputValue($event))" />
          <input :value="value" @input="updatePromptVar(key, inputValue($event))" />
          <button type="button" class="task-graph-mini-button" @click="removePromptVar(key)">
            <X aria-hidden="true" />
          </button>
        </div>
      </div>

      <div class="task-graph-inline-list">
        <div class="task-graph-inline-list-head">
          <strong>skills</strong>
          <button type="button" class="task-graph-mini-button" @click="addSkill">
            <Plus aria-hidden="true" />
          </button>
        </div>
        <div v-for="(skill, index) in llmSkills(node)" :key="`skill-${index}`" class="task-graph-inline-row">
          <input :value="skill" placeholder="triage" @input="updateSkill(index, inputValue($event))" />
          <button type="button" class="task-graph-mini-button" @click="removeSkill(index)">
            <X aria-hidden="true" />
          </button>
        </div>
      </div>

      <div class="task-graph-inline-list">
        <div class="task-graph-inline-list-head">
          <strong>MCP servers</strong>
          <button type="button" class="task-graph-mini-button" @click="addMcpServer">
            <Plus aria-hidden="true" />
          </button>
        </div>
        <div v-for="(server, index) in llmMcpServers(node)" :key="`mcp-${index}`" class="task-graph-mcp-editor">
          <div class="task-graph-inline-row">
            <input :value="server.name" placeholder="server name" @input="updateMcpServer(index, { name: inputValue($event) })" />
            <select :value="server.transport" @change="updateMcpServer(index, { transport: mcpTransport(inputValue($event)) })">
              <option value="stdio">stdio</option>
              <option value="sse">sse</option>
            </select>
            <button type="button" class="task-graph-mini-button" @click="removeMcpServer(index)">
              <X aria-hidden="true" />
            </button>
          </div>
          <label v-if="server.transport === 'stdio'">
            <span>command</span>
            <input :value="server.command ?? ''" @input="updateMcpServer(index, { command: inputValue($event) })" />
          </label>
          <label v-if="server.transport === 'stdio'">
            <span>args</span>
            <textarea :value="mcpArgsText(server)" placeholder="one argument per line" @input="updateMcpArgs(index, inputValue($event))" />
          </label>
          <label v-if="server.transport === 'sse'">
            <span>url</span>
            <input :value="server.url ?? ''" @input="updateMcpServer(index, { url: inputValue($event) })" />
          </label>
          <label>
            <span>env JSON</span>
            <textarea :value="mcpEnvText(server)" @change="updateMcpEnv(index, inputValue($event))" />
          </label>
        </div>
      </div>

      <details class="task-graph-advanced">
        <summary>advanced env / args</summary>
        <div class="task-graph-inline-list">
          <div class="task-graph-inline-list-head">
            <strong>custom env</strong>
            <button type="button" class="task-graph-mini-button" @click="addCustomEnv">
              <Plus aria-hidden="true" />
            </button>
          </div>
          <div v-for="[key, value] in customEnvEntries(node)" :key="`env-${key}`" class="task-graph-inline-row">
            <input :value="key" @change="updateCustomEnvKey(key, inputValue($event))" />
            <input :value="value" @input="updateCustomEnvValue(key, inputValue($event))" />
            <button type="button" class="task-graph-mini-button" @click="removeCustomEnv(key)">
              <X aria-hidden="true" />
            </button>
          </div>
        </div>
        <div class="task-graph-inline-list">
          <div class="task-graph-inline-list-head">
            <strong>custom args</strong>
            <button type="button" class="task-graph-mini-button" @click="addCustomArg">
              <Plus aria-hidden="true" />
            </button>
          </div>
          <div v-for="(arg, index) in llmCustomArgs(node)" :key="`arg-${index}`" class="task-graph-inline-row">
            <input :value="arg" @input="updateCustomArg(index, inputValue($event))" />
            <button type="button" class="task-graph-mini-button" @click="removeCustomArg(index)">
              <X aria-hidden="true" />
            </button>
          </div>
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

.task-graph-llm-form label {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.task-graph-llm-form label span {
  color: var(--bb-text-muted);
  font-size: 11px;
  font-weight: 760;
}

.task-graph-llm-form input,
.task-graph-llm-form select,
.task-graph-llm-form textarea {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  min-height: 32px;
  padding: 7px 8px;
  border: 1px solid var(--bb-border-warm-medium-strong);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-strong);
  font: inherit;
  font-size: 12px;
}

.task-graph-llm-form textarea {
  min-height: 80px;
  resize: vertical;
}

.task-graph-segmented {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 4px;
  padding: 3px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-segmented button,
.task-graph-mini-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-height: 28px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--bb-text-muted);
  cursor: pointer;
  font-size: 12px;
  font-weight: 760;
}

.task-graph-segmented button.active {
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
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

.task-graph-inline-list-head,
.task-graph-inline-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 6px;
}

.task-graph-inline-row:has(input + input) {
  grid-template-columns: minmax(0, 0.75fr) minmax(0, 1fr) auto;
}

.task-graph-mcp-editor > .task-graph-inline-row {
  grid-template-columns: minmax(0, 1fr) minmax(72px, 0.45fr) auto;
}

.task-graph-mini-button {
  width: 30px;
  border: 1px solid var(--bb-border-warm-medium);
  background: var(--bb-surface-soft);
}

.task-graph-mini-button svg {
  width: 14px;
  height: 14px;
}

.task-graph-advanced summary {
  color: var(--bb-text-muted);
  cursor: pointer;
  font-size: 12px;
  font-weight: 760;
}

.task-graph-inline-add {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-height: 28px;
  padding: 0 9px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-muted);
  cursor: pointer;
  font-size: 12px;
  font-weight: 760;
  justify-self: start;
}

.task-graph-inline-add svg {
  width: 14px;
  height: 14px;
}

.task-graph-muted-note {
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 11px;
  overflow-wrap: anywhere;
}
</style>
