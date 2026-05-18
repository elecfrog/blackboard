<script setup lang="ts">
import { Plus, Trash2 } from 'lucide-vue-next'
import { BbButton, BbDenseRow, BbField } from '@/components/common'
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
    <BbField :label="t('taskGraphBranchInputRef')">
      <input :value="configString(node, 'input_ref')" @input="emit('update-config', { input_ref: inputValue($event) })" />
    </BbField>
    <BbField :label="t('taskGraphBranchDefaultRuleId')">
      <select :value="configString(node, 'default_rule_id')" @change="emit('update-config', { default_rule_id: inputValue($event) })">
        <option v-for="rule in getBranchRules(node)" :key="rule.id" :value="rule.id">{{ rule.id }}</option>
      </select>
    </BbField>
    <div class="task-graph-rule-list">
      <div class="task-graph-rule-head" aria-hidden="true">
        <span>{{ t('taskGraphBranchRuleId') }}</span>
        <span>{{ t('taskGraphBranchRuleLabel') }}</span>
        <span>{{ t('taskGraphBranchRuleOp') }}</span>
        <span>{{ t('taskGraphBranchRulePath') }}</span>
        <span>{{ t('taskGraphBranchRuleValue') }}</span>
      </div>
      <BbDenseRow v-for="(rule, index) in getBranchRules(node)" :key="`${rule.id}-${index}`" class="task-graph-rule-row">
        <input class="bb-dense-control" :value="rule.id" :aria-label="t('taskGraphBranchRuleId')" @input="updateBranchRule(index, { id: kebab(inputValue($event), `rule-${index + 1}`) })" />
        <input class="bb-dense-control" :value="rule.label" :aria-label="t('taskGraphBranchRuleLabel')" @input="updateBranchRule(index, { label: inputValue($event) })" />
        <select class="bb-dense-control" :value="rule.when.op" @change="updateBranchRule(index, { when: { op: inputValue($event) } })">
          <option v-for="op in branchOps" :key="op" :value="op">{{ op }}</option>
        </select>
        <input class="bb-dense-control" :value="rule.when.path ?? ''" :aria-label="t('taskGraphBranchRulePath')" :placeholder="t('taskGraphBranchPathPlaceholder')" @input="updateBranchRule(index, { when: { path: inputValue($event) } })" />
        <input class="bb-dense-control" :value="rule.when.value ?? ''" :aria-label="t('taskGraphBranchRuleValue')" :placeholder="t('taskGraphBranchRuleValue')" @input="updateBranchRule(index, { when: { value: inputValue($event) } })" />
        <BbButton
          class="task-graph-rule-remove"
          size="mini"
          variant="danger"
          icon-only
          :aria-label="t('taskGraphBindingRemove')"
          @click="removeBranchRule(index)"
        >
          <Trash2 aria-hidden="true" />
        </BbButton>
      </BbDenseRow>
      <BbButton class="task-graph-rule-add" size="sm" variant="secondary" @click="addBranchRule">
        <template #leading>
          <Plus aria-hidden="true" />
        </template>
        {{ t('taskGraphRuleAdd') }}
      </BbButton>
    </div>
  </div>
</template>

<style scoped>
.task-graph-branch-form {
  display: grid;
  gap: 9px;
  min-width: 0;
}

.task-graph-rule-head span {
  color: var(--bb-text-muted);
  font-size: 11px;
  font-weight: 760;
}

.task-graph-rule-list {
  --task-graph-rule-columns: minmax(84px, 0.9fr) minmax(92px, 1fr) minmax(88px, 0.9fr) minmax(92px, 1fr) minmax(92px, 1fr) 30px;
  display: grid;
  gap: 7px;
  min-width: 0;
}

.task-graph-rule-head {
  display: grid;
  grid-template-columns: var(--task-graph-rule-columns);
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.task-graph-rule-row {
  --bb-dense-row-columns: var(--task-graph-rule-columns);
  --bb-dense-row-border: var(--bb-border-warm-medium);
  --bb-dense-row-bg: var(--bb-surface-soft);
  --bb-dense-control-height: 32px;
}

.task-graph-rule-remove {
  --bb-icon-button-size: 30px;
}

.task-graph-rule-add {
  justify-self: start;
}

@media (max-width: 720px) {
  .task-graph-rule-head {
    display: none;
  }

  .task-graph-rule-row {
    --bb-dense-row-columns: minmax(0, 1fr) 30px;
  }
}
</style>
