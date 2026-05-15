import type { TaskGraphInputParam, TaskGraphNode, TaskGraphNodeCategory, TaskGraphNodeRole } from '@/data/taskGraphs'
import { t } from '@/i18n'

export interface TaskGraphNodeVisual {
  key: string
  type: TaskGraphNode['type']
  role?: TaskGraphNodeRole
  label: string
  color: string
  category: TaskGraphNodeCategory
  categoryLabel: string
  icon: string
  description: string
  defaultConfig?: Record<string, unknown>
}

export function taskGraphNodeCategoryLabels(): Record<TaskGraphNodeCategory, string> {
  return {
    terminal: t('taskGraphNodeCategoryTerminal'),
    control: t('taskGraphNodeCategoryControl'),
    runtime: t('taskGraphNodeCategoryRuntime'),
    transform: t('taskGraphNodeCategoryTransform'),
    artifact: t('taskGraphNodeCategoryArtifact'),
    integration: t('taskGraphNodeCategoryIntegration'),
    approval: t('taskGraphNodeCategoryApproval'),
  }
}

export function taskGraphNodeTypes(): TaskGraphNodeVisual[] {
  const categories = taskGraphNodeCategoryLabels()
  return [
    { key: 'start', type: 'start', label: t('taskGraphNodeTypeStart'), color: '#64748b', category: 'terminal', categoryLabel: categories.terminal, icon: 'S', description: t('taskGraphNodeDescStart') },
    businessRole('explorer_agent', 'llm', t('taskGraphNodeTypeExplorerAgent'), '#2563eb', categories.runtime, 'EX', t('taskGraphNodeDescExplorerAgent')),
    businessRole('implementer_agent', 'llm', t('taskGraphNodeTypeImplementerAgent'), '#0f766e', categories.runtime, 'IM', t('taskGraphNodeDescImplementerAgent')),
    businessRole('verifier_agent', 'llm', t('taskGraphNodeTypeVerifierAgent'), '#16a34a', categories.runtime, 'VE', t('taskGraphNodeDescVerifierAgent')),
    businessRole('reviewer_agent', 'llm', t('taskGraphNodeTypeReviewerAgent'), '#7c3aed', categories.runtime, 'RV', t('taskGraphNodeDescReviewerAgent')),
    businessRole('handoff_writer', 'llm', t('taskGraphNodeTypeHandoffWriter'), '#84cc16', categories.artifact, 'HW', t('taskGraphNodeDescHandoffWriter')),
    { key: 'llm', type: 'llm', label: t('taskGraphNodeTypeLlm'), color: '#2563eb', category: 'runtime', categoryLabel: categories.runtime, icon: 'AI', description: t('taskGraphNodeDescLlm') },
    businessRole('local_shell', 'shell', t('taskGraphNodeTypeLocalShell'), '#0f766e', categories.runtime, '$', t('taskGraphNodeDescLocalShell')),
    { key: 'shell', type: 'shell', label: t('taskGraphNodeTypeShell'), color: '#0f766e', category: 'runtime', categoryLabel: categories.runtime, icon: '$', description: t('taskGraphNodeDescShell') },
    { key: 'sub_graph', type: 'sub_graph', label: t('taskGraphNodeTypeSubPipeline'), color: '#7c3aed', category: 'artifact', categoryLabel: categories.artifact, icon: 'G', description: t('taskGraphNodeDescSubGraph') },
    { key: 'branch', type: 'branch', label: t('taskGraphNodeTypeBranch'), color: '#9333ea', category: 'control', categoryLabel: categories.control, icon: 'B', description: t('taskGraphNodeDescBranch') },
    { key: 'loop', type: 'loop', label: t('taskGraphNodeTypeLoop'), color: '#0891b2', category: 'control', categoryLabel: categories.control, icon: 'L', description: t('taskGraphNodeDescLoop') },
    { key: 'human_gate', type: 'human_gate', label: t('taskGraphNodeTypeHumanGate'), color: '#d97706', category: 'approval', categoryLabel: categories.approval, icon: 'H', description: t('taskGraphNodeDescHumanGate') },
    { key: 'end', type: 'end', label: t('taskGraphNodeTypeEnd'), color: '#059669', category: 'terminal', categoryLabel: categories.terminal, icon: 'E', description: t('taskGraphNodeDescEnd') },
  ]
}

export function taskGraphNodeVisual(type?: string): TaskGraphNodeVisual {
  const items = taskGraphNodeTypes()
  return items.find((item) => item.key === type)
    ?? items.find((item) => !item.role && item.type === type)
    ?? {
    key: type ?? 'unknown',
    type: 'llm',
    label: type ?? '',
    color: '#64748b',
    category: 'runtime',
    categoryLabel: taskGraphNodeCategoryLabels().runtime,
    icon: '?',
    description: '',
  }
}

export function taskGraphNodeVisualForNode(node: TaskGraphNode): TaskGraphNodeVisual {
  const role = typeof node.config?.role === 'string' ? node.config.role : ''
  return taskGraphNodeTypes().find((item) => item.type === node.type && item.role === role)
    ?? taskGraphNodeVisual(node.type)
}

export function taskGraphNodeColor(type?: string) {
  return taskGraphNodeVisual(type).color
}

function configString(node: TaskGraphNode, key: string) {
  const value = node.config[key]
  return typeof value === 'string' ? value : ''
}

function loopMeta(node: TaskGraphNode, graphInputs: TaskGraphInputParam[] = []) {
  const config = node.config ?? {}
  const ref = typeof config.max_iterations_ref === 'string' ? config.max_iterations_ref : ''
  const inputId = ref.match(/^\{\{inputs\.([^}]+)\}\}$/)?.[1]
  const inputDefault = inputId ? graphInputs.find((input) => input.id === inputId)?.default : undefined
  const value = inputDefault ?? config.max_iterations ?? ref
  return value === undefined || value === '' ? t('taskGraphLoopsEmpty') : t('taskGraphLoops', { count: String(value) })
}

export function taskGraphNodeMetaLabel(node: TaskGraphNode, graphInputs: TaskGraphInputParam[] = []) {
  const visual = taskGraphNodeVisualForNode(node)
  if (node.type === 'loop') return `${visual.categoryLabel} · ${loopMeta(node, graphInputs)}`
  if (node.type === 'shell') {
    const command = configString(node, 'command')
    return command ? `${visual.categoryLabel} · ${command}` : visual.categoryLabel
  }
  return visual.categoryLabel
}

function businessRole(
  role: TaskGraphNodeRole,
  type: TaskGraphNode['type'],
  label: string,
  color: string,
  categoryLabel: string,
  icon: string,
  description: string,
): TaskGraphNodeVisual {
  return {
    key: role,
    type,
    role,
    label,
    color,
    category: role === 'handoff_writer' ? 'artifact' : 'runtime',
    categoryLabel,
    icon,
    description,
    defaultConfig: { role },
  }
}
