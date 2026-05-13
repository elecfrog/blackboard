<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import GraphCanvas, {
  type GraphCanvasConnection,
  type GraphCanvasEdge,
  type GraphCanvasNode,
  type GraphCanvasNodeMove,
  type GraphCanvasViewport,
} from '@/components/GraphCanvas.vue'
import { patchTicket, type BlackboardTicket } from '@/data/tickets'
import { t, ticketStatusLabel } from '@/i18n'

const props = defineProps<{
  project: string
  tickets: BlackboardTicket[]
  visibleTicketIds?: string[]
  focusId?: string
  savingId?: string | null
}>()

const emit = defineEmits<{
  open: [id: string]
  'dependencies-change': [id: string, dependencies: string[]]
  dependenciesChange: [id: string, dependencies: string[]]
}>()

interface GraphNode {
  id: string
  title: string
  lane: string
  status: string
  missing: boolean
  titleLines: string[]
}

interface GraphEdge {
  from: string
  to: string
}

interface CanvasNode extends GraphNode {
  x: number
  y: number
  width: number
  height: number
  color: string
  disabled: boolean
}

const nodeWidth = 320
const nodeHeight = 102
const laneGap = 170
const rowGap = 42
const worldSize = { width: 1200, height: 760 }
const titleInset = 20
const titleLineUnits = [24, 31]
const canvasRef = ref<InstanceType<typeof GraphCanvas> | null>(null)
const graphViewport = ref<GraphCanvasViewport>({ x: 40, y: 36, scale: 1 })
const positions = ref<Record<string, { x: number; y: number }>>({})
const transientMessage = ref('')
const dependencyOverrides = ref<Record<string, string[]>>({})
const localSavingId = ref('')

const graphStatusColors: Record<string, string> = {
  todo: '#64748b',
  in_progress: '#2563eb',
  blocked: '#dc2626',
  review: '#d97706',
  done: '#059669',
  archived: '#78716c',
  missing: '#94a3b8',
}

const layoutKey = computed(() => `blackboard.graph-layout.${props.project}`)
const visibleTicketIdSet = computed(() => new Set(props.visibleTicketIds ?? props.tickets.map((ticket) => ticket.id)))
const visibleTickets = computed(() => props.tickets.filter((ticket) => visibleTicketIdSet.value.has(ticket.id)))
const allTicketIdSet = computed(() => new Set(props.tickets.map((ticket) => ticket.id)))
const activeSavingId = computed(() => props.savingId || localSavingId.value || null)

const hasVisibilityFilter = computed(() => {
  if (!props.visibleTicketIds) return false
  if (props.visibleTicketIds.length !== props.tickets.length) return true
  return props.tickets.some((ticket) => !visibleTicketIdSet.value.has(ticket.id))
})

function titleLines(title: string) {
  const clean = title.replace(/\s+/g, ' ').trim()
  const lines: string[] = []
  let rest = clean
  while (rest.length > 0 && lines.length < 2) {
    const maxUnits = titleLineUnits[lines.length] ?? titleLineUnits[titleLineUnits.length - 1]
    if (visualUnits(rest) <= maxUnits) {
      lines.push(rest)
      rest = ''
      break
    }
    let splitAt = fitTitleIndex(rest, maxUnits)
    const spaceBefore = rest.lastIndexOf(' ', splitAt)
    if (spaceBefore >= 8) splitAt = spaceBefore
    lines.push(rest.slice(0, splitAt).trim())
    rest = rest.slice(splitAt).trim()
  }
  if (rest.length > 0 && lines.length > 0) {
    lines[lines.length - 1] = ellipsizeTitle(lines[lines.length - 1], titleLineUnits[lines.length - 1] ?? 28)
  }
  return lines.length > 0 ? lines : ['-']
}

function visualUnits(value: string) {
  return [...value].reduce((sum, char) => {
    if (char === ' ') return sum + 0.55
    return sum + (/[\u3400-\u9fff\uff00-\uffef]/.test(char) ? 1.9 : 1)
  }, 0)
}

function fitTitleIndex(value: string, maxUnits: number) {
  let units = 0
  let index = 0
  for (const char of value) {
    const width = char === ' ' ? 0.55 : /[\u3400-\u9fff\uff00-\uffef]/.test(char) ? 1.9 : 1
    if (index > 0 && units + width > maxUnits) break
    units += width
    index += char.length
  }
  return Math.max(index, 1)
}

function ellipsizeTitle(value: string, maxUnits: number) {
  let result = value
  while (result.length > 0 && visualUnits(`${result}...`) > maxUnits) {
    result = result.slice(0, -1)
  }
  return `${result.trimEnd()}...`
}

const graph = computed(() => {
  const nodes = new Map<string, GraphNode>()
  const edges: GraphEdge[] = []
  for (const ticket of visibleTickets.value) {
    nodes.set(ticket.id, {
      id: ticket.id,
      title: ticket.title,
      lane: ticket.lane,
      status: ticket.status,
      missing: false,
      titleLines: titleLines(ticket.title),
    })
  }
  for (const ticket of visibleTickets.value) {
    for (const dep of dependenciesFor(ticket)) {
      if (!visibleTicketIdSet.value.has(dep)) {
        if (allTicketIdSet.value.has(dep)) continue
        nodes.set(dep, {
          id: dep,
          title: t('missingTicket'),
          lane: '-',
          status: 'missing',
          missing: true,
          titleLines: [t('missingTicket')],
        })
      } else if (!nodes.has(dep)) {
        nodes.set(dep, {
          id: dep,
          title: t('missingTicket'),
          lane: '-',
          status: 'missing',
          missing: true,
          titleLines: [t('missingTicket')],
        })
      }
      edges.push({ from: dep, to: ticket.id })
    }
  }
  return { nodes: [...nodes.values()].sort((a, b) => a.id.localeCompare(b.id)), edges }
})

const graphRelations = computed(() => {
  const parents = new Map<string, string[]>()
  const children = new Map<string, string[]>()
  for (const node of graph.value.nodes) {
    parents.set(node.id, [])
    children.set(node.id, [])
  }
  for (const edge of graph.value.edges) {
    parents.get(edge.to)?.push(edge.from)
    children.get(edge.from)?.push(edge.to)
  }
  return { parents, children }
})

const dependencyMap = computed(() => {
  const map = new Map<string, string[]>()
  for (const ticket of props.tickets) map.set(ticket.id, dependenciesFor(ticket))
  return map
})

function dependenciesFor(ticket: BlackboardTicket) {
  return dependencyOverrides.value[ticket.id] ?? [...ticket.dependencies]
}

const cycle = computed(() => {
  const adjacency = new Map<string, string[]>()
  for (const node of graph.value.nodes) adjacency.set(node.id, [])
  for (const edge of graph.value.edges) adjacency.get(edge.to)?.push(edge.from)
  return findCycle(adjacency)
})

function autoPositions() {
  const deps = graphRelations.value.parents
  const children = graphRelations.value.children
  const componentGap = 110

  const memo = new Map<string, number>()
  function levelFor(id: string, seen = new Set<string>()): number {
    if (memo.has(id)) return memo.get(id) ?? 0
    if (seen.has(id)) return 0
    seen.add(id)
    const level = Math.max(0, ...((deps.get(id) ?? []).map((dep) => levelFor(dep, seen) + 1)))
    seen.delete(id)
    memo.set(id, level)
    return level
  }

  const byLevel = new Map<number, GraphNode[]>()
  const levelById = new Map<string, number>()
  for (const node of graph.value.nodes) {
    const level = levelFor(node.id)
    levelById.set(node.id, level)
    const list = byLevel.get(level) ?? []
    list.push(node)
    byLevel.set(level, list)
  }

  const levels = [...byLevel.keys()].sort((a, b) => a - b)
  const order = new Map<string, number>()
  for (const level of levels) {
    const nodes = byLevel.get(level) ?? []
    nodes.sort((a, b) => a.id.localeCompare(b.id))
    nodes.forEach((node, index) => order.set(node.id, index))
  }

  const neighborAverage = (ids: string[]) => {
    const values = ids.map((id) => order.get(id)).filter((value): value is number => value !== undefined)
    if (values.length === 0) return null
    return values.reduce((sum, value) => sum + value, 0) / values.length
  }

  for (let sweep = 0; sweep < 3; sweep += 1) {
    for (const level of levels) {
      if (level === levels[0]) continue
      const nodes = byLevel.get(level) ?? []
      nodes.sort((a, b) => {
        const aPressure = neighborAverage(deps.get(a.id) ?? []) ?? order.get(a.id) ?? 0
        const bPressure = neighborAverage(deps.get(b.id) ?? []) ?? order.get(b.id) ?? 0
        return aPressure - bPressure || a.id.localeCompare(b.id)
      })
      nodes.forEach((node, index) => order.set(node.id, index))
    }

    const lastLevel = levels[levels.length - 1]
    for (const level of [...levels].reverse()) {
      if (level === lastLevel) continue
      const nodes = byLevel.get(level) ?? []
      nodes.sort((a, b) => {
        const aPressure = neighborAverage(children.get(a.id) ?? []) ?? order.get(a.id) ?? 0
        const bPressure = neighborAverage(children.get(b.id) ?? []) ?? order.get(b.id) ?? 0
        return aPressure - bPressure || a.id.localeCompare(b.id)
      })
      nodes.forEach((node, index) => order.set(node.id, index))
    }
  }

  const yById = new Map<string, number>()
  const desiredY = new Map<string, number>()
  const yForRow = (index: number) => 84 + index * (nodeHeight + rowGap)
  for (const level of levels) {
    const nodes = [...(byLevel.get(level) ?? [])].sort((a, b) => (order.get(a.id) ?? 0) - (order.get(b.id) ?? 0))
    nodes.forEach((node, index) => yById.set(node.id, yForRow(index)))
  }

  const averageChildCenter = (id: string) => {
    const childCenters = (children.get(id) ?? [])
      .map((childId) => yById.get(childId))
      .filter((value): value is number => value !== undefined)
      .map((y) => y + nodeHeight / 2)
    if (childCenters.length === 0) return null
    return childCenters.reduce((sum, value) => sum + value, 0) / childCenters.length
  }

  for (const level of [...levels].reverse()) {
    const nodes = [...(byLevel.get(level) ?? [])].sort((a, b) => (order.get(a.id) ?? 0) - (order.get(b.id) ?? 0))
    for (const node of nodes) {
      const centered = averageChildCenter(node.id)
      desiredY.set(node.id, centered === null ? yById.get(node.id) ?? 84 : centered - nodeHeight / 2)
    }
    let previous = -Infinity
    for (const node of [...nodes].sort((a, b) => (desiredY.get(a.id) ?? 0) - (desiredY.get(b.id) ?? 0))) {
      const desired = desiredY.get(node.id) ?? 84
      const y = Math.max(desired, previous + nodeHeight + rowGap)
      yById.set(node.id, y)
      previous = y
    }
  }

  const isSoleSuccessor = (id: string) => {
    const parentIds = deps.get(id) ?? []
    return parentIds.length > 0 && parentIds.every((parentId) => (children.get(parentId) ?? []).length === 1)
  }

  for (const level of levels) {
    const nodes = [...(byLevel.get(level) ?? [])].sort((a, b) => (order.get(a.id) ?? 0) - (order.get(b.id) ?? 0))
    for (const node of nodes) {
      const parentCenters = (deps.get(node.id) ?? [])
        .map((parentId) => yById.get(parentId))
        .filter((value): value is number => value !== undefined)
        .map((y) => y + nodeHeight / 2)
      const centered = parentCenters.length > 0
        ? parentCenters.reduce((sum, value) => sum + value, 0) / parentCenters.length
        : null
      desiredY.set(node.id, isSoleSuccessor(node.id) && centered !== null ? centered - nodeHeight / 2 : yById.get(node.id) ?? 84)
    }
    let previous = -Infinity
    for (const node of [...nodes].sort((a, b) => (desiredY.get(a.id) ?? 0) - (desiredY.get(b.id) ?? 0))) {
      const desired = desiredY.get(node.id) ?? 84
      const y = Math.max(desired, previous + nodeHeight + rowGap)
      yById.set(node.id, y)
      previous = y
    }
  }

  const componentIds = new Map<string, number>()
  const components: string[][] = []
  const byId = new Map(graph.value.nodes.map((node) => [node.id, node]))
  for (const node of graph.value.nodes) {
    if (componentIds.has(node.id)) continue
    const index = components.length
    const queue = [node.id]
    const ids: string[] = []
    componentIds.set(node.id, index)
    while (queue.length > 0) {
      const id = queue.shift()
      if (!id) continue
      ids.push(id)
      const linked = [...(deps.get(id) ?? []), ...(children.get(id) ?? [])]
      for (const linkedId of linked) {
        if (!byId.has(linkedId) || componentIds.has(linkedId)) continue
        componentIds.set(linkedId, index)
        queue.push(linkedId)
      }
    }
    components.push(ids)
  }

  const compactComponent = (ids: string[]) => {
    const localLevels = [...new Set(ids.map((id) => levelById.get(id) ?? 0))].sort((a, b) => a - b)
    const nodesAtLevel = (level: number) => ids
      .filter((id) => (levelById.get(id) ?? 0) === level)
      .map((id) => byId.get(id))
      .filter((node): node is GraphNode => node !== undefined)

    const placeCompact = (nodes: GraphNode[], desiredTop: Map<string, number> | null) => {
      const sorted = [...nodes].sort((a, b) => {
        const aDesired = desiredTop?.get(a.id)
        const bDesired = desiredTop?.get(b.id)
        if (aDesired !== undefined || bDesired !== undefined) {
          return (aDesired ?? yById.get(a.id) ?? 84) - (bDesired ?? yById.get(b.id) ?? 84)
            || (order.get(a.id) ?? 0) - (order.get(b.id) ?? 0)
            || a.id.localeCompare(b.id)
        }
        return (order.get(a.id) ?? 0) - (order.get(b.id) ?? 0) || a.id.localeCompare(b.id)
      })
      const groupHeight = sorted.length * nodeHeight + Math.max(0, sorted.length - 1) * rowGap
      const desiredCenters = sorted
        .map((node) => desiredTop?.get(node.id))
        .filter((value): value is number => value !== undefined)
        .map((y) => y + nodeHeight / 2)
      const center = desiredCenters.length > 0
        ? desiredCenters.reduce((sum, value) => sum + value, 0) / desiredCenters.length
        : 84 + groupHeight / 2
      const desiredTops = sorted.map((node) => desiredTop?.get(node.id) ?? yById.get(node.id) ?? 84)
      const start = desiredCenters.length === sorted.length
        ? center - groupHeight / 2
        : desiredCenters.length > 0
          ? Math.min(...desiredTops)
          : 84
      sorted.forEach((node, index) => yById.set(node.id, start + index * (nodeHeight + rowGap)))
    }

    for (const level of localLevels) placeCompact(nodesAtLevel(level), null)

    for (let sweep = 0; sweep < 3; sweep += 1) {
      for (const level of [...localLevels].reverse()) {
        const desired = new Map<string, number>()
        for (const node of nodesAtLevel(level)) {
          const childIds = children.get(node.id) ?? []
          if (childIds.length === 1 && (deps.get(childIds[0]) ?? []).length > 1) continue
          const childCenters = childIds
            .map((childId) => yById.get(childId))
            .filter((value): value is number => value !== undefined)
            .map((y) => y + nodeHeight / 2)
          if (childCenters.length > 0) {
            desired.set(node.id, childCenters.reduce((sum, value) => sum + value, 0) / childCenters.length - nodeHeight / 2)
          }
        }
        if (desired.size > 0) placeCompact(nodesAtLevel(level), desired)
      }

      for (const level of localLevels) {
        const desired = new Map<string, number>()
        for (const node of nodesAtLevel(level)) {
          const parentCenters = (deps.get(node.id) ?? [])
            .map((parentId) => yById.get(parentId))
            .filter((value): value is number => value !== undefined)
            .map((y) => y + nodeHeight / 2)
          if (parentCenters.length > 0) {
            desired.set(node.id, parentCenters.reduce((sum, value) => sum + value, 0) / parentCenters.length - nodeHeight / 2)
          }
        }
        if (desired.size > 0) placeCompact(nodesAtLevel(level), desired)
      }
    }
  }

  for (const ids of components) compactComponent(ids)

  let nextComponentY = 84
  for (const ids of components.sort((a, b) => a[0].localeCompare(b[0]))) {
    const ys = ids.map((id) => yById.get(id) ?? 84)
    const minY = Math.min(...ys)
    const maxY = Math.max(...ys.map((y) => y + nodeHeight))
    const offset = nextComponentY - minY
    for (const id of ids) yById.set(id, (yById.get(id) ?? 84) + offset)
    nextComponentY += maxY - minY + componentGap
  }

  const next: Record<string, { x: number; y: number }> = {}
  for (const [level, nodes] of [...byLevel.entries()].sort((a, b) => a[0] - b[0])) {
    nodes.forEach((node) => {
      next[node.id] = {
        x: 72 + level * (nodeWidth + laneGap),
        y: yById.get(node.id) ?? 84,
      }
    })
  }
  return next
}

const canvasNodes = computed<CanvasNode[]>(() => {
  const fallback = autoPositions()
  return graph.value.nodes.map((node) => {
    const position = positions.value[node.id] ?? fallback[node.id] ?? { x: 72, y: 84 }
    return {
      ...node,
      x: position.x,
      y: position.y,
      width: nodeWidth,
      height: nodeHeight,
      color: statusColor(node.status),
      disabled: node.missing,
    }
  })
})

const canvasSize = computed(() => worldSize)
const graphEdges = computed<GraphCanvasEdge[]>(() =>
  graph.value.edges.map((edge) => ({
    id: `${edge.from}-${edge.to}`,
    from: edge.from,
    to: edge.to,
  })),
)

function fitViewportAfterRender() {
  void nextTick(() => canvasRef.value?.fitViewportToVisibleNodes())
}

function loadLayout() {
  if (typeof window === 'undefined') {
    positions.value = autoPositions()
    fitViewportAfterRender()
    return
  }
  if (hasVisibilityFilter.value) {
    positions.value = autoPositions()
    fitViewportAfterRender()
    return
  }
  try {
    const saved = window.localStorage.getItem(layoutKey.value)
    positions.value = saved ? JSON.parse(saved) : autoPositions()
  } catch {
    positions.value = autoPositions()
  }
  fitViewportAfterRender()
}

function saveLayout() {
  if (typeof window === 'undefined') return
  window.localStorage.setItem(layoutKey.value, JSON.stringify(positions.value))
}

function resetLayout() {
  positions.value = autoPositions()
  saveLayout()
  fitViewportAfterRender()
}

async function persistDependencies(targetId: string, dependencies: string[]) {
  const previous = dependencyOverrides.value[targetId]
  dependencyOverrides.value = { ...dependencyOverrides.value, [targetId]: dependencies }
  localSavingId.value = targetId
  try {
    await patchTicket(props.project, targetId, { depends_on: dependencies })
    emit('dependencies-change', targetId, dependencies)
    emit('dependenciesChange', targetId, dependencies)
    flash(t('dependencySaved'))
  } catch (err) {
    const next = { ...dependencyOverrides.value }
    if (previous) next[targetId] = previous
    else delete next[targetId]
    dependencyOverrides.value = next
    flash(`${t('dependencySaveFailed')}: ${err instanceof Error ? err.message : String(err)}`)
  } finally {
    localSavingId.value = ''
  }
}

function addDependency(targetId: string, sourceId: string) {
  const current = dependencyMap.value.get(targetId) ?? []
  if (current.includes(sourceId)) {
    flash(t('dependencySaved'))
    return
  }
  const next = [...current, sourceId]
  if (wouldCycle(targetId, next)) {
    flash(`${t('dependencyCycle')} ${sourceId} -> ${targetId}`)
    return
  }
  void persistDependencies(targetId, next)
}

function removeDependency(targetId: string, sourceId: string, event?: MouseEvent) {
  event?.stopPropagation()
  const current = dependencyMap.value.get(targetId) ?? []
  const next = current.filter((id) => id !== sourceId)
  void persistDependencies(targetId, next)
}

function handleNodeMove(move: GraphCanvasNodeMove) {
  positions.value = {
    ...positions.value,
    [move.id]: {
      x: move.x,
      y: move.y,
    },
  }
}

function handleNodeMoveEnd(move: GraphCanvasNodeMove) {
  handleNodeMove(move)
  saveLayout()
}

function handleConnectionCreate(connection: GraphCanvasConnection) {
  addDependency(connection.targetId, connection.sourceId)
}

function handleEdgeRemove(edge: GraphCanvasEdge) {
  removeDependency(edge.to, edge.from)
}

function wouldCycle(targetId: string, nextDeps: string[]) {
  const adjacency = new Map<string, string[]>()
  for (const node of graph.value.nodes) adjacency.set(node.id, [...(dependencyMap.value.get(node.id) ?? [])])
  adjacency.set(targetId, nextDeps)
  return findCycle(adjacency).length > 0
}

function findCycle(adjacency: Map<string, string[]>) {
  const state = new Map<string, number>()
  const stack: string[] = []
  function visit(id: string): string[] | null {
    const mark = state.get(id) ?? 0
    if (mark === 2) return null
    if (mark === 1) {
      const start = stack.indexOf(id)
      return [...stack.slice(start), id]
    }
    state.set(id, 1)
    stack.push(id)
    for (const next of adjacency.get(id) ?? []) {
      const found = visit(next)
      if (found) return found
    }
    stack.pop()
    state.set(id, 2)
    return null
  }
  for (const id of adjacency.keys()) {
    const found = visit(id)
    if (found) return found
  }
  return []
}

function flash(message: string) {
  transientMessage.value = message
  window.setTimeout(() => {
    if (transientMessage.value === message) transientMessage.value = ''
  }, 1600)
}

function statusColor(status: string) {
  return graphStatusColors[status] ?? graphStatusColors.todo
}

onMounted(() => {
  loadLayout()
})

watch(() => props.project, loadLayout)
watch(
  () => visibleTickets.value.map((ticket) => ticket.id).join('|'),
  () => {
    const next = autoPositions()
    for (const id of Object.keys(next)) {
      if (!graph.value.nodes.some((node) => node.id === id)) delete next[id]
    }
    positions.value = next
    fitViewportAfterRender()
  },
)

watch(
  () => props.tickets.map((ticket) => `${ticket.id}:${ticket.dependencies.join(',')}`).join('|'),
  () => {
    dependencyOverrides.value = {}
  },
)
</script>

<template>
  <section class="ticket-graph-workspace">
    <div class="graph-toolbar">
      <span>{{ t('dependencyConnectHint') }}</span>
      <div class="graph-toolbar-actions">
        <span>{{ graph.nodes.length }} {{ t('tickets') }} · {{ graph.edges.length }} {{ t('dependencies') }}</span>
        <span>{{ Math.round(graphViewport.scale * 100) }}%</span>
        <button type="button" class="bb-top-action-button" @click="resetLayout">{{ t('resetLayout') }}</button>
      </div>
    </div>

    <div v-if="cycle.length > 0" class="bb-mutation-error">
      {{ t('dependencyCycle') }} {{ cycle.join(' -> ') }}
    </div>
    <div v-if="transientMessage" class="graph-toast">{{ transientMessage }}</div>

    <GraphCanvas
      ref="canvasRef"
      class="ticket-graph-canvas"
      :nodes="canvasNodes"
      :edges="graphEdges"
      :canvas-size="canvasSize"
      :default-node-width="nodeWidth"
      :default-node-height="nodeHeight"
      :focus-id="focusId"
      :saving-id="activeSavingId"
      @viewport-change="graphViewport = $event"
      @node-open="emit('open', $event)"
      @node-move="handleNodeMove"
      @node-move-end="handleNodeMoveEnd"
      @connection-create="handleConnectionCreate"
      @edge-remove="handleEdgeRemove"
    >
      <template #defs>
        <linearGradient id="rdg-node-fill" x1="0" x2="1" y1="0" y2="1">
          <stop offset="0%" stop-color="var(--bb-surface)" />
          <stop offset="100%" stop-color="var(--bb-surface-soft)" />
        </linearGradient>
      </template>

      <template #node="{ node, width, height }">
        <rect :width="width" :height="height" rx="8" />
        <rect class="graph-node-accent" width="4" :height="height" rx="2" />
        <text x="18" y="27" class="graph-node-id">{{ node.id }}</text>
        <g
          class="graph-node-status"
          :class="`status-${node.status || 'unknown'}`"
          :transform="`translate(${width - 96}, 15)`"
        >
          <rect width="72" height="24" rx="12" />
          <text x="36" y="16">{{ ticketStatusLabel(node.status || '') }}</text>
        </g>
        <text :x="titleInset" y="55" class="graph-node-title">
          <tspan
            v-for="(line, index) in node.titleLines ?? []"
            :key="line"
            :x="titleInset"
            :dy="index === 0 ? 0 : 18"
          >
            {{ line }}
          </tspan>
        </text>
        <text :x="titleInset" y="88" class="graph-node-meta">{{ node.lane }}</text>
      </template>
    </GraphCanvas>
  </section>
</template>
