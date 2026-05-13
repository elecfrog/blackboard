<script setup lang="ts">
import { Plus, Trash2 } from 'lucide-vue-next'
import { t } from '@/i18n'
import { type TaskGraphInputParam, type TaskGraphNode } from '@/data/taskGraphs'

interface BranchRule {
  id: string
  label: string
  when: {
    path?: string
    op: string
    value?: string | number | boolean
  }
}

type BranchRulePatch = Omit<Partial<BranchRule>, 'when'> & {
  when?: Partial<BranchRule['when']>
}

const props = defineProps<{
  node: TaskGraphNode
  readonly: boolean
  graphInputs: TaskGraphInputParam[]
}>()

const emit = defineEmits<{
  'update-config': [patch: Record<string, unknown>]
}>()

const branchOps = ['always', 'exists', 'equals', 'not_equals', '>', '>=', '<', '<=', 'contains', 'is_empty', 'not_empty', 'truthy', 'falsy']

function inputValue(event: Event) {
  return event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLSelectElement
    ? event.target.value
    : ''
}

function configString(node: TaskGraphNode, key: string) {
  const value = node.config[key]
  return typeof value === 'string' ? value : ''
}

function configNumber(node: TaskGraphNode, key: string) {
  const value = node.config[key]
  return typeof value === 'number' ? value : Number(value || 0)
}

function numberValue(event: Event) {
  return Number(inputValue(event))
}

function kebab(value: string, fallback: string) {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return normalized || fallback
}

function inputReference(inputId: string) {
  return `{{inputs.${inputId}}}`
}

function getBranchRules(node: TaskGraphNode): BranchRule[] {
  const rules = node.config.rules
  if (!Array.isArray(rules)) return []
  return rules.map((rule) => {
    const value = rule && typeof rule === 'object' ? rule as Record<string, unknown> : {}
    const when = value.when && typeof value.when === 'object' ? value.when as Record<string, unknown> : {}
    return {
      id: String(value.id ?? ''),
      label: String(value.label ?? value.id ?? ''),
      when: {
        path: typeof when.path === 'string' ? when.path : '',
        op: typeof when.op === 'string' ? when.op : 'always',
        value: typeof when.value === 'string' || typeof when.value === 'number' || typeof when.value === 'boolean' ? when.value : '',
      },
    }
  })
}

function setBranchRules(rules: BranchRule[]) {
  emit('update-config', { rules })
}

function updateBranchRule(index: number, patch: BranchRulePatch) {
  const rules = getBranchRules(props.node)
  const current = rules[index]
  if (!current) return
  rules[index] = {
    ...current,
    ...patch,
    when: {
      ...current.when,
      ...(patch.when ?? {}),
    },
  }
  setBranchRules(rules)
}

function addBranchRule() {
  const rules = getBranchRules(props.node)
  const id = kebab(`rule-${rules.length + 1}`, `rule-${rules.length + 1}`)
  setBranchRules([...rules, { id, label: t('taskGraphRuleWithIndex', { index: String(rules.length + 1) }), when: { op: 'always' } }])
}

function removeBranchRule(index: number) {
  const removed = getBranchRules(props.node)[index]
  const rules = getBranchRules(props.node).filter((_, itemIndex) => itemIndex !== index)
  const defaultRule = configString(props.node, 'default_rule_id')
  emit('update-config', {
    rules,
    default_rule_id: removed?.id === defaultRule ? rules[0]?.id ?? '' : defaultRule,
  })
}
</script>

<template>
  <div class="task-graph-branch-form">
    <label>
      <span>input_ref</span>
      <input :value="configString(node, 'input_ref')" @input="emit('update-config', { input_ref: inputValue($event) })" />
    </label>
    <label>
      <span>default_rule_id</span>
      <select :value="configString(node, 'default_rule_id')" @change="emit('update-config', { default_rule_id: inputValue($event) })">
        <option v-for="rule in getBranchRules(node)" :key="rule.id" :value="rule.id">{{ rule.id }}</option>
      </select>
    </label>
    <div class="task-graph-rule-list">
      <div v-for="(rule, index) in getBranchRules(node)" :key="`${rule.id}-${index}`" class="task-graph-rule-row">
        <input :value="rule.id" @input="updateBranchRule(index, { id: kebab(inputValue($event), `rule-${index + 1}`) })" />
        <input :value="rule.label" @input="updateBranchRule(index, { label: inputValue($event) })" />
        <select :value="rule.when.op" @change="updateBranchRule(index, { when: { op: inputValue($event) } })">
          <option v-for="op in branchOps" :key="op" :value="op">{{ op }}</option>
        </select>
        <input :value="rule.when.path ?? ''" placeholder="$.path" @input="updateBranchRule(index, { when: { path: inputValue($event) } })" />
        <input :value="rule.when.value ?? ''" placeholder="value" @input="updateBranchRule(index, { when: { value: inputValue($event) } })" />
        <button type="button" @click="removeBranchRule(index)">
          <Trash2 aria-hidden="true" />
        </button>
      </div>
      <button type="button" class="task-graph-inline-add" @click="addBranchRule">
        <Plus aria-hidden="true" />
        {{ t('taskGraphRuleAdd') }}
      </button>
    </div>
  </div>
</template>
