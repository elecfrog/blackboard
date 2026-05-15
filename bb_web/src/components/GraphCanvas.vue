<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { t } from '@/i18n'

export type GraphCanvasPinSide = 'top' | 'right' | 'bottom' | 'left'
export type GraphCanvasPinRole = 'source' | 'target' | 'both'

export interface GraphCanvasViewport {
  x: number
  y: number
  scale: number
}

export interface GraphCanvasNode {
  id: string
  x: number
  y: number
  width?: number
  height?: number
  status?: string
  kind?: string
  color?: string
  disabled?: boolean
  connectable?: boolean
  label?: string
  meta?: string
  title?: string
  titleLines?: string[]
  lane?: string
  pins?: GraphCanvasPin[]
  classes?: string[]
}

export interface GraphCanvasPin {
  handle: string
  side: GraphCanvasPinSide
  x: number
  y: number
  label?: string
  role?: GraphCanvasPinRole
  category?: 'exec' | 'data'
  valueType?: string
  color?: string
}

export interface GraphCanvasEdge {
  id?: string
  from: string
  to: string
  label?: string
  color?: string
  sourceHandle?: string
  targetHandle?: string
  removable?: boolean
  classes?: string[]
  dashed?: boolean
  edgeColor?: string
}

export interface GraphCanvasConnection {
  sourceId: string
  targetId: string
  sourceSide: GraphCanvasPinSide
  targetSide: GraphCanvasPinSide
  sourceHandle?: string
  targetHandle?: string
}

export interface GraphCanvasNodeMove {
  id: string
  x: number
  y: number
}

interface NodePin {
  side: GraphCanvasPinSide
  x: number
  y: number
  handle?: string
  label?: string
  role?: GraphCanvasPinRole
  category?: 'exec' | 'data'
  valueType?: string
  color?: string
}

interface ConnectionDrag {
  sourceId: string
  sourceSide: GraphCanvasPinSide
  start: { x: number; y: number }
  pointer: { x: number; y: number }
  targetId: string
  targetSide: GraphCanvasPinSide
  sourceHandle?: string
  targetHandle?: string
}

const props = withDefaults(defineProps<{
  nodes: GraphCanvasNode[]
  edges: GraphCanvasEdge[]
  canvasSize: { width: number; height: number }
  defaultNodeWidth?: number
  defaultNodeHeight?: number
  focusId?: string
  savingId?: string | null
  readonly?: boolean
  snapRadius?: number
  sniffPadding?: number
  fitPadding?: number
  minScale?: number
  maxScale?: number
  fitOnMount?: boolean
  nodeGridSize?: number
  nodeGridSnapThreshold?: number
  nodeAlignThreshold?: number
  nodeSnapStartDistance?: number
  nodeSnapping?: boolean
}>(), {
  canvasSize: () => ({ width: 1200, height: 760 }),
  defaultNodeWidth: 320,
  defaultNodeHeight: 102,
  focusId: '',
  savingId: null,
  readonly: false,
  snapRadius: 42,
  sniffPadding: 58,
  fitPadding: 72,
  minScale: 0.03,
  maxScale: 64,
  fitOnMount: true,
  nodeGridSize: 32,
  nodeGridSnapThreshold: 7,
  nodeAlignThreshold: 10,
  nodeSnapStartDistance: 10,
  nodeSnapping: true,
})

const emit = defineEmits<{
  nodeOpen: [id: string]
  nodeSelect: [id: string]
  nodeMove: [move: GraphCanvasNodeMove]
  nodeMoveEnd: [move: GraphCanvasNodeMove]
  connectionCreate: [connection: GraphCanvasConnection]
  edgeSelect: [edge: GraphCanvasEdge]
  edgeRemove: [edge: GraphCanvasEdge]
  viewportChange: [viewport: GraphCanvasViewport]
}>()

const pinSides: GraphCanvasPinSide[] = ['top', 'right', 'bottom', 'left']
const canvas = ref<SVGSVGElement | null>(null)
const viewport = ref<GraphCanvasViewport>({ x: 40, y: 36, scale: 1 })
const selectedNode = ref('')
const connectionDrag = ref<ConnectionDrag | null>(null)
const dragState = ref<
  | { type: 'node'; id: string; startX: number; startY: number; nodeX: number; nodeY: number }
  | { type: 'pan'; startX: number; startY: number; viewX: number; viewY: number }
  | null
>(null)

const transform = computed(() => `translate(${viewport.value.x} ${viewport.value.y}) scale(${viewport.value.scale})`)
const canvasDimensions = computed(() => props.canvasSize ?? { width: 1200, height: 760 })
const nodeById = computed(() => new Map(props.nodes.map((node) => [node.id, node])))

function nodeWidth(node: GraphCanvasNode) {
  return node.width ?? props.defaultNodeWidth
}

function nodeHeight(node: GraphCanvasNode) {
  return node.height ?? props.defaultNodeHeight
}

function clientPoint(event: PointerEvent | WheelEvent) {
  const rect = canvas.value?.getBoundingClientRect()
  const scaleX = rect ? props.canvasSize.width / rect.width : 1
  const scaleY = rect ? props.canvasSize.height / rect.height : 1
  return {
    x: (event.clientX - (rect?.left ?? 0)) * scaleX,
    y: (event.clientY - (rect?.top ?? 0)) * scaleY,
  }
}

function clientDelta(startX: number, startY: number, event: PointerEvent) {
  const rect = canvas.value?.getBoundingClientRect()
  const scaleX = rect ? props.canvasSize.width / rect.width : 1
  const scaleY = rect ? props.canvasSize.height / rect.height : 1
  return {
    x: (event.clientX - startX) * scaleX,
    y: (event.clientY - startY) * scaleY,
  }
}

function screenToWorld(point: { x: number; y: number }) {
  return {
    x: (point.x - viewport.value.x) / viewport.value.scale,
    y: (point.y - viewport.value.y) / viewport.value.scale,
  }
}

function clampNodePosition(position: { x: number; y: number }) {
  return {
    x: Math.max(-10000, position.x),
    y: Math.max(-10000, position.y),
  }
}

function snapThreshold(screenPx: number, worldCap: number) {
  return Math.min(screenPx / viewport.value.scale, worldCap)
}

function snapValueToGrid(value: number, threshold: number) {
  if (!props.nodeGridSize || props.nodeGridSize <= 0) return value
  const snapped = Math.round(value / props.nodeGridSize) * props.nodeGridSize
  return Math.abs(snapped - value) <= threshold ? snapped : value
}

function nodeAnchors(node: GraphCanvasNode, x = node.x, y = node.y) {
  const width = nodeWidth(node)
  const height = nodeHeight(node)
  return {
    x: [x, x + width / 2, x + width],
    y: [y, y + height / 2, y + height],
  }
}

function nearestAxisAlignment(
  movingAnchors: number[],
  targetAnchors: number[],
  threshold: number,
) {
  let best: { delta: number; guide: number; distance: number } | null = null

  for (const moving of movingAnchors) {
    for (const target of targetAnchors) {
      const delta = target - moving
      const distance = Math.abs(delta)
      if (distance <= threshold && (!best || distance < best.distance)) {
        best = { delta, guide: target, distance }
      }
    }
  }

  return best
}

function resolveNodePosition(
  id: string,
  rawPosition: { x: number; y: number },
  event?: PointerEvent,
) {
  const node = nodeById.value.get(id)
  if (!node || !props.nodeSnapping || event?.shiftKey) {
    return clampNodePosition(rawPosition)
  }

  const gridThreshold = snapThreshold(props.nodeGridSnapThreshold, props.nodeGridSize * 0.28)
  const alignThreshold = snapThreshold(props.nodeAlignThreshold, props.nodeGridSize * 0.5)
  const movingAnchors = nodeAnchors(node, rawPosition.x, rawPosition.y)
  let nextX = rawPosition.x
  let nextY = rawPosition.y
  let guideX: number | undefined
  let guideY: number | undefined

  for (const other of props.nodes) {
    if (other.id === id || other.disabled) continue
    const otherAnchors = nodeAnchors(other)
    const xAlignment = nearestAxisAlignment(movingAnchors.x, otherAnchors.x, alignThreshold)
    const yAlignment = nearestAxisAlignment(movingAnchors.y, otherAnchors.y, alignThreshold)

    if (xAlignment && (guideX === undefined || xAlignment.distance < Math.abs(nextX - rawPosition.x))) {
      nextX = rawPosition.x + xAlignment.delta
      guideX = xAlignment.guide
    }

    if (yAlignment && (guideY === undefined || yAlignment.distance < Math.abs(nextY - rawPosition.y))) {
      nextY = rawPosition.y + yAlignment.delta
      guideY = yAlignment.guide
    }
  }

  if (guideX === undefined) nextX = snapValueToGrid(nextX, gridThreshold)
  if (guideY === undefined) nextY = snapValueToGrid(nextY, gridThreshold)

  return clampNodePosition({ x: nextX, y: nextY })
}

function nodeDragPosition(state: { id: string; startX: number; startY: number; nodeX: number; nodeY: number }, event: PointerEvent) {
  const delta = clientDelta(state.startX, state.startY, event)
  const rawPosition = {
    x: state.nodeX + delta.x / viewport.value.scale,
    y: state.nodeY + delta.y / viewport.value.scale,
  }
  const moved = Math.hypot(event.clientX - state.startX, event.clientY - state.startY)
  if (moved < props.nodeSnapStartDistance) return clampNodePosition(rawPosition)
  return resolveNodePosition(state.id, rawPosition, event)
}

function beginNodeDrag(event: PointerEvent, node: GraphCanvasNode) {
  if (node.disabled || connectionDrag.value) return
  selectedNode.value = node.id
  emit('nodeSelect', node.id)
  // readonly 模式下只允许选择，不允许拖拽
  if (props.readonly) return
  dragState.value = {
    type: 'node',
    id: node.id,
    startX: event.clientX,
    startY: event.clientY,
    nodeX: node.x,
    nodeY: node.y,
  }
  ;(event.currentTarget as Element).setPointerCapture(event.pointerId)
}

function beginPan(event: PointerEvent) {
  if (event.target !== canvas.value || connectionDrag.value) return
  dragState.value = {
    type: 'pan',
    startX: event.clientX,
    startY: event.clientY,
    viewX: viewport.value.x,
    viewY: viewport.value.y,
  }
  canvas.value?.setPointerCapture(event.pointerId)
}

function onPointerMove(event: PointerEvent) {
  if (connectionDrag.value) {
    updateConnectionDrag(event)
    return
  }
  const state = dragState.value
  if (!state) return
  if (state.type === 'node') {
    const position = nodeDragPosition(state, event)
    emit('nodeMove', {
      id: state.id,
      x: position.x,
      y: position.y,
    })
    return
  }
  const delta = clientDelta(state.startX, state.startY, event)
  viewport.value = {
    ...viewport.value,
    x: state.viewX + delta.x,
    y: state.viewY + delta.y,
  }
}

function endPointer(event?: PointerEvent) {
  if (connectionDrag.value) {
    finishConnectionDrag(event)
    return
  }
  const state = dragState.value
  if (state?.type === 'node') {
    const node = nodeById.value.get(state.id)
    if (event) {
      const position = nodeDragPosition(state, event)
      emit('nodeMoveEnd', {
        id: state.id,
        x: position.x,
        y: position.y,
      })
    } else if (node) {
      emit('nodeMoveEnd', { id: node.id, x: node.x, y: node.y })
    }
  } else if (state?.type === 'pan' && event) {
    // 如果 pan 没有实际移动（即点击空白区域），取消节点选择
    const delta = clientDelta(state.startX, state.startY, event)
    const moved = Math.abs(delta.x) > 3 || Math.abs(delta.y) > 3
    if (!moved) {
      selectedNode.value = ''
      emit('nodeSelect', '')
    }
  }
  dragState.value = null
}

function onWheel(event: WheelEvent) {
  event.preventDefault()
  const point = clientPoint(event)
  const before = screenToWorld(point)
  const factor = event.deltaY < 0 ? 1.12 : 0.89
  const nextScale = Math.min(props.maxScale, Math.max(props.minScale, viewport.value.scale * factor))
  viewport.value = {
    scale: nextScale,
    x: point.x - before.x * nextScale,
    y: point.y - before.y * nextScale,
  }
}

function pinOffset(node: GraphCanvasNode, side: GraphCanvasPinSide): NodePin {
  const width = nodeWidth(node)
  const height = nodeHeight(node)
  if (side === 'top') return { side, x: width / 2, y: 0 }
  if (side === 'right') return { side, x: width, y: height / 2 }
  if (side === 'bottom') return { side, x: width / 2, y: height }
  return { side, x: 0, y: height / 2 }
}

function explicitPinOffset(_node: GraphCanvasNode, pin: GraphCanvasPin): NodePin {
  return {
    side: pin.side,
    x: pin.x,
    y: pin.y,
    handle: pin.handle,
    label: pin.label,
    role: pin.role ?? 'both',
    category: pin.category,
    valueType: pin.valueType,
    color: pin.color,
  }
}

function pinMatchesRole(pin: NodePin, role?: GraphCanvasPinRole) {
  if (!role) return true
  const pinRole = pin.role ?? 'both'
  return pinRole === 'both' || pinRole === role
}

function nodePins(node: GraphCanvasNode, role?: GraphCanvasPinRole) {
  const explicit = node.pins?.map((pin) => explicitPinOffset(node, pin)) ?? []
  if (explicit.length > 0) return explicit.filter((pin) => pinMatchesRole(pin, role))
  return pinSides
    .map((side) => ({ ...pinOffset(node, side), handle: `pin:${side}`, role: 'both' as GraphCanvasPinRole }))
    .filter((pin) => pinMatchesRole(pin, role))
}

function semanticPins(node: GraphCanvasNode) {
  return node.pins?.map((pin) => explicitPinOffset(node, pin)) ?? []
}

function isDataPin(pin: NodePin) {
  return pin.category === 'data'
}

function visiblePinLabel(pin: NodePin) {
  const label = pin.label ?? ''
  if (pin.category === 'exec' && (pin.handle === 'exec_in' || pin.handle === 'exec_out' || label === 'In' || label === 'Out')) {
    return ''
  }
  return label
}

function pinTransform(pin: NodePin) {
  return `translate(${pin.x}, ${pin.y})`
}

function pinKey(pin: NodePin) {
  return pin.handle ?? pin.side
}

function canStartConnection(pin: NodePin) {
  const role = pin.role ?? 'both'
  return role === 'source' || role === 'both'
}

function pinLabelX(pin: NodePin) {
  if (pin.side === 'left') return 10
  if (pin.side === 'right') return -10
  if (pin.side === 'bottom') return 8
  return 8
}

function pinLabelY(pin: NodePin) {
  if (pin.side === 'bottom') return -9
  if (pin.side === 'top') return 14
  return 4
}

function pinLabelAnchor(pin: NodePin) {
  return pin.side === 'right' ? 'end' : 'start'
}

function beginConnection(event: PointerEvent, node: GraphCanvasNode, pin: NodePin) {
  event.preventDefault()
  event.stopPropagation()
  if (props.readonly || node.disabled || node.connectable === false || !canStartConnection(pin)) return
  selectedNode.value = node.id
  emit('nodeSelect', node.id)
  const start = { x: node.x + pin.x, y: node.y + pin.y }
  connectionDrag.value = {
    sourceId: node.id,
    sourceSide: pin.side,
    sourceHandle: pin.handle,
    start,
    pointer: start,
    targetId: '',
    targetSide: 'left',
  }
  ;(event.currentTarget as Element).setPointerCapture(event.pointerId)
  window.addEventListener('pointermove', updateConnectionDrag)
  window.addEventListener('pointerup', finishConnectionDrag)
  window.addEventListener('pointercancel', finishConnectionDrag)
  updateConnectionDrag(event)
}

function updateConnectionDrag(event: PointerEvent) {
  const drag = connectionDrag.value
  if (!drag) return
  const pointer = screenToWorld(clientPoint(event))
  const target = findConnectionTarget(pointer, drag.sourceId)
  connectionDrag.value = {
    ...drag,
    pointer: target ? target.pin : pointer,
    targetId: target?.node.id ?? '',
    targetSide: target?.pin.side ?? 'left',
    targetHandle: target?.pin.handle,
  }
}

function finishConnectionDrag(event?: PointerEvent) {
  if (event) updateConnectionDrag(event)
  const drag = connectionDrag.value
  window.removeEventListener('pointermove', updateConnectionDrag)
  window.removeEventListener('pointerup', finishConnectionDrag)
  window.removeEventListener('pointercancel', finishConnectionDrag)
  connectionDrag.value = null
  if (!drag?.targetId || drag.targetId === drag.sourceId) return
  emit('connectionCreate', {
    sourceId: drag.sourceId,
    targetId: drag.targetId,
    sourceSide: drag.sourceSide,
    targetSide: drag.targetSide,
    sourceHandle: drag.sourceHandle,
    targetHandle: drag.targetHandle,
  })
}

function findConnectionTarget(pointer: { x: number; y: number }, sourceId: string) {
  let best: { node: GraphCanvasNode; pin: NodePin; distance: number } | null = null
  for (const node of props.nodes) {
    if (node.disabled || node.connectable === false || node.id === sourceId) continue
    const width = nodeWidth(node)
    const height = nodeHeight(node)
    const insideSniffBox =
      pointer.x >= node.x - props.sniffPadding &&
      pointer.x <= node.x + width + props.sniffPadding &&
      pointer.y >= node.y - props.sniffPadding &&
      pointer.y <= node.y + height + props.sniffPadding
    if (!insideSniffBox) continue
    for (const pin of nodePins(node, 'target')) {
      const distance = Math.hypot(pointer.x - (node.x + pin.x), pointer.y - (node.y + pin.y))
      if (distance <= props.snapRadius && (!best || distance < best.distance)) {
        best = { node, pin: { ...pin, x: node.x + pin.x, y: node.y + pin.y }, distance }
      }
    }
  }
  return best
}

function openNode(node: GraphCanvasNode) {
  if (node.disabled) return
  selectedNode.value = node.id
  emit('nodeSelect', node.id)
  emit('nodeOpen', node.id)
}

function edgeKey(edge: GraphCanvasEdge) {
  return edge.id ?? `${edge.from}-${edge.to}`
}

function edgeGradientId(edge: GraphCanvasEdge) {
  return `graph-edge-gradient-${edgeKey(edge)}`.replace(/[^a-zA-Z0-9_-]/g, '-')
}

function edgeNodes(edge: GraphCanvasEdge) {
  return {
    from: nodeById.value.get(edge.from),
    to: nodeById.value.get(edge.to),
  }
}

function edgePath(edge: GraphCanvasEdge) {
  const { from, to } = edgeNodes(edge)
  if (!from || !to) return ''
  return connectionPath(bestEdgePins(from, to, edge), edge)
}

function edgeGradient(edge: GraphCanvasEdge) {
  const { from, to } = edgeNodes(edge)
  if (!from || !to) {
    return { id: edgeGradientId(edge), x1: 0, y1: 0, x2: 1, y2: 0, fromColor: '#64748b', toColor: '#64748b' }
  }
  const pins = bestEdgePins(from, to, edge)
  return {
    id: edgeGradientId(edge),
    x1: pins.start.x,
    y1: pins.start.y,
    x2: pins.end.x,
    y2: pins.end.y,
    fromColor: edge.color ?? from.color ?? '#64748b',
    toColor: edge.color ?? to.color ?? '#64748b',
  }
}

function edgeMidpoint(edge: GraphCanvasEdge) {
  const { from, to } = edgeNodes(edge)
  if (!from || !to) return { x: 0, y: 0 }
  const pins = bestEdgePins(from, to, edge)
  if (edge.targetHandle === 'return') {
    const direction = pins.start.x >= pins.end.x ? 1 : -1
    return {
      x: (pins.start.x + pins.end.x) / 2 + direction * 54,
      y: (pins.start.y + pins.end.y) / 2 + 8,
    }
  }
  return { x: (pins.start.x + pins.end.x) / 2, y: (pins.start.y + pins.end.y) / 2 }
}

function bestEdgePins(from: GraphCanvasNode, to: GraphCanvasNode, edge?: GraphCanvasEdge) {
  const fromWidth = nodeWidth(from)
  const fromHeight = nodeHeight(from)
  const toWidth = nodeWidth(to)
  const toHeight = nodeHeight(to)
  const dx = to.x + toWidth / 2 - (from.x + fromWidth / 2)
  const dy = to.y + toHeight / 2 - (from.y + fromHeight / 2)
  const horizontalSeparation = Math.min(fromWidth, toWidth) * 0.42
  const inferredFromSide: GraphCanvasPinSide = Math.abs(dx) > horizontalSeparation ? (dx >= 0 ? 'right' : 'left') : dy >= 0 ? 'bottom' : 'top'
  const inferredToSide: GraphCanvasPinSide = Math.abs(dx) > horizontalSeparation ? (dx >= 0 ? 'left' : 'right') : dy >= 0 ? 'top' : 'bottom'
  const hintedFromSide = sourceHandleSide(edge?.sourceHandle) ?? inferredFromSide
  const hintedToSide = targetHandleSide(edge?.targetHandle) ?? inferredToSide
  const fromPin = edgePinOffset(from, hintedFromSide, edge?.sourceHandle, 'source')
  const toPin = edgePinOffset(to, hintedToSide, edge?.targetHandle, 'target')
  const fromSide = fromPin.side
  const toSide = toPin.side
  return {
    start: { x: from.x + fromPin.x, y: from.y + fromPin.y },
    end: { x: to.x + toPin.x, y: to.y + toPin.y },
    fromSide,
    toSide,
  }
}

function edgePinOffset(node: GraphCanvasNode, side: GraphCanvasPinSide, handle: string | undefined, role: 'source' | 'target'): NodePin {
  const explicit = handle ? node.pins?.find((pin) => pin.handle === handle) : undefined
  if (explicit) {
    const pin = explicitPinOffset(node, explicit)
    if (pinMatchesRole(pin, role)) return pin
  }
  const width = nodeWidth(node)
  const height = nodeHeight(node)
  if (role === 'source' && handle === 'body') return { side: 'bottom', x: width * 0.44, y: height }
  if (role === 'source' && handle === 'exit') return { side: 'right', x: width, y: height / 2 }
  if (role === 'target' && handle === 'return') return { side: 'bottom', x: width * 0.72, y: height }
  if (role === 'source' && handle?.startsWith('rule:')) {
    const rule = handle.slice(5).toLowerCase()
    const y = rule.includes('false') || rule.includes('clean') || rule.includes('no') ? height * 0.68 : height * 0.34
    return { side: 'right', x: width, y }
  }
  return pinOffset(node, side)
}

function sourceHandleSide(handle?: string): GraphCanvasPinSide | null {
  const pinnedSide = pinHandleSide(handle)
  if (pinnedSide) return pinnedSide
  if (handle === 'body') return 'bottom'
  if (handle === 'exit') return 'right'
  if (handle?.startsWith('rule:')) return 'right'
  return null
}

function targetHandleSide(handle?: string): GraphCanvasPinSide | null {
  const pinnedSide = pinHandleSide(handle)
  if (pinnedSide) return pinnedSide
  if (handle === 'return') return 'bottom'
  return null
}

function pinHandleSide(handle?: string): GraphCanvasPinSide | null {
  const match = handle?.match(/^pin:(top|right|bottom|left)$/)
  return match ? (match[1] as GraphCanvasPinSide) : null
}

function connectionPath(
  points: { start: { x: number; y: number }; end: { x: number; y: number }; fromSide: GraphCanvasPinSide; toSide: GraphCanvasPinSide },
  edge?: GraphCanvasEdge,
) {
  if (edge?.targetHandle === 'return') return returnConnectionPath(points)
  const fromVector = sideVector(points.fromSide)
  const toVector = sideVector(points.toSide)
  const axisDistance = points.fromSide === 'left' || points.fromSide === 'right'
    ? Math.abs(points.end.x - points.start.x)
    : Math.abs(points.end.y - points.start.y)
  const distance = Math.max(90, Math.min(220, axisDistance * 0.48))
  const c1 = { x: points.start.x + fromVector.x * distance, y: points.start.y + fromVector.y * distance }
  const c2 = { x: points.end.x + toVector.x * distance, y: points.end.y + toVector.y * distance }
  return `M ${points.start.x} ${points.start.y} C ${c1.x} ${c1.y}, ${c2.x} ${c2.y}, ${points.end.x} ${points.end.y}`
}

function returnConnectionPath(points: { start: { x: number; y: number }; end: { x: number; y: number }; fromSide: GraphCanvasPinSide; toSide: GraphCanvasPinSide }) {
  const direction = points.start.x >= points.end.x ? 1 : -1
  const distance = Math.max(
    110,
    Math.min(260, Math.abs(points.end.x - points.start.x) * 0.42 + Math.abs(points.end.y - points.start.y) * 0.52),
  )
  const fromVector = sideVector(points.fromSide)
  const c1 = {
    x: points.start.x + (fromVector.x === 0 ? direction * distance * 0.58 : fromVector.x * distance),
    y: points.start.y + fromVector.y * distance * 0.74,
  }
  const c2 = {
    x: points.end.x + direction * distance * 0.56,
    y: points.end.y + distance * 0.72,
  }
  return `M ${points.start.x} ${points.start.y} C ${c1.x} ${c1.y}, ${c2.x} ${c2.y}, ${points.end.x} ${points.end.y}`
}

function sideVector(side: GraphCanvasPinSide) {
  if (side === 'top') return { x: 0, y: -1 }
  if (side === 'right') return { x: 1, y: 0 }
  if (side === 'bottom') return { x: 0, y: 1 }
  return { x: -1, y: 0 }
}

function previewPath() {
  const drag = connectionDrag.value
  if (!drag) return ''
  const endSide = drag.targetId ? drag.targetSide : oppositeSide(drag.sourceSide)
  return connectionPath({ start: drag.start, end: drag.pointer, fromSide: drag.sourceSide, toSide: endSide })
}

function oppositeSide(side: GraphCanvasPinSide): GraphCanvasPinSide {
  if (side === 'top') return 'bottom'
  if (side === 'right') return 'left'
  if (side === 'bottom') return 'top'
  return 'right'
}

function visibleCanvasArea() {
  const rect = canvas.value?.getBoundingClientRect()
  if (!rect || rect.width <= 0 || rect.height <= 0 || typeof window === 'undefined') {
    return { x: 0, y: 0, width: props.canvasSize.width, height: props.canvasSize.height }
  }

  const containerRect = canvas.value?.parentElement?.getBoundingClientRect()
  const visibleLeft = Math.max(0, rect.left, containerRect?.left ?? rect.left)
  const visibleTop = Math.max(0, rect.top, containerRect?.top ?? rect.top)
  const visibleRight = Math.min(window.innerWidth, rect.right, containerRect?.right ?? rect.right)
  const visibleBottom = Math.min(window.innerHeight, rect.bottom, containerRect?.bottom ?? rect.bottom)

  if (visibleRight <= visibleLeft || visibleBottom <= visibleTop) {
    return { x: 0, y: 0, width: props.canvasSize.width, height: props.canvasSize.height }
  }

  const scaleX = props.canvasSize.width / rect.width
  const scaleY = props.canvasSize.height / rect.height
  return {
    x: (visibleLeft - rect.left) * scaleX,
    y: (visibleTop - rect.top) * scaleY,
    width: (visibleRight - visibleLeft) * scaleX,
    height: (visibleBottom - visibleTop) * scaleY,
  }
}

function fitViewportToVisibleNodes() {
  const nodes = props.nodes
  if (nodes.length === 0) {
    viewport.value = { x: 40, y: 36, scale: 1 }
    return
  }

  const minX = Math.min(...nodes.map((node) => node.x))
  const minY = Math.min(...nodes.map((node) => node.y))
  const maxX = Math.max(...nodes.map((node) => node.x + nodeWidth(node)))
  const maxY = Math.max(...nodes.map((node) => node.y + nodeHeight(node)))
  const boundsWidth = Math.max(maxX - minX, 1)
  const boundsHeight = Math.max(maxY - minY, 1)
  const visibleArea = visibleCanvasArea()
  const availableWidth = Math.max(visibleArea.width - props.fitPadding * 2, 1)
  const availableHeight = Math.max(visibleArea.height - props.fitPadding * 2, 1)
  const scale = Math.min(1, availableWidth / boundsWidth, availableHeight / boundsHeight)
  const centerX = minX + boundsWidth / 2
  const centerY = minY + boundsHeight / 2

  viewport.value = {
    scale,
    x: visibleArea.x + visibleArea.width / 2 - centerX * scale,
    y: visibleArea.y + visibleArea.height / 2 - centerY * scale,
  }
}

function fitViewportAfterRender() {
  void nextTick(() => fitViewportToVisibleNodes())
}

function resetViewport() {
  viewport.value = { x: 40, y: 36, scale: 1 }
  fitViewportAfterRender()
}

function removeEdge(edge: GraphCanvasEdge) {
  if (props.readonly || edge.removable === false) return
  emit('edgeRemove', edge)
}

onMounted(() => {
  if (props.fitOnMount) fitViewportAfterRender()
})

onBeforeUnmount(() => {
  window.removeEventListener('pointermove', updateConnectionDrag)
  window.removeEventListener('pointerup', finishConnectionDrag)
  window.removeEventListener('pointercancel', finishConnectionDrag)
})

watch(
  viewport,
  (next) => emit('viewportChange', { ...next }),
  { deep: true, immediate: true },
)

defineExpose({
  fitViewportToVisibleNodes,
  fitViewportAfterRender,
  resetViewport,
  viewport,
})
</script>

<template>
  <div class="graph-canvas-foundation">
    <svg
      ref="canvas"
      :viewBox="`0 0 ${canvasDimensions.width} ${canvasDimensions.height}`"
      role="img"
      @pointerdown="beginPan"
      @pointermove="onPointerMove"
      @pointerup="endPointer"
      @pointercancel="endPointer"
      @wheel="onWheel"
    >
      <defs>
        <slot name="defs" />
        <linearGradient
          v-for="edge in edges"
          :id="edgeGradient(edge).id"
          :key="edgeGradient(edge).id"
          gradientUnits="userSpaceOnUse"
          :x1="edgeGradient(edge).x1"
          :y1="edgeGradient(edge).y1"
          :x2="edgeGradient(edge).x2"
          :y2="edgeGradient(edge).y2"
        >
          <stop offset="0%" :stop-color="edgeGradient(edge).fromColor" />
          <stop offset="100%" :stop-color="edgeGradient(edge).toColor" />
        </linearGradient>
      </defs>
      <g class="graph-world" :transform="transform">
        <g class="graph-edges">
          <g
            v-for="edge in edges"
            :key="edgeKey(edge)"
            class="graph-edge"
            :class="edge.classes"
            @click.stop="emit('edgeSelect', edge)"
          >
            <path :d="edgePath(edge)" :style="{ stroke: `url(#${edgeGradientId(edge)})`, strokeDasharray: edge.dashed ? '6 4' : undefined }" />
            <slot name="edge-label" :edge="edge" :midpoint="edgeMidpoint(edge)" />
            <g
              v-if="!readonly && edge.removable !== false"
              class="graph-edge-remove"
              :transform="`translate(${edgeMidpoint(edge).x}, ${edgeMidpoint(edge).y})`"
              @click.stop="removeEdge(edge)"
            >
              <circle r="12" />
              <text x="0" y="4">{{ t('graphEdgeRemove') }}</text>
            </g>
          </g>
        </g>
        <path v-if="connectionDrag" class="graph-preview-edge" :d="previewPath()" />
        <g class="graph-nodes">
          <g
            v-for="node in nodes"
            :key="node.id"
            :class="[
              'graph-node',
              node.status ? `status-${node.status}` : '',
              ...(node.classes ?? []),
              {
                'semantic-pins': Boolean(node.pins?.length),
                focus: node.id === focusId || node.id === selectedNode,
                missing: node.disabled,
                connecting: node.id === connectionDrag?.sourceId,
                'snap-target': node.id === connectionDrag?.targetId,
                saving: node.id === savingId,
                readonly,
              },
            ]"
            :style="{ '--graph-status-color': node.color ?? undefined }"
            :transform="`translate(${node.x}, ${node.y})`"
            @pointerdown.stop="beginNodeDrag($event, node)"
            @dblclick.stop="openNode(node)"
          >
            <slot
              name="node"
              :node="node"
              :width="nodeWidth(node)"
              :height="nodeHeight(node)"
              :selected="node.id === focusId || node.id === selectedNode"
              :connecting="node.id === connectionDrag?.sourceId"
              :snap-target="node.id === connectionDrag?.targetId"
            />
            <g v-if="semanticPins(node).length" class="graph-semantic-pins">
              <g
                v-for="pin in semanticPins(node)"
                :key="pinKey(pin)"
                class="graph-semantic-pin"
                :class="[`pin-${pin.role ?? 'both'}`, `pin-side-${pin.side}`, pin.category ? `pin-category-${pin.category}` : 'pin-category-exec']" 
                :transform="pinTransform(pin)"
              >
                <!-- Exec Pin -->
                <polygon
                  v-if="!isDataPin(pin)"
                  class="graph-exec-pin-glyph"
                  points="-8,-8 9,0 -8,8"
                />
                <!-- Data Pin -->
                <circle
                  v-else
                  class="graph-data-pin-glyph"
                  r="5"
                  :fill="pin.color ?? 'var(--pin-data-fill)'"
                />
                <text
                  v-if="visiblePinLabel(pin)"
                  :x="pinLabelX(pin)"
                  :y="pinLabelY(pin)"
                  :text-anchor="pinLabelAnchor(pin)"
                >
                  {{ visiblePinLabel(pin) }}
                </text>
              </g>
            </g>
            <g
              v-for="pin in nodePins(node)"
              v-show="!readonly && !node.disabled && node.connectable !== false"
              :key="pinKey(pin)"
              class="graph-node-pin"
              :class="{
                active: node.id === connectionDrag?.targetId && pin.handle === connectionDrag?.targetHandle,
                target: (pin.role ?? 'both') === 'target',
              }"
              :data-side="pin.side"
              :data-handle="pin.handle"
              :transform="pinTransform(pin)"
              @pointerdown.stop.prevent="beginConnection($event, node, pin)"
            >
              <slot
                name="pin"
                :node="node"
                :side="pin.side"
                :pin="pin"
                :active="node.id === connectionDrag?.targetId && pin.handle === connectionDrag?.targetHandle"
              >
                <circle class="graph-node-pin-hit" r="24" />
                <circle r="11" />
              </slot>
            </g>
          </g>
        </g>
      </g>
    </svg>
  </div>
</template>

<style scoped>
.graph-semantic-pin {
  --pin-exec-fill: var(--bb-surface);
  --pin-exec-stroke: color-mix(in srgb, var(--graph-status-color, var(--bb-text-muted)) 58%, var(--bb-text-muted));
  --pin-data-fill: var(--bb-text-muted);
}

.graph-exec-pin-glyph {
  fill: var(--pin-exec-fill);
  stroke: var(--pin-exec-stroke);
  stroke-linejoin: round;
  stroke-width: 1.6;
  filter: drop-shadow(0 2px 4px color-mix(in srgb, var(--pin-exec-stroke) 24%, transparent));
}

.graph-data-pin-glyph {
  stroke: var(--bb-surface);
  stroke-width: 1.8;
  filter: drop-shadow(0 2px 5px rgba(15, 23, 42, 0.16));
}
</style>
