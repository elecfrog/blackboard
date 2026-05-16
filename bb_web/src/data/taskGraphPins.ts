import type { NodePin, PinValueType, TaskGraphNode } from './taskGraphs'

export const PIN_VALUE_TYPE_COLORS: Record<PinValueType, string> = {
  string: '#ec4899',
  text: '#14b8a6',
  int: '#06b6d4',
  float: '#06b6d4',
  bool: '#ef4444',
  json: '#f97316',
  array: '#8b5cf6',
  any: '#6b7280',
  markdown: '#10b981',
  file_ref: '#64748b',
  wiki_ref: '#0ea5e9',
  ticket_ref: '#2563eb',
  diff: '#f59e0b',
  test_result: '#22c55e',
  review_comment: '#a855f7',
  handoff_summary: '#84cc16',
  runtime_log: '#475569',
  artifact_ref: '#0891b2',
}

export const PIN_EXEC_COLOR = '#9ca3af'

export function getDefaultPins(nodeType: TaskGraphNode['type'], config: Record<string, unknown> = {}): NodePin[] {
  switch (nodeType) {
    case 'start':
      return [
        { id: 'exec_out', label: 'Out', direction: 'out', category: 'exec' },
      ]
    case 'end':
      return [
        { id: 'exec_in', label: 'In', direction: 'in', category: 'exec', required: true },
      ]
    case 'llm':
      return [
        { id: 'exec_in', label: 'In', direction: 'in', category: 'exec', required: true },
        { id: 'exec_out', label: 'Out', direction: 'out', category: 'exec' },
        { id: 'output', label: 'Output', direction: 'out', category: 'data', value_type: 'json' },
      ]
    case 'plan':
      return [
        { id: 'exec_in', label: 'In', direction: 'in', category: 'exec', required: true },
        { id: 'plan_input', label: 'Plan Input', direction: 'in', category: 'data', value_type: 'json' },
        { id: 'exec_out', label: 'Out', direction: 'out', category: 'exec' },
        { id: 'output', label: 'Output', direction: 'out', category: 'data', value_type: 'json' },
      ]
    case 'llm_mutation':
      return [
        { id: 'exec_in', label: 'In', direction: 'in', category: 'exec', required: true },
        { id: 'plan_input', label: 'Plan Input', direction: 'in', category: 'data', value_type: 'json' },
        { id: 'exec_out', label: 'Out', direction: 'out', category: 'exec' },
        { id: 'mutation_artifact', label: 'Mutation Artifact', direction: 'out', category: 'data', value_type: 'json' },
      ]
    case 'shell':
      return [
        { id: 'exec_in', label: 'In', direction: 'in', category: 'exec', required: true },
        { id: 'exec_out', label: 'Out', direction: 'out', category: 'exec' },
        { id: 'output', label: 'Output', direction: 'out', category: 'data', value_type: 'json' },
      ]
    case 'sub_graph':
      return [
        { id: 'exec_in', label: 'In', direction: 'in', category: 'exec', required: true },
        { id: 'exec_out', label: 'Out', direction: 'out', category: 'exec' },
        { id: 'output', label: 'Output', direction: 'out', category: 'data', value_type: 'json' },
      ]
    case 'branch': {
      const pins: NodePin[] = [
        { id: 'exec_in', label: 'In', direction: 'in', category: 'exec', required: true },
      ]
      const rules = (config.rules ?? []) as Array<{ id: string; label: string }>
      for (const rule of rules) {
        pins.push({ id: `rule:${rule.id}`, label: rule.label, direction: 'out', category: 'exec' })
      }
      return pins
    }
    case 'loop':
      return [
        { id: 'exec_in', label: 'In', direction: 'in', category: 'exec', required: true },
        { id: 'body', label: 'Body', direction: 'out', category: 'exec' },
        { id: 'exit', label: 'Exit', direction: 'out', category: 'exec' },
        { id: 'index', label: 'Index', direction: 'out', category: 'data', value_type: 'int' },
      ]
    case 'human_gate':
      return [
        { id: 'exec_in', label: 'In', direction: 'in', category: 'exec', required: true },
        { id: 'exec_out', label: 'Out', direction: 'out', category: 'exec' },
        { id: 'action', label: 'Action', direction: 'out', category: 'data', value_type: 'string' },
      ]
    default:
      return []
  }
}

export function getNodePins(node: TaskGraphNode): NodePin[] {
  if (node.pins && node.pins.length > 0) return node.pins
  return getDefaultPins(node.type, node.config)
}

export function canConnect(sourcePin: NodePin, targetPin: NodePin): boolean {
  if (sourcePin.direction !== 'out' || targetPin.direction !== 'in') return false
  if (sourcePin.category !== targetPin.category) return false
  if (sourcePin.category === 'data') {
    if (targetPin.value_type === 'any' || sourcePin.value_type === 'any') return true
    return sourcePin.value_type === targetPin.value_type
  }
  return true
}

export interface CanvasPinDescriptor {
  handle: string
  side: 'left' | 'right'
  x: number
  y: number
  label: string
  role: 'source' | 'target'
  category: 'exec' | 'data'
  valueType?: string
  color: string
}

export const NODE_HEADER_HEIGHT = 32
export const PIN_ROW_HEIGHT = 24
export const PIN_AREA_PADDING = 6
export const NODE_MIN_HEIGHT = 48

export function nodeToCanvasPins(node: TaskGraphNode): CanvasPinDescriptor[] {
  const pins = getNodePins(node)
  const inPins = pins.filter((p) => p.direction === 'in')
  const outPins = pins.filter((p) => p.direction === 'out')
  const result: CanvasPinDescriptor[] = []
  const nodeWidth = 230
  const pinAreaTop = NODE_HEADER_HEIGHT + PIN_AREA_PADDING

  for (let i = 0; i < inPins.length; i++) {
    const pin = inPins[i]
    result.push({
      handle: pin.id,
      side: 'left',
      x: 0,
      y: pinAreaTop + i * PIN_ROW_HEIGHT + PIN_ROW_HEIGHT / 2,
      label: pin.label,
      role: 'target',
      category: pin.category,
      valueType: pin.value_type,
      color: pin.category === 'data' ? PIN_VALUE_TYPE_COLORS[pin.value_type ?? 'any'] : PIN_EXEC_COLOR,
    })
  }

  for (let i = 0; i < outPins.length; i++) {
    const pin = outPins[i]
    result.push({
      handle: pin.id,
      side: 'right',
      x: nodeWidth,
      y: pinAreaTop + i * PIN_ROW_HEIGHT + PIN_ROW_HEIGHT / 2,
      label: pin.label,
      role: 'source',
      category: pin.category,
      valueType: pin.value_type,
      color: pin.category === 'data' ? PIN_VALUE_TYPE_COLORS[pin.value_type ?? 'any'] : PIN_EXEC_COLOR,
    })
  }

  return result
}

export function nodeHeightForPins(node: TaskGraphNode): number {
  const pins = getNodePins(node)
  const inCount = pins.filter((p) => p.direction === 'in').length
  const outCount = pins.filter((p) => p.direction === 'out').length
  const maxPins = Math.max(inCount, outCount, 0)
  if (maxPins === 0) return NODE_MIN_HEIGHT
  return NODE_HEADER_HEIGHT + PIN_AREA_PADDING + maxPins * PIN_ROW_HEIGHT + PIN_AREA_PADDING
}
