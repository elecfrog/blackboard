<script setup lang="ts">
import { Trash2 } from 'lucide-vue-next'
import { t } from '@/i18n'
import TaskGraphLlmNodeForm from './TaskGraphLlmNodeForm.vue'
import TaskGraphBranchNodeForm from './TaskGraphBranchNodeForm.vue'
import TaskGraphBindingList from './TaskGraphBindingList.vue'
import {
  type TaskGraphNode,
  type TaskGraphInputParam,
} from '@/data/taskGraphs'
import { type ProjectAgentProfile, type McpServerConfig } from '@/data/agents'

const props = defineProps<{
  node: TaskGraphNode
  readonly: boolean
  projectAgents: ProjectAgentProfile[]
  projectAgentsError: string
  graphInputs: TaskGraphInputParam[]
  graphInputIds: string[]
  availableGraphs: Array<{ id: string; scope: string; title: string }>
  project: string
  graphScope: string
  graphId: string
  selectedNodeErrors: Array<{ message: string }>
  promptFileContent: string
}>()

const emit = defineEmits<{
  'update-config': [patch: Record<string, unknown>]
  'switch-mode': [mode: 'llm' | 'agent']
  'load-prompt-file': []
  'update-prompt-template': [value: string]
  'update-prompt-mode': [mode: string]
  'save-prompt-file': []
  'remove': []
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
</script>

<template>
  <section class="task-graph-node-card task-graph-editor-overlay task-graph-editor-node-overlay" :class="{ 'task-graph-node-readonly': readonly }">
    <header>
      <h4>{{ t('taskGraphInspectorNode') }}</h4>
      <button
        type="button"
        class="task-graph-node-delete-button"
        :title="t('taskGraphDeleteSelectedNode')"
        :disabled="readonly"
        @click="emit('remove')"
      >
        <Trash2 aria-hidden="true" />
        <span>{{ t('taskGraphDeleteNode') }}</span>
      </button>
    </header>
    <label>
      <span>ID</span>
      <input :value="node.id" disabled />
    </label>
    <label>
      <span>{{ t('label') }}</span>
      <input :value="node.label" @input="emit('update-label', inputValue($event))" />
    </label>
    <label>
      <span>{{ t('agentKind') }}</span>
      <input :value="node.type" disabled />
    </label>

    <template v-if="node.type === 'llm'">
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
      <label>
        <span>output type</span>
        <input :value="String((node.config.output as Record<string, unknown> | undefined)?.artifact_type ?? 'markdown')" disabled />
      </label>
      <TaskGraphBindingList
        title="inputs"
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

    <template v-else-if="node.type === 'sub_graph'">
      <label>
        <span>{{ t('pipeline') || 'graph_id' }}</span>
        <select :value="configString(node, 'graph_id')" @change="onSubGraphSelect($event)">
          <option value="">{{ t('taskGraphSelectSubPipeline') }}</option>
          <option v-for="g in availableGraphs" :key="g.id" :value="g.id">
            [{{ g.scope }}] {{ g.title }} ({{ g.id }})
          </option>
        </select>
      </label>
      <label>
        <span>graph_scope</span>
        <input type="text" :value="configString(node, 'graph_scope') || 'project'" disabled />
      </label>
      <TaskGraphBindingList
        title="input_bindings"
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
      <button v-if="configString(node, 'graph_id')" type="button" class="task-graph-inline-add" @click="openSubGraph()">
        {{ t('taskGraphOpenSubPipeline') }}
      </button>
    </template>

    <template v-else-if="node.type === 'human_gate'">
      <label>
        <span>title</span>
        <input :value="configString(node, 'title')" @input="emit('update-config', { title: inputValue($event) })" />
      </label>
      <label>
        <span>instructions</span>
        <textarea :value="configString(node, 'instructions')" @input="emit('update-config', { instructions: inputValue($event) })" />
      </label>
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
      <label>
        <span>max_iterations</span>
        <input type="number" min="1" max="10" :value="configNumber(node, 'max_iterations')" @input="emit('update-config', { max_iterations: numberValue($event) })" />
      </label>
      <label>
        <span>max_iterations_ref</span>
        <select :value="configString(node, 'max_iterations_ref')" @change="emit('update-config', { max_iterations_ref: inputValue($event) })">
          <option value="">{{ t('taskGraphFallbackNumber') }}</option>
          <option v-for="input in graphInputs.filter((item) => item.type === 'number')" :key="input.id" :value="inputReference(input.id)">
            {{ input.label || input.id }} · {{ inputReference(input.id) }}
          </option>
        </select>
      </label>
      <div v-if="graphInputs.some((item) => item.type === 'number')" class="task-graph-variable-list">
        <button
          v-for="input in graphInputs.filter((item) => item.type === 'number')"
          :key="input.id"
          type="button"
          @click="bindSelectedLoopIterations(input.id)"
        >
          {{ inputReference(input.id) }}
        </button>
      </div>
      <label>
        <span>body_entry</span>
        <input :value="configString(node, 'body_entry')" @input="emit('update-config', { body_entry: inputValue($event) })" />
      </label>
      <label>
        <span>body_exit</span>
        <input :value="configString(node, 'body_exit')" @input="emit('update-config', { body_exit: inputValue($event) })" />
      </label>
      <label>
        <span>on_max_iterations</span>
        <select :value="configString(node, 'on_max_iterations')" @change="emit('update-config', { on_max_iterations: inputValue($event) })">
          <option value="fail">{{ t('taskGraphOnMaxIterationsFail') }}</option>
          <option value="cancel">{{ t('taskGraphOnMaxIterationsCancel') }}</option>
          <option value="succeed">{{ t('taskGraphOnMaxIterationsSucceed') }}</option>
        </select>
      </label>
    </template>

    <template v-else-if="node.type === 'end'">
      <label>
        <span>result</span>
        <select :value="configString(node, 'result')" @change="emit('update-config', { result: inputValue($event) })">
          <option value="succeeded">{{ t('taskGraphResultSucceeded') }}</option>
          <option value="failed">{{ t('taskGraphResultFailed') }}</option>
          <option value="cancelled">{{ t('taskGraphResultCancelled') }}</option>
        </select>
      </label>
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

.task-graph-node-card header button {
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

.task-graph-node-card header button svg {
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

.task-graph-node-card label {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.task-graph-node-card label span {
  color: var(--bb-text-muted);
  font-size: 11px;
  font-weight: 760;
}

.task-graph-node-card input,
.task-graph-node-card select,
.task-graph-node-card textarea {
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

.task-graph-node-card textarea {
  min-height: 80px;
  resize: vertical;
}

.task-graph-node-readonly input,
.task-graph-node-readonly select,
.task-graph-node-readonly textarea,
.task-graph-node-readonly button {
  pointer-events: none;
  opacity: 0.7;
}

.task-graph-variable-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.task-graph-variable-list button {
  width: auto;
  cursor: pointer;
}

.task-graph-inline-add {
  justify-self: start;
  padding: 0 9px;
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
