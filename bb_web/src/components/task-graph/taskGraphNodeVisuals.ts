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
  requiredInputs?: TaskGraphInputParam[]
}

export function taskGraphNodeCategoryLabels(): Record<TaskGraphNodeCategory, string> {
  return {
    terminal: t('taskGraphNodeCategoryTerminal'),
    control: t('taskGraphNodeCategoryControl'),
    data: t('taskGraphNodeCategoryData'),
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
    {
      key: 'task_intent',
      type: 'intent_extract',
      label: t('taskGraphNodeTypeTaskIntent'),
      color: '#0d9488',
      category: 'transform',
      categoryLabel: categories.transform,
      icon: 'IN',
      description: t('taskGraphNodeDescTaskIntent'),
      requiredInputs: [
        {
          id: 'intent',
          label: 'Prompt Intent',
          type: 'string',
          default: '',
          description: 'User prompt for this task graph run.',
        },
      ],
      defaultConfig: {
        preset: 'task_intent',
        mode: 'intent_gate',
        language: 'zh-CN',
        inputs: {
          request: '{{inputs.intent}}',
          draft: {
            intent_type: 'task',
            request: '{{inputs.intent}}',
          },
        },
      },
    },
    { key: 'llm', type: 'llm', label: t('taskGraphNodeTypeLlm'), color: '#2563eb', category: 'runtime', categoryLabel: categories.runtime, icon: 'AI', description: t('taskGraphNodeDescLlm') },
    {
      key: 'input_var',
      type: 'input_var',
      label: t('taskGraphNodeTypeInputVar'),
      color: '#10b981',
      category: 'transform',
      categoryLabel: categories.transform,
      icon: 'IN',
      description: t('taskGraphNodeDescInputVar'),
    },
    {
      key: 'data_value',
      type: 'data_value',
      label: t('taskGraphNodeTypeDataValue'),
      color: '#10b981',
      category: 'data',
      categoryLabel: categories.data,
      icon: 'VAL',
      description: t('taskGraphNodeDescDataValue'),
      defaultConfig: {
        preset: 'data_value',
        value_type: 'string',
        value: '',
      },
    },
    {
      key: 'resource_bundle',
      type: 'data_value',
      label: t('taskGraphNodeTypeResourceBundle'),
      color: '#10b981',
      category: 'data',
      categoryLabel: categories.data,
      icon: 'RB',
      description: t('taskGraphNodeDescResourceBundle'),
      requiredInputs: [
        {
          id: 'input',
          label: 'Input Prompt',
          type: 'string',
          default: '',
          description: 'User prompt for this task graph run.',
        },
        {
          id: 'source-root',
          label: 'Source Root',
          type: 'string',
          default: '',
          description: 'Root directory for code or documents that agents should investigate.',
        },
        {
          id: 'wiki-slice',
          label: 'Wiki Slice',
          type: 'string',
          default: '',
          description: 'Optional wiki/context excerpt to inject as structured context.',
        },
        {
          id: 'skills',
          label: 'Skills',
          type: 'json',
          default: ['code-research'],
          description: 'Skill ids requested by this graph.',
        },
      ],
      defaultConfig: {
        preset: 'resource_bundle',
        value_type: 'json',
        value: {
          schema_version: 1,
          kind: 'task_graph_llm_resource_bundle',
          task: {
            input: '{{inputs.input}}',
          },
          data_sources: {
            source_root: '{{inputs.source-root}}',
            wiki_slice: '{{inputs.wiki-slice}}',
            extra_roots: [],
          },
          upstream_artifacts: {},
          runtime: {
            project: '{{env.project}}',
            workspace: '{{env.workspace}}',
          },
          skills: '{{inputs.skills}}',
          contracts: {
            llm_input: 'resource_bundle',
            path_policy: 'data_sources.source_root is the research/data root; runtime.workspace is only the agent working directory.',
            write_policy: 'LLM nodes should not write final files directly unless the graph contract explicitly allows it.',
          },
        },
      },
    },
    {
      key: 'llm_coordinator',
      type: 'llm_coordinator',
      label: t('taskGraphNodeTypeCoordinator'),
      color: '#0f766e',
      category: 'runtime',
      categoryLabel: categories.runtime,
      icon: 'CO',
      description: t('taskGraphNodeDescCoordinator'),
      defaultConfig: {
        preset: 'llm_coordinator',
        run_as: 'llm',
        runtime: 'opencode',
        agent: 'native',
        model: '',
        prompt: {
          mode: 'inline',
          template: [
            '你是 Blackboard TaskGraph 的 LLM Coordinator。',
            '根据输入上下文生成一个隔离子图草案，用于完成动态分解、内部收敛和结果产出。',
            '只输出严格 JSON，不要 Markdown。',
            '',
            '输出必须是一个完整 TaskGraphDefinition，或形如 { "subgraph": TaskGraphDefinition }。',
            '子图必须包含 start/end，内部节点和边必须自洽，输出会作为 Coordinator 节点结果返回。',
            '',
            '上游任务上下文:',
            '{{inputs.input}}',
          ].join('\n'),
        },
        inputs: {
          input: '',
        },
        output: { artifact_type: 'json', required: true },
        skills: [],
        mcp_servers: [],
        toolkits: ['blackboard_mcp'],
        custom_env: {},
        custom_args: [],
        coordinator: {
          max_nodes: 32,
          max_edges: 64,
          allowed_node_types: [],
        },
        input_bindings: {},
      },
    },
    {
      key: 'mutation',
      type: 'llm_mutation',
      label: t('taskGraphNodeTypeMutation'),
      color: '#be123c',
      category: 'runtime',
      categoryLabel: categories.runtime,
      icon: 'MU',
      description: t('taskGraphNodeDescMutation'),
      defaultConfig: {
        preset: 'mutation',
        mode: 'fanout',
        target_node_id: '',
        run_as: 'llm',
        runtime: 'opencode',
        agent: 'native',
        model: '',
        prompt: {
          mode: 'inline',
          template: [
            '你是 Blackboard TaskGraph 的 Mutation coordinator。',
            '基于上游任务目标，拆解需要并行 scout 的调查模块。',
            '只输出严格 JSON，不要 Markdown。',
            '',
            '输出 schema:',
            '{',
            '  "scouts": [',
            '    { "id": "editor", "goal": "调查一个独立方向", "scope_hints": ["editor"] }',
            '  ],',
            '  "merge_goal": "最终汇总目标",',
            '  "doc_target": "可选文档目标路径"',
            '}',
            '',
            '上游任务上下文:',
            '{{inputs.input}}',
          ].join('\n'),
        },
        inputs: {
          input: '',
        },
        output: { artifact_type: 'json', required: true },
        skills: [],
        mcp_servers: [],
        toolkits: ['blackboard_mcp'],
        custom_env: {},
        custom_args: [],
        generated: {
          runtime: 'opencode',
          agent: 'native',
          model: '',
          output_artifact_type: 'markdown',
          prompt_template: [
            '你是一个动态 scout LLM。',
            '只调查分配给你的方向，不要汇总其他 scout 的工作。',
            '',
            'Scout id: {{item.id}}',
            'Goal: {{item.goal}}',
            'Scope hints: {{item.scope_hints}}',
            'Expected output: {{item.expected_output}}',
            '',
            '输出 markdown findings，包含关键事实、证据位置、风险和未覆盖范围。',
          ].join('\n'),
        },
      },
    },
    { key: 'plan', type: 'plan', label: t('taskGraphNodeTypePlan'), color: '#f97316', category: 'transform', categoryLabel: categories.transform, icon: 'PL', description: t('taskGraphNodeDescPlan') },
    { key: 'kb_plan', type: 'kb_plan', label: t('taskGraphNodeTypeKbPlan'), color: '#ea580c', category: 'transform', categoryLabel: categories.transform, icon: 'KB', description: t('taskGraphNodeDescKbPlan') },
    { key: 'manifest_merge', type: 'manifest_merge', label: t('taskGraphNodeTypeManifestMerge'), color: '#c2410c', category: 'transform', categoryLabel: categories.transform, icon: 'MF', description: t('taskGraphNodeDescManifestMerge') },
    { key: 'schema_validate', type: 'schema_validate', label: t('taskGraphNodeTypeSchemaValidate'), color: '#475569', category: 'transform', categoryLabel: categories.transform, icon: 'SV', description: t('taskGraphNodeDescSchemaValidate') },
    { key: 'shell', type: 'shell', label: t('taskGraphNodeTypeShell'), color: '#0f766e', category: 'runtime', categoryLabel: categories.runtime, icon: '$', description: t('taskGraphNodeDescShell') },
    { key: 'system_write_output', type: 'system_write_output', label: t('taskGraphNodeTypeSystemWriteOutput'), color: '#16a34a', category: 'artifact', categoryLabel: categories.artifact, icon: 'WR', description: t('taskGraphNodeDescSystemWriteOutput') },
    { key: 'sub_graph', type: 'sub_graph', label: t('taskGraphNodeTypeSubPipeline'), color: '#7c3aed', category: 'artifact', categoryLabel: categories.artifact, icon: 'G', description: t('taskGraphNodeDescSubGraph') },
    { key: 'branch', type: 'branch', label: t('taskGraphNodeTypeBranch'), color: '#9333ea', category: 'control', categoryLabel: categories.control, icon: 'B', description: t('taskGraphNodeDescBranch') },
    { key: 'loop', type: 'loop', label: t('taskGraphNodeTypeLoop'), color: '#0891b2', category: 'control', categoryLabel: categories.control, icon: 'L', description: t('taskGraphNodeDescLoop') },
    { key: 'human_gate', type: 'human_gate', label: t('taskGraphNodeTypeHumanGate'), color: '#d97706', category: 'approval', categoryLabel: categories.approval, icon: 'H', description: t('taskGraphNodeDescHumanGate') },
    { key: 'end', type: 'end', label: t('taskGraphNodeTypeEnd'), color: '#059669', category: 'terminal', categoryLabel: categories.terminal, icon: 'E', description: t('taskGraphNodeDescEnd') },
    { key: 'failed_end', type: 'end', label: t('taskGraphNodeTypeFailedEnd'), color: '#dc2626', category: 'terminal', categoryLabel: categories.terminal, icon: 'F', description: t('taskGraphNodeDescFailedEnd'), defaultConfig: { preset: 'failed_end', result: 'failed' } },
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
  const preset = typeof node.config?.preset === 'string' ? node.config.preset : ''
  const role = typeof node.config?.role === 'string' ? node.config.role : ''
  return taskGraphNodeTypes().find((item) => item.key === preset || item.defaultConfig?.preset === preset)
    ?? (node.type === 'end' && node.config?.result === 'failed'
      ? taskGraphNodeTypes().find((item) => item.key === 'failed_end')
      : undefined)
    ?? taskGraphNodeTypes().find((item) => item.type === node.type && item.role === role)
    ?? taskGraphNodeVisual(node.type)
}

export function taskGraphNodeColor(type?: string) {
  return taskGraphNodeVisual(type).color
}

function configString(node: TaskGraphNode, key: string) {
  const value = node.config[key]
  return typeof value === 'string' ? value : ''
}

function llmCapabilities(node: TaskGraphNode) {
  const caps: string[] = []
  const toolkits = Array.isArray(node.config?.toolkits)
    ? node.config.toolkits.map(toolkitId).filter(Boolean)
    : []
  if (toolkits.length > 0) caps.push(`Toolkits: ${toolkits.join(', ')}`)
  if (node.config?.resource_bundle) caps.push('ResourceBundle')
  if (node.config?.tool_policy) caps.push('Tools')
  if (node.config?.output_contract) caps.push('Output Contract')
  if (node.config?.lifecycle) caps.push('Lifecycle')
  return caps
}

function toolkitId(value: unknown) {
  if (typeof value === 'string') return value
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    const id = (value as Record<string, unknown>).id
    return typeof id === 'string' ? id : ''
  }
  return ''
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
  if (node.type === 'llm_mutation') {
    const target = configString(node, 'target_node_id')
    return target ? `${visual.categoryLabel} · target ${target}` : `${visual.categoryLabel} · fanout`
  }
  if (node.type === 'llm_coordinator') {
    const maxNodes = (node.config.coordinator as Record<string, unknown> | undefined)?.max_nodes
    return `${visual.categoryLabel} · subgraph${maxNodes ? ` ≤ ${maxNodes} nodes` : ''}`
  }
  if (node.type === 'data_value') {
    if (node.config?.preset === 'resource_bundle') return `${visual.categoryLabel} · ResourceBundle`
    return `${visual.categoryLabel} · ${configString(node, 'value_type') || 'string'}`
  }
  if (node.type === 'input_var') {
    const inputId = configString(node, 'input_id')
    return inputId ? `${visual.categoryLabel} · ${inputId}` : visual.categoryLabel
  }
  if (node.type === 'kb_plan') {
    const mode = configString(node, 'mode')
    return mode ? `${visual.categoryLabel} · ${mode}` : visual.categoryLabel
  }
  if (node.type === 'schema_validate') {
    const schemaName = configString(node, 'schema_name')
    return schemaName ? `${visual.categoryLabel} · ${schemaName}` : visual.categoryLabel
  }
  if (node.type === 'system_write_output') {
    const outputPath = configString(node, 'output_path')
    return outputPath ? `${visual.categoryLabel} · ${outputPath}` : visual.categoryLabel
  }
  if (node.type === 'llm') {
    const runtime = configString(node, 'runtime')
    const caps = llmCapabilities(node)
    if (caps.length > 0) {
      return `${visual.categoryLabel} · LLM/${runtime || 'default'} · ${caps.join(' + ')}`
    }
    return runtime ? `${visual.categoryLabel} · LLM/${runtime}` : visual.categoryLabel
  }
  if (node.type === 'shell') {
    const command = configString(node, 'command')
    return command ? `${visual.categoryLabel} · ${command}` : visual.categoryLabel
  }
  return visual.categoryLabel
}
