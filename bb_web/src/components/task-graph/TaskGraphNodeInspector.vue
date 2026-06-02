<script setup lang="ts">
import { ref, computed } from 'vue'
import { PanelRightClose, Trash2 } from 'lucide-vue-next'
import { BbButton, BbCheckboxField, BbField, BbRefChip } from '@/components/common'
import { t } from '@/i18n'
import TaskGraphLlmNodeForm from './TaskGraphLlmNodeForm.vue'
import TaskGraphBranchNodeForm from './TaskGraphBranchNodeForm.vue'
import TaskGraphBindingList from './TaskGraphBindingList.vue'
import {
  type TaskGraphNode,
  type TaskGraphInputParam,
  type PinValueType,
} from '@/data/taskGraphs'
import { type ProjectAgentProfile, type McpServerConfig } from '@/data/agents'

const props = defineProps<{
  node: TaskGraphNode
  readonly: boolean
  projectAgents: ProjectAgentProfile[]
  projectAgentsError: string
  graphInputs: TaskGraphInputParam[]
  graphInputIds: string[]
  graphResourceIds: string[]
  availableGraphs: Array<{ id: string; scope: string; title: string }>
  project: string
  graphScope: string
  graphId: string
  selectedNodeErrors: Array<{ message: string }>
  promptFileContent: string
}>()

const dataValueTypes: PinValueType[] = [
  'string',
  'markdown',
  'text',
  'json',
  'array',
  'int',
  'float',
  'bool',
  'any',
  'file_ref',
  'wiki_ref',
  'ticket_ref',
  'artifact_ref',
]

const emit = defineEmits<{
  'update-config': [patch: Record<string, unknown>]
  'switch-mode': [mode: 'llm' | 'agent']
  'load-prompt-file': []
  'update-prompt-template': [value: string]
  'update-prompt-mode': [mode: string]
  'save-prompt-file': []
  'remove': []
  'close': []
  'open-sub-graph': [graphId: string]
  'update-label': [value: string]
}>()

function inputValue(event: Event) {
  return event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLSelectElement
    ? event.target.value
    : ''
}

function numberValue(event: Event) {
  return Number(inputValue(event))
}

function configString(node: TaskGraphNode, key: string) {
  const value = node.config[key]
  return typeof value === 'string' ? value : ''
}

function configNumber(node: TaskGraphNode, key: string) {
  const value = node.config[key]
  return typeof value === 'number' ? value : Number(value || 0)
}

function configRecord(node: TaskGraphNode, key: string): Record<string, unknown> {
  const value = node.config[key]
  return value && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}
}

const nodeJsonErrors = ref<Record<string, string>>({})

function configRecordString(node: TaskGraphNode, key: string, field: string) {
  const value = configRecord(node, key)[field]
  return typeof value === 'string' ? value : ''
}

function updateConfigRecord(key: string, patch: Record<string, unknown>) {
  emit('update-config', { [key]: { ...configRecord(props.node, key), ...patch } })
}

// ─── uses_resources helpers ──────────────────────────────────────────────────

const nodeUsesResources = computed<string[]>(() => {
  const raw = props.node.config.uses_resources
  return Array.isArray(raw) ? raw.filter((v): v is string => typeof v === 'string') : []
})

function toggleNodeResource(resId: string, checked: boolean) {
  const current = [...nodeUsesResources.value]
  if (checked && !current.includes(resId)) {
    current.push(resId)
  } else if (!checked) {
    const idx = current.indexOf(resId)
    if (idx >= 0) current.splice(idx, 1)
  }
  emit('update-config', { uses_resources: current })
}

function configJsonText(node: TaskGraphNode, key: string) {
  const value = node.config[key]
  if (value === undefined || value === null) return ''
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function configPreview(node: TaskGraphNode) {
  if (!node.config || Object.keys(node.config).length === 0) return ''
  try {
    return JSON.stringify(node.config, null, 2)
  } catch {
    return String(node.config)
  }
}

function nodeJsonErrorKey(node: TaskGraphNode, key: string) {
  return `${node.id}:${key}`
}

function nodeJsonError(node: TaskGraphNode, key: string) {
  return nodeJsonErrors.value[nodeJsonErrorKey(node, key)] ?? ''
}

function updateJsonConfigField(key: string, value: string) {
  const errorKey = nodeJsonErrorKey(props.node, key)
  if (!value.trim()) {
    emit('update-config', { [key]: null })
    const errors = { ...nodeJsonErrors.value }
    delete errors[errorKey]
    nodeJsonErrors.value = errors
    return
  }
  try {
    const parsed = JSON.parse(value) as unknown
    emit('update-config', { [key]: parsed })
    const errors = { ...nodeJsonErrors.value }
    delete errors[errorKey]
    nodeJsonErrors.value = errors
  } catch {
    nodeJsonErrors.value = { ...nodeJsonErrors.value, [errorKey]: t('taskGraphInvalidJson') }
  }
}

function llmOutputArtifactType(node: TaskGraphNode) {
  return String(
    configRecord(node, 'output_contract').artifact_type
      ?? configRecord(node, 'output').artifact_type
      ?? 'markdown',
  )
}

function shellArgsText(node: TaskGraphNode) {
  const args = node.config.args
  return Array.isArray(args) ? args.filter((item): item is string => typeof item === 'string').join('\n') : ''
}

function updateShellArgs(value: string) {
  emit('update-config', { args: value.split(/\r?\n/).map((item) => item.trim()).filter(Boolean) })
}

function shellExpectedExitCodesText(node: TaskGraphNode) {
  const codes = node.config.expected_exit_codes
  return Array.isArray(codes) ? codes.join(', ') : '0'
}

function updateShellExpectedExitCodes(value: string) {
  const codes = value
    .split(',')
    .map((item) => Number(item.trim()))
    .filter((item) => Number.isInteger(item))
  emit('update-config', { expected_exit_codes: codes.length > 0 ? codes : [0] })
}

function shellEnvText(node: TaskGraphNode) {
  return JSON.stringify(configRecord(node, 'env'), null, 2)
}

function updateShellEnv(value: string) {
  try {
    const parsed = JSON.parse(value) as unknown
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      emit('update-config', { env: parsed })
    }
  } catch {
    // Keep the last valid environment object while the user edits.
  }
}

function shellCapture(node: TaskGraphNode) {
  return configRecord(node, 'capture')
}

function updateShellCapture(patch: Record<string, unknown>) {
  emit('update-config', { capture: { ...shellCapture(props.node), ...patch } })
}

function inputReference(inputId: string) {
  return `{{inputs.${inputId}}}`
}

function subGraphBindings(node: TaskGraphNode): Record<string, unknown> {
  const raw = node.config.input_bindings
  return raw && typeof raw === 'object' && !Array.isArray(raw)
    ? raw as Record<string, unknown>
    : {}
}

function updateSubGraphBinding(key: string, value: string) {
  emit('update-config', { input_bindings: { ...subGraphBindings(props.node), [key]: value } })
}

async function loadSubGraphInputs() {
  const graphId = configString(props.node, 'graph_id')
  const graphScope = configString(props.node, 'graph_scope') || 'project'
  if (!graphId) return
  try {
    const resp = await fetch(`/api/projects/${props.project}/task-graphs/${graphScope}/${graphId}/inputs`)
    if (resp.ok) {
      const data = await resp.json()
      const inputs = data.inputs as Array<{ id: string; default?: unknown }> ?? []
      const currentBindings = subGraphBindings(props.node)
      const newBindings: Record<string, unknown> = {}
      for (const input of inputs) {
        newBindings[input.id] = currentBindings[input.id] ?? ''
      }
      emit('update-config', { input_bindings: newBindings })
    }
  } catch { /* ignore */ }
}

function onSubGraphSelect(event: Event) {
  const graphId = inputValue(event)
  if (!graphId) {
    emit('update-config', { graph_id: '', graph_scope: 'project' })
    return
  }
  const matched = props.availableGraphs.find(g => g.id === graphId)
  const scope = matched?.scope ?? 'project'
  emit('update-config', { graph_id: graphId, graph_scope: scope })
}

function openSubGraph() {
  const graphId = configString(props.node, 'graph_id')
  if (graphId) emit('open-sub-graph', graphId)
}

function bindSelectedLoopIterations(inputId: string) {
  emit('update-config', { max_iterations_ref: inputReference(inputId) })
}

function llmInputs(node: TaskGraphNode) {
  const raw = node.config.inputs
  return raw && typeof raw === 'object' && !Array.isArray(raw)
    ? raw as Record<string, unknown>
    : {}
}

function renameLlmInput(oldKey: string, newKey: string) {
  if (!newKey || oldKey === newKey) return
  const inputs = { ...llmInputs(props.node) }
  const value = inputs[oldKey]
  delete inputs[oldKey]
  inputs[newKey] = value ?? ''
  emit('update-config', { inputs })
}

function updateLlmInput(key: string, value: string) {
  emit('update-config', { inputs: { ...llmInputs(props.node), [key]: value } })
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

function dataValueText(node: TaskGraphNode) {
  const value = node.config.value
  if (typeof value === 'string') return value
  if (value === undefined || value === null) return ''
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function setDataValueFromInput(inputId: string) {
  emit('update-config', { value: inputReference(inputId) })
}

function coordinatorPolicy(node: TaskGraphNode): Record<string, unknown> {
  return configRecord(node, 'coordinator')
}

function updateCoordinatorPolicy(patch: Record<string, unknown>) {
  emit('update-config', { coordinator: { ...coordinatorPolicy(props.node), ...patch } })
}

function coordinatorAllowedTypesText(node: TaskGraphNode) {
  const raw = coordinatorPolicy(node).allowed_node_types
  return Array.isArray(raw) ? raw.filter((item): item is string => typeof item === 'string').join(', ') : ''
}

function updateCoordinatorAllowedTypes(value: string) {
  const allowed_node_types = value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean)
  updateCoordinatorPolicy({ allowed_node_types })
}

function inputBindingString(node: TaskGraphNode, key: string) {
  const value = llmInputs(node)[key]
  return typeof value === 'string' ? value : ''
}

function updateIntentInput(key: string, value: string) {
  emit('update-config', { inputs: { ...llmInputs(props.node), [key]: value } })
}

function mutationGenerated(node: TaskGraphNode): Record<string, unknown> {
  return configRecord(node, 'generated')
}

function mutationGeneratedString(node: TaskGraphNode, key: string) {
  const value = mutationGenerated(node)[key]
  return typeof value === 'string' ? value : ''
}

function updateMutationGenerated(patch: Record<string, unknown>) {
  emit('update-config', { generated: { ...mutationGenerated(props.node), ...patch } })
}
</script>

<template>
  <section class="task-graph-node-card task-graph-editor-overlay task-graph-editor-node-overlay" :class="{ 'task-graph-node-readonly': readonly }">
    <header>
      <h4>{{ t('taskGraphInspectorNode') }}</h4>
      <div class="task-graph-node-card-actions">
        <BbButton
          class="task-graph-node-icon-button"
          size="mini"
          variant="secondary"
          icon-only
          :title="t('taskGraphEditorCloseConfig')"
          :aria-label="t('taskGraphEditorCloseConfig')"
          @click="emit('close')"
        >
          <PanelRightClose aria-hidden="true" />
        </BbButton>
        <BbButton
          size="mini"
          variant="danger"
          :title="t('taskGraphDeleteSelectedNode')"
          :disabled="readonly"
          @click="emit('remove')"
        >
          <template #leading>
            <Trash2 aria-hidden="true" />
          </template>
          {{ t('taskGraphDeleteNode') }}
        </BbButton>
      </div>
    </header>
    <BbField :label="t('ticketTableId')">
      <input :value="node.id" disabled />
    </BbField>
    <BbField :label="t('label')">
      <input :value="node.label" @input="emit('update-label', inputValue($event))" />
    </BbField>
    <BbField :label="t('agentKind')">
      <input :value="node.type" disabled />
    </BbField>

    <template v-if="node.type === 'start'">
      <section class="task-graph-node-section">
        <div class="task-graph-node-section-head">
          <strong>{{ t('taskGraphNodeConfigSection') }}</strong>
          <span>{{ t('taskGraphNodeTypeStart') }}</span>
        </div>
        <p class="task-graph-node-section-note">{{ t('taskGraphNodeStartEmpty') }}</p>
      </section>
    </template>

    <template v-else-if="node.type === 'input_var'">
      <BbField :label="t('taskGraphNodeInputId')">
        <select :value="configString(node, 'input_id')" @change="emit('update-config', { input_id: inputValue($event) })">
          <option value="">{{ t('taskGraphEditorNoInputs') }}</option>
          <option v-for="input in graphInputs" :key="input.id" :value="input.id">
            {{ input.label || input.id }} · {{ inputReference(input.id) }}
          </option>
        </select>
      </BbField>
      <div v-if="graphInputIds.length > 0" class="task-graph-variable-list">
        <BbRefChip
          v-for="inputId in graphInputIds"
          :key="inputId"
          interactive
          :disabled="readonly"
          @click="emit('update-config', { input_id: inputId })"
        >
          {{ inputReference(inputId) }}
        </BbRefChip>
      </div>
    </template>

    <template v-else-if="node.type === 'llm'">
      <TaskGraphLlmNodeForm
        :node="node"
        :readonly="readonly"
        :project-agents="projectAgents"
        :project-agents-error="projectAgentsError"
        :prompt-file-content="promptFileContent"
        :graph-scope="graphScope"
        :graph-id="graphId"
        :project="project"
        @update-config="emit('update-config', $event)"
        @switch-mode="emit('switch-mode', $event)"
        @load-prompt-file="emit('load-prompt-file')"
        @update-prompt-template="emit('update-prompt-template', $event)"
        @update-prompt-mode="emit('update-prompt-mode', $event)"
        @save-prompt-file="emit('save-prompt-file')"
      />
      <BbField :label="t('taskGraphNodeOutputType')">
        <input :value="llmOutputArtifactType(node)" disabled />
      </BbField>
      <TaskGraphBindingList
        :title="t('taskGraphInputs')"
        :bindings="llmInputs(node)"
        :readonly="readonly"
        :input-ids="graphInputIds"
        :value-placeholder="t('taskGraphInputBindingPlaceholder')"
        :add-label="t('taskGraphAddInput')"
        @rename="renameLlmInput"
        @update="updateLlmInput"
        @remove="removeLlmInput"
        @add="addLlmInput"
        @use-input="(key, inputId) => updateLlmInput(key, inputReference(inputId))"
      />
    </template>

    <template v-else-if="node.type === 'data_value'">
      <BbField :label="t('taskGraphNodeValueType')">
        <select :value="configString(node, 'value_type') || 'string'" @change="emit('update-config', { value_type: inputValue($event) })">
          <option v-for="type in dataValueTypes" :key="type" :value="type">{{ type }}</option>
        </select>
      </BbField>
      <BbField :label="t('taskGraphNodeValue')">
        <textarea
          :value="dataValueText(node)"
          :placeholder="inputReference('intent')"
          @input="emit('update-config', { value: inputValue($event) })"
        />
      </BbField>
      <div v-if="graphInputIds.length > 0" class="task-graph-variable-list">
        <BbRefChip
          v-for="inputId in graphInputIds"
          :key="inputId"
          interactive
          :disabled="readonly"
          @click="setDataValueFromInput(inputId)"
        >
          {{ inputReference(inputId) }}
        </BbRefChip>
      </div>
    </template>

    <template v-else-if="node.type === 'llm_coordinator'">
      <TaskGraphLlmNodeForm
        :node="node"
        :readonly="readonly"
        :project-agents="projectAgents"
        :project-agents-error="projectAgentsError"
        :prompt-file-content="promptFileContent"
        :graph-scope="graphScope"
        :graph-id="graphId"
        :project="project"
        @update-config="emit('update-config', $event)"
        @switch-mode="emit('switch-mode', $event)"
        @load-prompt-file="emit('load-prompt-file')"
        @update-prompt-template="emit('update-prompt-template', $event)"
        @update-prompt-mode="emit('update-prompt-mode', $event)"
        @save-prompt-file="emit('save-prompt-file')"
      />
      <BbField :label="t('taskGraphNodeMaxNodes')">
        <input
          type="number"
          min="1"
          :value="Number(coordinatorPolicy(node).max_nodes ?? 32)"
          @input="updateCoordinatorPolicy({ max_nodes: numberValue($event) })"
        />
      </BbField>
      <BbField :label="t('taskGraphNodeMaxEdges')">
        <input
          type="number"
          min="1"
          :value="Number(coordinatorPolicy(node).max_edges ?? 64)"
          @input="updateCoordinatorPolicy({ max_edges: numberValue($event) })"
        />
      </BbField>
      <BbField :label="t('taskGraphNodeAllowedNodeTypes')">
        <input
          :value="coordinatorAllowedTypesText(node)"
          :placeholder="t('taskGraphNodeAllowedNodeTypes')"
          @input="updateCoordinatorAllowedTypes(inputValue($event))"
        />
      </BbField>
      <TaskGraphBindingList
        :title="t('taskGraphInputs')"
        :bindings="llmInputs(node)"
        :readonly="readonly"
        :input-ids="graphInputIds"
        :value-placeholder="t('taskGraphInputBindingPlaceholder')"
        :add-label="t('taskGraphAddInput')"
        @rename="renameLlmInput"
        @update="updateLlmInput"
        @remove="removeLlmInput"
        @add="addLlmInput"
        @use-input="(key, inputId) => updateLlmInput(key, inputReference(inputId))"
      />
      <!-- uses_resources multi-select -->
      <div v-if="graphResourceIds.length > 0" class="task-graph-uses-resources">
        <label class="bb-field-label">Uses Resources</label>
        <div v-for="resId in graphResourceIds" :key="resId" class="task-graph-resource-checkbox">
          <input
            type="checkbox"
            :checked="nodeUsesResources.includes(resId)"
            :disabled="readonly"
            @change="toggleNodeResource(resId, ($event.target as HTMLInputElement).checked)"
          />
          <span>{{ resId }}</span>
        </div>
      </div>
    </template>

    <template v-else-if="node.type === 'llm_mutation'">
      <TaskGraphLlmNodeForm
        :node="node"
        :readonly="readonly"
        :project-agents="projectAgents"
        :project-agents-error="projectAgentsError"
        :prompt-file-content="promptFileContent"
        :graph-scope="graphScope"
        :graph-id="graphId"
        :project="project"
        @update-config="emit('update-config', $event)"
        @switch-mode="emit('switch-mode', $event)"
        @load-prompt-file="emit('load-prompt-file')"
        @update-prompt-template="emit('update-prompt-template', $event)"
        @update-prompt-mode="emit('update-prompt-mode', $event)"
        @save-prompt-file="emit('save-prompt-file')"
      />
      <BbField :label="t('taskGraphNodeMutationMode')">
        <input :value="configString(node, 'mode') || 'fanout'" disabled />
      </BbField>
      <BbField :label="t('taskGraphNodeTargetNodeId')">
        <input :value="configString(node, 'target_node_id')" :placeholder="t('taskGraphConnectMutation')" @input="emit('update-config', { target_node_id: inputValue($event) })" />
      </BbField>
      <BbField :label="t('taskGraphNodeGeneratedOutput')">
        <select :value="mutationGeneratedString(node, 'output_artifact_type') || 'markdown'" @change="updateMutationGenerated({ output_artifact_type: inputValue($event) })">
          <option value="markdown">markdown</option>
          <option value="json">json</option>
          <option value="text">text</option>
        </select>
      </BbField>
      <BbField :label="t('taskGraphNodeGeneratedScoutPrompt')">
        <textarea
          :value="mutationGeneratedString(node, 'prompt_template')"
          :placeholder="t('taskGraphScoutPromptTemplate')"
          @input="updateMutationGenerated({ prompt_template: inputValue($event) })"
        />
      </BbField>
    </template>

    <template v-else-if="node.type === 'intent_extract'">
      <BbField :label="t('taskGraphMode')">
        <select :value="configString(node, 'mode') || 'intent_gate'" @change="emit('update-config', { mode: inputValue($event) })">
          <option value="intent_gate">intent_gate</option>
          <option value="kb_wiki">kb_wiki</option>
        </select>
      </BbField>
      <BbField :label="t('taskGraphRequest')">
        <input
          :value="inputBindingString(node, 'request')"
          :placeholder="inputReference('intent')"
          @input="updateIntentInput('request', inputValue($event))"
        />
      </BbField>
      <div v-if="graphInputIds.length > 0" class="task-graph-variable-list">
        <BbRefChip
          v-for="inputId in graphInputIds"
          :key="inputId"
          interactive
          :disabled="readonly"
          @click="updateIntentInput('request', inputReference(inputId))"
        >
          {{ inputReference(inputId) }}
        </BbRefChip>
      </div>
      <BbField :label="t('taskGraphLanguage')">
        <input :value="configString(node, 'language') || 'zh-CN'" @input="emit('update-config', { language: inputValue($event) || 'zh-CN' })" />
      </BbField>
    </template>

    <template v-else-if="node.type === 'plan'">
      <TaskGraphBindingList
        :title="t('taskGraphInputs')"
        :bindings="llmInputs(node)"
        :readonly="readonly"
        :input-ids="graphInputIds"
        :value-placeholder="t('taskGraphInputBindingPlaceholder')"
        :add-label="t('taskGraphAddInput')"
        @rename="renameLlmInput"
        @update="updateLlmInput"
        @remove="removeLlmInput"
        @add="addLlmInput"
        @use-input="(key, inputId) => updateLlmInput(key, inputReference(inputId))"
      />
      <BbField :label="t('taskGraphNodePlanOutputArtifactPath')">
        <input
          :value="configRecordString(node, 'output', 'artifact_path')"
          @input="updateConfigRecord('output', { artifact_path: inputValue($event) })"
        />
      </BbField>
      <BbField :label="t('taskGraphNodePlanOutputSchemaName')">
        <input
          :value="configRecordString(node, 'output', 'schema_name')"
          @input="updateConfigRecord('output', { schema_name: inputValue($event) })"
        />
      </BbField>
      <section class="task-graph-node-section">
        <div class="task-graph-node-section-head">
          <strong>{{ t('taskGraphNodeConfigPreview') }}</strong>
          <span>{{ t('taskGraphNodeTypePlan') }}</span>
        </div>
        <pre v-if="configPreview(node)" class="task-graph-node-config-preview">{{ configPreview(node) }}</pre>
        <p v-else class="task-graph-node-section-note">{{ t('taskGraphNodeConfigEmpty') }}</p>
      </section>
    </template>

    <template v-else-if="node.type === 'kb_plan'">
      <BbField :label="t('taskGraphMode')">
        <select :value="configString(node, 'mode') || 'wiki_plan'" @change="emit('update-config', { mode: inputValue($event) })">
          <option value="wiki_plan">wiki_plan</option>
          <option value="writer_plan">writer_plan</option>
        </select>
      </BbField>
      <TaskGraphBindingList
        :title="t('taskGraphInputs')"
        :bindings="llmInputs(node)"
        :readonly="readonly"
        :input-ids="graphInputIds"
        :value-placeholder="t('taskGraphInputBindingPlaceholder')"
        :add-label="t('taskGraphAddInput')"
        @rename="renameLlmInput"
        @update="updateLlmInput"
        @remove="removeLlmInput"
        @add="addLlmInput"
        @use-input="(key, inputId) => updateLlmInput(key, inputReference(inputId))"
      />
    </template>

    <template v-else-if="node.type === 'manifest_merge'">
      <TaskGraphBindingList
        :title="t('taskGraphInputs')"
        :bindings="llmInputs(node)"
        :readonly="readonly"
        :input-ids="graphInputIds"
        :value-placeholder="t('taskGraphInputBindingPlaceholder')"
        :add-label="t('taskGraphAddInput')"
        @rename="renameLlmInput"
        @update="updateLlmInput"
        @remove="removeLlmInput"
        @add="addLlmInput"
        @use-input="(key, inputId) => updateLlmInput(key, inputReference(inputId))"
      />
      <section class="task-graph-node-section">
        <div class="task-graph-node-section-head">
          <strong>{{ t('taskGraphNodeConfigSection') }}</strong>
          <span>{{ t('taskGraphNodeTypeManifestMerge') }}</span>
        </div>
        <p class="task-graph-node-section-note">{{ t('taskGraphNodeGenericConfigHint') }}</p>
        <pre v-if="configPreview(node)" class="task-graph-node-config-preview">{{ configPreview(node) }}</pre>
      </section>
    </template>

    <template v-else-if="node.type === 'schema_validate'">
      <TaskGraphBindingList
        :title="t('taskGraphInputs')"
        :bindings="llmInputs(node)"
        :readonly="readonly"
        :input-ids="graphInputIds"
        :value-placeholder="t('taskGraphInputBindingPlaceholder')"
        :add-label="t('taskGraphAddInput')"
        @rename="renameLlmInput"
        @update="updateLlmInput"
        @remove="removeLlmInput"
        @add="addLlmInput"
        @use-input="(key, inputId) => updateLlmInput(key, inputReference(inputId))"
      />
      <BbField :label="t('taskGraphSchemaValidateValueKey')">
        <input :value="configString(node, 'value_key') || 'value'" @input="emit('update-config', { value_key: inputValue($event) || 'value' })" />
      </BbField>
      <BbCheckboxField
        :label="t('taskGraphSchemaValidateFailOnInvalid')"
        :checked="node.config.fail_on_invalid === true"
        @change="(checked) => emit('update-config', { fail_on_invalid: checked })"
      />
      <BbField :label="t('taskGraphSchemaValidateSchema')">
        <textarea :value="configJsonText(node, 'schema')" @change="updateJsonConfigField('schema', inputValue($event))" />
      </BbField>
      <p v-if="nodeJsonError(node, 'schema')" class="task-graph-node-section-note">{{ nodeJsonError(node, 'schema') }}</p>
      <BbField :label="t('taskGraphSchemaValidateSourceJson')">
        <textarea :value="configJsonText(node, 'source')" @change="updateJsonConfigField('source', inputValue($event))" />
      </BbField>
      <p v-if="nodeJsonError(node, 'source')" class="task-graph-node-section-note">{{ nodeJsonError(node, 'source') }}</p>
      <BbField :label="t('taskGraphSchemaValidateRepairJson')">
        <textarea :value="configJsonText(node, 'repair')" @change="updateJsonConfigField('repair', inputValue($event))" />
      </BbField>
      <p v-if="nodeJsonError(node, 'repair')" class="task-graph-node-section-note">{{ nodeJsonError(node, 'repair') }}</p>
    </template>

    <template v-else-if="node.type === 'system_write_output'">
      <TaskGraphBindingList
        :title="t('taskGraphInputs')"
        :bindings="llmInputs(node)"
        :readonly="readonly"
        :input-ids="graphInputIds"
        :value-placeholder="t('taskGraphInputBindingPlaceholder')"
        :add-label="t('taskGraphAddInput')"
        @rename="renameLlmInput"
        @update="updateLlmInput"
        @remove="removeLlmInput"
        @add="addLlmInput"
        @use-input="(key, inputId) => updateLlmInput(key, inputReference(inputId))"
      />
      <BbField :label="t('taskGraphSystemWriteOutputPath')">
        <input :value="configString(node, 'output_path')" @input="emit('update-config', { output_path: inputValue($event) })" />
      </BbField>
      <BbField :label="t('taskGraphSystemWriteArtifactType')">
        <select :value="configString(node, 'artifact_type') || 'markdown'" @change="emit('update-config', { artifact_type: inputValue($event) })">
          <option value="markdown">markdown</option>
          <option value="json">json</option>
          <option value="text">text</option>
        </select>
      </BbField>
      <BbField :label="t('taskGraphSystemWriteContent')">
        <textarea :value="configString(node, 'content') || '{{inputs.content}}'" @input="emit('update-config', { content: inputValue($event) })" />
      </BbField>
      <BbCheckboxField
        :label="t('taskGraphSystemWriteCreateParentDirs')"
        :checked="node.config.create_parent_dirs !== false"
        @change="(checked) => emit('update-config', { create_parent_dirs: checked })"
      />
      <BbCheckboxField
        :label="t('taskGraphSystemWriteOverwrite')"
        :checked="node.config.overwrite !== false"
        @change="(checked) => emit('update-config', { overwrite: checked })"
      />
    </template>

    <template v-else-if="node.type === 'shell'">
      <BbField :label="t('taskGraphLlmNodeCommand')">
        <input :value="configString(node, 'command')" @input="emit('update-config', { command: inputValue($event) })" />
      </BbField>
      <BbField :label="t('taskGraphLlmNodeArgs')">
        <textarea :value="shellArgsText(node)" @input="updateShellArgs(inputValue($event))" />
      </BbField>
      <BbField :label="t('taskGraphNodeCwd')">
        <input :value="configString(node, 'cwd') || '.'" @input="emit('update-config', { cwd: inputValue($event) || '.' })" />
      </BbField>
      <BbField :label="t('taskGraphNodePermission')">
        <select :value="configString(node, 'permission') || 'read_only'" @change="emit('update-config', { permission: inputValue($event) })">
          <option value="read_only">read_only</option>
          <option value="project_write">project_write</option>
          <option value="git_write">git_write</option>
          <option value="network">network</option>
        </select>
      </BbField>
      <BbField :label="t('taskGraphNodeTimeoutMs')">
        <input type="number" min="1" :value="configNumber(node, 'timeout_ms') || 600000" @input="emit('update-config', { timeout_ms: numberValue($event) })" />
      </BbField>
      <BbField :label="t('taskGraphNodeExpectedExitCodes')">
        <input :value="shellExpectedExitCodesText(node)" @input="updateShellExpectedExitCodes(inputValue($event))" />
      </BbField>
      <BbField :label="t('taskGraphLlmNodeEnvJson')">
        <textarea :value="shellEnvText(node)" @change="updateShellEnv(inputValue($event))" />
      </BbField>
      <BbField :label="t('taskGraphNodeCaptureMaxBytes')">
        <input
          type="number"
          min="1"
          :value="Number(shellCapture(node).max_bytes ?? 1048576)"
          @input="updateShellCapture({ max_bytes: numberValue($event) })"
        />
      </BbField>
      <BbCheckboxField
        :label="t('taskGraphNodeStripAnsi')"
        :checked="shellCapture(node).strip_ansi !== false"
        @change="(checked) => updateShellCapture({ strip_ansi: checked })"
      />
    </template>

    <template v-else-if="node.type === 'sub_graph'">
      <BbField :label="t('pipeline') || 'graph_id'">
        <select :value="configString(node, 'graph_id')" @change="onSubGraphSelect($event)">
          <option value="">{{ t('taskGraphSelectSubPipeline') }}</option>
          <option v-for="g in availableGraphs" :key="g.id" :value="g.id">
            [{{ g.scope }}] {{ g.title }} ({{ g.id }})
          </option>
        </select>
      </BbField>
      <BbField :label="t('taskGraphNodeGraphScope')">
        <input type="text" :value="configString(node, 'graph_scope') || 'project'" disabled />
      </BbField>
      <TaskGraphBindingList
        :title="t('taskGraphInputBindings')"
        :bindings="subGraphBindings(node)"
        :readonly="readonly"
        readonly-keys
        :removable="false"
        :input-ids="graphInputIds"
        :value-placeholder="t('taskGraphInputBindingPlaceholder')"
        :add-label="t('taskGraphRefreshSubPipelineInputs')"
        @update="updateSubGraphBinding"
        @add="loadSubGraphInputs"
        @use-input="(key, inputId) => updateSubGraphBinding(key, inputReference(inputId))"
      />
      <BbButton v-if="configString(node, 'graph_id')" class="task-graph-inline-action" size="sm" variant="secondary" @click="openSubGraph()">
        {{ t('taskGraphOpenSubPipeline') }}
      </BbButton>
    </template>

    <template v-else-if="node.type === 'human_gate'">
      <BbField :label="t('taskGraphNodeTitle')">
        <input :value="configString(node, 'title')" @input="emit('update-config', { title: inputValue($event) })" />
      </BbField>
      <BbField :label="t('taskGraphNodeInstructions')">
        <textarea :value="configString(node, 'instructions')" @input="emit('update-config', { instructions: inputValue($event) })" />
      </BbField>
    </template>

    <template v-else-if="node.type === 'branch'">
      <TaskGraphBranchNodeForm
        :node="node"
        :readonly="readonly"
        :graph-inputs="graphInputs"
        @update-config="emit('update-config', $event)"
      />
    </template>

    <template v-else-if="node.type === 'loop'">
      <BbField :label="t('taskGraphNodeMaxIterations')">
        <input type="number" min="1" max="10" :value="configNumber(node, 'max_iterations')" @input="emit('update-config', { max_iterations: numberValue($event) })" />
      </BbField>
      <BbField :label="t('taskGraphNodeMaxIterationsRef')">
        <select :value="configString(node, 'max_iterations_ref')" @change="emit('update-config', { max_iterations_ref: inputValue($event) })">
          <option value="">{{ t('taskGraphFallbackNumber') }}</option>
          <option v-for="input in graphInputs.filter((item) => item.type === 'number')" :key="input.id" :value="inputReference(input.id)">
            {{ input.label || input.id }} · {{ inputReference(input.id) }}
          </option>
        </select>
      </BbField>
      <div v-if="graphInputs.some((item) => item.type === 'number')" class="task-graph-variable-list">
        <BbRefChip
          v-for="input in graphInputs.filter((item) => item.type === 'number')"
          :key="input.id"
          interactive
          :disabled="readonly"
          @click="bindSelectedLoopIterations(input.id)"
        >
          {{ inputReference(input.id) }}
        </BbRefChip>
      </div>
      <BbField :label="t('taskGraphNodeBodyEntry')">
        <input :value="configString(node, 'body_entry')" @input="emit('update-config', { body_entry: inputValue($event) })" />
      </BbField>
      <BbField :label="t('taskGraphNodeBodyExit')">
        <input :value="configString(node, 'body_exit')" @input="emit('update-config', { body_exit: inputValue($event) })" />
      </BbField>
      <BbField :label="t('taskGraphNodeOnMaxIterations')">
        <select :value="configString(node, 'on_max_iterations')" @change="emit('update-config', { on_max_iterations: inputValue($event) })">
          <option value="fail">{{ t('taskGraphOnMaxIterationsFail') }}</option>
          <option value="cancel">{{ t('taskGraphOnMaxIterationsCancel') }}</option>
          <option value="succeed">{{ t('taskGraphOnMaxIterationsSucceed') }}</option>
        </select>
      </BbField>
    </template>

    <template v-else-if="node.type === 'end'">
      <BbField :label="t('taskGraphNodeResult')">
        <select :value="configString(node, 'result')" @change="emit('update-config', { result: inputValue($event) })">
          <option value="succeeded">{{ t('taskGraphResultSucceeded') }}</option>
          <option value="failed">{{ t('taskGraphResultFailed') }}</option>
          <option value="cancelled">{{ t('taskGraphResultCancelled') }}</option>
        </select>
      </BbField>
    </template>

    <template v-else>
      <section class="task-graph-node-section">
        <div class="task-graph-node-section-head">
          <strong>{{ t('taskGraphNodeConfigSection') }}</strong>
          <span>{{ node.type }}</span>
        </div>
        <p class="task-graph-node-section-note">{{ t('taskGraphNodeGenericConfigHint') }}</p>
        <pre v-if="configPreview(node)" class="task-graph-node-config-preview">{{ configPreview(node) }}</pre>
        <p v-else class="task-graph-node-section-note">{{ t('taskGraphNodeConfigEmpty') }}</p>
      </section>
    </template>

    <ul v-if="selectedNodeErrors.length > 0" class="task-graph-error-list">
      <li v-for="(err, index) in selectedNodeErrors" :key="index">{{ err.message }}</li>
    </ul>
  </section>
</template>

<style scoped>
.task-graph-node-card {
  display: grid;
  align-content: start;
  gap: 9px;
  min-width: 0;
  padding: 10px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface-soft);
}

.task-graph-node-card h4 {
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 12px;
  font-weight: 760;
}

.task-graph-node-card header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.task-graph-node-card-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}

.task-graph-node-card header button:not(.bb-button) {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-height: 28px;
  padding: 0 8px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-muted);
  cursor: pointer;
}

.task-graph-node-icon-button {
  width: 28px;
  padding: 0;
}

.task-graph-node-card header button:not(.bb-button) svg {
  width: 14px;
  height: 14px;
}

.task-graph-node-delete-button {
  padding: 0 8px;
}

.task-graph-node-delete-button span {
  font-size: 11px;
  font-weight: 760;
}

.task-graph-node-section {
  display: grid;
  gap: 7px;
  min-width: 0;
  padding: 8px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 10px;
  background: var(--bb-surface);
}

.task-graph-node-section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-width: 0;
}

.task-graph-node-section-head strong {
  color: var(--bb-text-strong);
  font-size: 12px;
  font-weight: 760;
}

.task-graph-node-section-head span,
.task-graph-node-section-note {
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 11px;
  overflow-wrap: anywhere;
}

.task-graph-node-config-preview {
  max-height: 220px;
  margin: 0;
  padding: 8px;
  overflow: auto;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface-soft);
  color: var(--bb-text-strong);
  font-size: 11px;
  line-height: 1.45;
  white-space: pre-wrap;
}

.task-graph-node-readonly input,
.task-graph-node-readonly select,
.task-graph-node-readonly textarea,
.task-graph-node-readonly button:not(.task-graph-node-icon-button) {
  pointer-events: none;
  opacity: 0.7;
}

.task-graph-variable-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.task-graph-inline-action {
  justify-self: start;
}

.task-graph-error-list {
  display: grid;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
  color: var(--bb-error);
  font-size: 12px;
  line-height: 1.35;
}
</style>
