<script setup lang="ts">
import { computed, defineAsyncComponent, nextTick, onBeforeUnmount, onMounted, ref, watch, type Component } from 'vue'
import { useRouter } from 'vue-router'
import { Badge } from 'tdesign-vue-next/es/badge'
import {
  Bot,
  Book,
  Columns3,
  GitFork,
  Inbox,
  Languages,
  ListTodo,
  Moon,
  RefreshCw,
  Search,
  Settings as SettingsIcon,
  SlidersHorizontal,
  StickyNote,
  Sun,
  Workflow,
} from 'lucide-vue-next'
import BbDropdown, { type BbDropdownOption } from '@/components/BbDropdown.vue'
import ProjectSwitcher from '@/components/ProjectSwitcher.vue'
import TicketCard from '@/components/TicketCard.vue'
import blackboardIconUrl from '@/assets/blackboard-icon.png'
import { loadProjectAgents, type ProjectAgentProfile } from '@/data/agents'
import type {
  BlackboardPayload,
  BlackboardTicket,
  BoardSummary,
  LaneDef,
  TicketAttachment,
  TicketStatus,
  TicketWriteResult,
} from '@/data/tickets'
import {
  deprecateTicket,
  isOpenTicketStatus,
  loadBlackboardData,
  loadBoardSummary,
  loadInboxNotes,
  loadLanes,
  patchProjectBoardView,
  patchTicket,
  resolveLaneMeta,
  ticketStatusOrder,
} from '@/data/tickets'
import { locale, localeLabel, t, ticketStatusLabel, toggleLocale } from '@/i18n'
import {
  themeLabel,
  themeMode,
  toggleTheme,
} from '@/theme'
import {
  dependenciesFromExtra,
  hiddenStatusesFromVisible,
  visibleStatusesFromHidden,
} from './boardViewUtils'

const AgentConnectorPanel = defineAsyncComponent(() => import('@/components/AgentConnectorPanel.vue'))
const AgentWorkbench = defineAsyncComponent(() => import('@/components/AgentWorkbench.vue'))
const IdeaCanvasPanel = defineAsyncComponent(() => import('@/components/IdeaCanvasPanel.vue'))
const InboxPanel = defineAsyncComponent(() => import('@/components/InboxPanel.vue'))
const LaneManager = defineAsyncComponent(() => import('@/components/LaneManager.vue'))
const TicketDependencyGraph = defineAsyncComponent(() => import('@/components/TicketDependencyGraph.vue'))
const TicketDetailPanel = defineAsyncComponent(() => import('@/components/TicketDetailPanel.vue'))
const TaskGraphCatalogPanel = defineAsyncComponent(() => import('@/components/TaskGraphCatalogPanel.vue'))
const WikiPanel = defineAsyncComponent(() => import('@/components/WikiPanel.vue'))

type WorkspaceKey = 'tickets' | 'taskGraphs' | 'inbox' | 'ideaCanvas' | 'agents' | 'settings' | 'wiki'
type WorkspaceIconKey = WorkspaceKey
type TicketViewMode = 'kanban' | 'graph' | 'list'

const props = defineProps<{
  project: string
  section?: WorkspaceKey
  ticketView?: TicketViewMode
  id?: string
  path?: string
  focus?: string
  taskGraphScope?: string
  taskGraphId?: string
  taskGraphMode?: string
}>()

const router = useRouter()

const payload = ref<BlackboardPayload | null>(null)
const summary = ref<BoardSummary | null>(null)
const lanes = ref<LaneDef[]>([])
const inboxNoteCount = ref(0)
const projectAgents = ref<ProjectAgentProfile[]>([])
const showLaneManager = ref(false)
const loading = ref(true)
const error = ref('')
const query = ref('')
const selectedLane = ref('all')
const activeWorkspace = ref<WorkspaceKey>('tickets')
const activeTicketView = ref<TicketViewMode>('kanban')
const selectedTicketId = ref<string | null>(null)
const detailReturnWorkspace = ref<WorkspaceKey | null>(null)
const draggingTicketId = ref<string | null>(null)
const dragOverStatus = ref<TicketStatus | null>(null)
const movingTicketId = ref<string | null>(null)
const assigneeSavingId = ref<string | null>(null)
const statusSavingId = ref<string | null>(null)
const dependencySavingId = ref<string | null>(null)
const attachmentsSavingId = ref<string | null>(null)
const deprecatingTicketId = ref<string | null>(null)
const boardViewSaving = ref(false)
const mutationError = ref('')
const kanbanArea = ref<HTMLElement | null>(null)
const kanbanTopScroller = ref<HTMLElement | null>(null)
const kanbanScrollWidth = ref(0)

const statusOrder = [...ticketStatusOrder]
const visibleStatusColumns = ref<TicketStatus[]>([...statusOrder])
const workspaceIconComponents: Record<WorkspaceIconKey, Component> = {
  tickets: ListTodo,
  taskGraphs: Workflow,
  inbox: Inbox,
  ideaCanvas: StickyNote,
  agents: Bot,
  settings: SettingsIcon,
  wiki: Book,
}
let kanbanResizeObserver: ResizeObserver | null = null
let syncingKanbanScroll = false
let boardViewSaveSeq = 0

function applyBoardViewSettings(hiddenStatuses: readonly string[] = []) {
  visibleStatusColumns.value = visibleStatusesFromHidden(hiddenStatuses)
}

function updateKanbanScrollWidth() {
  const area = kanbanArea.value
  if (!area) {
    kanbanScrollWidth.value = 0
    return
  }
  kanbanScrollWidth.value = area.scrollWidth
  if (kanbanTopScroller.value) {
    kanbanTopScroller.value.scrollLeft = area.scrollLeft
  }
}

function attachKanbanResizeObserver() {
  kanbanResizeObserver?.disconnect()
  kanbanResizeObserver = null

  const area = kanbanArea.value
  if (!area) {
    kanbanScrollWidth.value = 0
    return
  }

  if (typeof ResizeObserver !== 'undefined') {
    kanbanResizeObserver = new ResizeObserver(updateKanbanScrollWidth)
    kanbanResizeObserver.observe(area)
    const board = area.querySelector('.kanban-board')
    if (board instanceof HTMLElement) kanbanResizeObserver.observe(board)
  }

  updateKanbanScrollWidth()
}

async function refreshKanbanScroller() {
  await nextTick()
  attachKanbanResizeObserver()
}

function syncKanbanScroll(source: 'top' | 'body') {
  if (syncingKanbanScroll) return
  const top = kanbanTopScroller.value
  const body = kanbanArea.value
  if (!top || !body) return

  const from = source === 'top' ? top : body
  const to = source === 'top' ? body : top
  syncingKanbanScroll = true
  to.scrollLeft = from.scrollLeft
  requestAnimationFrame(() => {
    syncingKanbanScroll = false
  })
}

onMounted(() => {
  refreshKanbanScroller()
  window.addEventListener('resize', updateKanbanScrollWidth)
})

onBeforeUnmount(() => {
  kanbanResizeObserver?.disconnect()
  window.removeEventListener('resize', updateKanbanScrollWidth)
})

async function reload(project: string) {
  loading.value = true
  error.value = ''
  payload.value = null
  summary.value = null
  lanes.value = []
  inboxNoteCount.value = 0
  try {
    const [data, summaryResult, lanesResult, inboxResult] = await Promise.all([
      loadBlackboardData(project),
      loadBoardSummary(project),
      loadLanes(project),
      loadInboxNotes(project).catch((err) => {
        console.warn('[bb] inbox notes unavailable', err)
        return { data: [] }
      }),
    ])
    payload.value = data
    summary.value = summaryResult.data
    lanes.value = lanesResult.data
    inboxNoteCount.value = inboxResult.data.length
    applyBoardViewSettings(data.meta.board_view?.hidden_statuses)
    try {
      projectAgents.value = (await loadProjectAgents(project)).agents
    } catch (err) {
      console.warn('[bb] project agents unavailable', err)
      projectAgents.value = []
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

watch(
  () => props.project,
  (project) => {
    if (project) reload(project)
  },
  { immediate: true },
)

watch(
  () => props.section,
  (section) => {
    activeWorkspace.value = section ?? 'tickets'
  },
  { immediate: true },
)

watch(
  () => props.ticketView,
  (view) => {
    activeTicketView.value = view ?? 'kanban'
  },
  { immediate: true },
)

watch(
  () => props.id,
  (id) => {
    selectedTicketId.value = id ?? null
    if (!id) {
      detailReturnWorkspace.value = null
    } else if (!detailReturnWorkspace.value) {
      detailReturnWorkspace.value = activeWorkspace.value
    }
  },
  { immediate: true },
)

const tickets = computed(() => payload.value?.tickets ?? [])
const activeLanes = computed(() => lanes.value.filter((lane) => lane.status === 'active'))
const laneDropdownOptions = computed<BbDropdownOption[]>(() => [
  { value: 'all', label: t('laneAll') },
  ...activeLanes.value.map((lane) => ({
    value: lane.id,
    label: `${lane.id} · ${lane.label}`,
  })),
])

const queryMatchedTickets = computed(() => {
  const q = query.value.trim().toLowerCase()
  return tickets.value.filter((ticket) => {
    const assignee = ticket.extra.assignee ?? ''
    return !q || [
      ticket.id,
      ticket.lane,
      ticket.title,
      assignee,
      ticketSpecSearchText(ticket),
      ticket.status,
    ].some((value) => value.toLowerCase().includes(q))
  })
})

function ticketSpecSearchText(ticket: BlackboardTicket): string {
  return [
    ticket.spec?.summary,
    ...(ticket.spec?.stories ?? []).flatMap((story) => [
      story.given,
      story.when,
      story.then,
      story.sample ?? '',
    ]),
    ...(ticket.spec?.risks ?? []).flatMap((risk) => [
      risk.description,
      risk.mitigation ?? '',
      risk.status ?? '',
    ]),
    ...(ticket.spec?.progress_record ?? []).flatMap((record) => [
      record.at ?? '',
      record.summary,
      ...(record.evidence ?? []),
    ]),
  ].join(' ')
}

function ticketPreviewText(ticket: BlackboardTicket): string {
  const records = ticket.spec?.progress_record ?? []
  const latestRecord = records.length > 0 ? records[records.length - 1]?.summary : ''
  return latestRecord || ticket.spec?.summary || ''
}

const laneFilteredTickets = computed(() =>
  queryMatchedTickets.value.filter((ticket) =>
    selectedLane.value === 'all' || ticket.lane === selectedLane.value,
  ),
)

const visibleStatusColumnSet = computed(() => new Set(visibleStatusColumns.value))

const ticketList = computed(() =>
  laneFilteredTickets.value.filter((ticket) =>
    visibleStatusColumnSet.value.has(ticket.status as TicketStatus),
  ),
)

const ticketListIds = computed(() => ticketList.value.map((ticket) => ticket.id))

const selectedTicket = computed<BlackboardTicket | null>(() =>
  selectedTicketId.value
    ? tickets.value.find((ticket) => ticket.id === selectedTicketId.value) ?? null
    : null,
)

const stats = computed(() => {
  const byStatus = {} as Record<TicketStatus, number>
  for (const status of statusOrder) {
    byStatus[status] = summary.value
      ? summary.value.by_status[status] ?? 0
      : tickets.value.filter((ticket) => ticket.status === status).length
  }
  if (summary.value) {
    return {
      total: summary.value.total,
      byStatus,
      open: statusOrder
        .filter(isOpenTicketStatus)
        .reduce((sum, status) => sum + byStatus[status], 0),
    }
  }
  return {
    total: tickets.value.length,
    byStatus,
    open: tickets.value.filter((ticket) => isOpenTicketStatus(ticket.status)).length,
  }
})

const groupedByStatus = computed(() => {
  const result = {} as Record<TicketStatus, BlackboardTicket[]>
  for (const status of statusOrder) result[status] = []
  for (const ticket of laneFilteredTickets.value) {
    const key = statusOrder.includes(ticket.status as TicketStatus) ? ticket.status as TicketStatus : 'todo'
    result[key].push(ticket)
  }
  return result
})

const boardStatusOrder = computed(() =>
  statusOrder.filter((status) => visibleStatusColumnSet.value.has(status)),
)

const groupedByLane = computed(() => {
  const result: Record<string, BlackboardTicket[]> = {}
  for (const lane of activeLanes.value) result[lane.id] = []
  for (const ticket of queryMatchedTickets.value) {
    if (!result[ticket.lane]) result[ticket.lane] = []
    result[ticket.lane].push(ticket)
  }
  return result
})

const workspaceTabGroups = computed(() => [
  {
    label: t('workspace'),
    tabs: [
      { key: 'inbox' as const, label: t('inbox'), icon: 'inbox' as const, count: inboxNoteCount.value },
      { key: 'ideaCanvas' as const, label: t('ideaCanvas'), icon: 'ideaCanvas' as const, count: null },
      { key: 'tickets' as const, label: t('tickets'), icon: 'tickets' as const, count: null },
      { key: 'taskGraphs' as const, label: t('taskGraphs'), icon: 'taskGraphs' as const, count: null },
      { key: 'wiki' as const, label: t('wiki'), icon: 'wiki' as const, count: null },
      { key: 'agents' as const, label: t('agents'), icon: 'agents' as const, count: null },
    ],
  },
  {
    label: t('system'),
    tabs: [
      { key: 'settings' as const, label: t('settings'), icon: 'settings' as const, count: null },
    ],
  },
])

const ticketViewTabs = computed<Array<{ key: TicketViewMode; label: string; icon: Component }>>(() => [
  { key: 'kanban', label: t('kanban'), icon: Columns3 },
  { key: 'graph', label: t('graph'), icon: GitFork },
  { key: 'list', label: t('listView'), icon: ListTodo },
])

watch(
  [
    activeWorkspace,
    activeTicketView,
    loading,
    selectedLane,
    query,
    () => tickets.value.length,
    () => visibleStatusColumns.value.join('|'),
  ],
  () => refreshKanbanScroller(),
  { flush: 'post' },
)

function workspaceRoute(workspace: WorkspaceKey) {
  if (workspace === 'tickets') return `/projects/${props.project}/tickets`
  if (workspace === 'taskGraphs') return `/projects/${props.project}/task-graphs`
  if (workspace === 'inbox') return `/projects/${props.project}/inbox`
  if (workspace === 'ideaCanvas') return `/projects/${props.project}/idea-canvas`
  if (workspace === 'agents') return `/projects/${props.project}/agents`
  if (workspace === 'settings') return `/projects/${props.project}/settings`
  if (workspace === 'wiki') return `/projects/${props.project}/wiki`
  return `/projects/${props.project}/tickets`
}

function setWorkspace(workspace: WorkspaceKey) {
  router.push(workspaceRoute(workspace))
}

function ticketViewRoute(view: TicketViewMode) {
  if (view === 'graph') return `/projects/${props.project}/tickets/graph`
  if (view === 'list') return `/projects/${props.project}/tickets/list`
  return `/projects/${props.project}/tickets`
}

function setTicketView(view: TicketViewMode) {
  router.push(ticketViewRoute(view))
}

function toggleLane(laneId: string) {
  selectedLane.value = selectedLane.value === laneId ? 'all' : laneId
}

function isStatusColumnVisible(status: TicketStatus) {
  return visibleStatusColumnSet.value.has(status)
}

async function toggleStatusColumn(status: TicketStatus) {
  const next = new Set(visibleStatusColumns.value)
  if (next.has(status)) {
    if (next.size === 1) return
    next.delete(status)
  } else {
    next.add(status)
  }
  const previous = visibleStatusColumns.value
  visibleStatusColumns.value = statusOrder.filter((item) => next.has(item))
  mutationError.value = ''

  const seq = ++boardViewSaveSeq
  boardViewSaving.value = true
  try {
    const settings = await patchProjectBoardView(props.project, {
      hidden_statuses: hiddenStatusesFromVisible(visibleStatusColumns.value),
    })
    if (seq === boardViewSaveSeq) {
      applyBoardViewSettings(settings.hidden_statuses)
      if (payload.value) {
        payload.value = {
          ...payload.value,
          meta: {
            ...payload.value.meta,
            board_view: settings,
          },
        }
      }
    }
  } catch (err) {
    if (seq === boardViewSaveSeq) {
      visibleStatusColumns.value = previous
      mutationError.value =
        err instanceof Error
          ? `${err.message}。${t('bbServerRunning')}`
          : t('boardViewSaveFailed')
    }
  } finally {
    if (seq === boardViewSaveSeq) {
      boardViewSaving.value = false
    }
  }
}

function openTicket(id: string) {
  detailReturnWorkspace.value = activeWorkspace.value
  if (activeWorkspace.value === 'tickets') {
    router.push({
      path: ticketViewRoute(activeTicketView.value),
      query: { preview: id },
    })
    return
  }
  selectedTicketId.value = id
}

function closeTicket() {
  const returnWorkspace = detailReturnWorkspace.value ?? activeWorkspace.value
  selectedTicketId.value = null
  detailReturnWorkspace.value = null
  router.push(returnWorkspace === 'tickets' ? ticketViewRoute(activeTicketView.value) : workspaceRoute(returnWorkspace))
}

function laneMetaFor(laneId: string) {
  return resolveLaneMeta(laneId, lanes.value)
}

async function onLanesChanged() {
  const result = await loadLanes(props.project)
  lanes.value = result.data
}

function onTicketDragStart(ticket: BlackboardTicket, event: DragEvent) {
  draggingTicketId.value = ticket.id
  mutationError.value = ''
  event.dataTransfer?.setData('text/plain', ticket.id)
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move'
  }
}

function onTicketDragEnd() {
  draggingTicketId.value = null
  dragOverStatus.value = null
}

function onColumnDragOver(status: TicketStatus, event: DragEvent) {
  if (!draggingTicketId.value) return
  event.preventDefault()
  dragOverStatus.value = status
  if (event.dataTransfer) {
    event.dataTransfer.dropEffect = 'move'
  }
}

function onColumnDragLeave(status: TicketStatus, event: DragEvent) {
  const currentTarget = event.currentTarget as HTMLElement | null
  const relatedTarget = event.relatedTarget as Node | null
  if (currentTarget && relatedTarget && currentTarget.contains(relatedTarget)) return
  if (dragOverStatus.value === status) {
    dragOverStatus.value = null
  }
}

async function onColumnDrop(status: TicketStatus, event: DragEvent) {
  event.preventDefault()
  const id = event.dataTransfer?.getData('text/plain') || draggingTicketId.value
  dragOverStatus.value = null
  draggingTicketId.value = null
  if (!id) return
  const ticket = tickets.value.find((item) => item.id === id)
  if (!ticket || ticket.status === status) return
  await moveTicketToStatus(ticket, status)
}

async function moveTicketToStatus(ticket: BlackboardTicket, status: TicketStatus) {
  if (!payload.value || movingTicketId.value) return
  const previousPayload = payload.value
  movingTicketId.value = ticket.id
  mutationError.value = ''
  payload.value = {
    ...payload.value,
    tickets: payload.value.tickets.map((item) =>
      item.id === ticket.id
        ? { ...item, status, updated_at: new Date().toISOString().slice(0, 10) }
        : item,
    ),
  }

  try {
    const result = await patchTicket(props.project, ticket.id, { status })
    mergeTicketWriteResult(result)

    const [liveData, summaryResult] = await Promise.all([
      loadBlackboardData(props.project),
      loadBoardSummary(props.project),
    ])
    payload.value = liveData
    summary.value = summaryResult.data
  } catch (err) {
    payload.value = previousPayload
    mutationError.value =
      err instanceof Error
        ? `${err.message}。${t('bbServerRunning')}`
        : t('boardViewTicketMoveFailed')
  } finally {
    movingTicketId.value = null
  }
}

function mergeTicketWriteResult(result: TicketWriteResult) {
  if (!payload.value) return
  payload.value = {
    ...payload.value,
    tickets: payload.value.tickets.map((item) =>
      item.id === result.ticket.id
        ? {
            ...item,
            lane: result.ticket.lane || item.lane,
            title: result.ticket.title || item.title,
            status: result.ticket.status || item.status,
            created_at: result.ticket.created_at || item.created_at,
            updated_at: result.ticket.updated_at || item.updated_at,
            file_name: result.ticket.file_name || item.file_name,
            file_path: result.ticket.path || item.file_path,
            extra: result.ticket.extra ?? item.extra,
            dependencies: dependenciesFromExtra(result.ticket.extra ?? item.extra, item.id),
            attachments: result.ticket.attachments,
          }
        : item,
    ),
  }
}

async function updateTicketDependencies(ticketId: string, dependencies: string[]) {
  if (!payload.value || dependencySavingId.value) return
  const previousPayload = payload.value
  const unique = dependencies.filter((id, index, all) => id !== ticketId && all.indexOf(id) === index)
  dependencySavingId.value = ticketId
  mutationError.value = ''
  payload.value = {
    ...payload.value,
    tickets: payload.value.tickets.map((item) =>
      item.id === ticketId
        ? {
            ...item,
            dependencies: unique,
            updated_at: new Date().toISOString().slice(0, 10),
            extra: unique.length
              ? { ...item.extra, depends_on: unique.join(' ') }
              : Object.fromEntries(
                  Object.entries(item.extra).filter(([key]) => key !== 'depends_on' && key !== 'dependencies'),
                ),
          }
        : item,
    ),
  }

  try {
    const result = await patchTicket(props.project, ticketId, { depends_on: unique })
    mergeTicketWriteResult(result)
  } catch (err) {
    payload.value = previousPayload
    mutationError.value =
      err instanceof Error
        ? `${err.message}。${t('boardViewDependencyModifyFailed')}`
        : t('boardViewDependencyModifyFailed')
  } finally {
    dependencySavingId.value = null
  }
}

async function updateTicketAssignee(ticket: BlackboardTicket, assignee: string) {
  if (!payload.value || assigneeSavingId.value) return
  const previousPayload = payload.value
  const nextAssignee = assignee.trim()
  assigneeSavingId.value = ticket.id
  mutationError.value = ''
  payload.value = {
    ...payload.value,
    tickets: payload.value.tickets.map((item) =>
      item.id === ticket.id
        ? {
            ...item,
            updated_at: new Date().toISOString().slice(0, 10),
            extra: nextAssignee
              ? { ...item.extra, assignee: nextAssignee }
              : Object.fromEntries(
                  Object.entries(item.extra).filter(([key]) => key !== 'assignee'),
                ),
          }
        : item,
    ),
  }

  try {
    const result = await patchTicket(props.project, ticket.id, { assignee: nextAssignee })
    mergeTicketWriteResult(result)
  } catch (err) {
    payload.value = previousPayload
    mutationError.value =
      err instanceof Error
        ? `${err.message}。${t('bbServerRunning')}`
        : t('boardViewAssigneeModifyFailed')
  } finally {
    assigneeSavingId.value = null
  }
}

function normalizeTicketAttachments(attachments: TicketAttachment[]): TicketAttachment[] {
  return attachments
    .map((attachment) => {
      const kind = attachment.kind.trim().toLowerCase()
      const target = attachment.target.trim()
      const label = attachment.label?.trim()
      const description = attachment.description?.trim()
      if (!kind || !target) return null
      return {
        kind,
        target,
        ...(label ? { label } : {}),
        ...(description ? { description } : {}),
      }
    })
    .filter((attachment): attachment is TicketAttachment => Boolean(attachment))
}

async function updateTicketAttachments(ticket: BlackboardTicket, attachments: TicketAttachment[]) {
  if (!payload.value || attachmentsSavingId.value) return
  const previousPayload = payload.value
  const normalized = normalizeTicketAttachments(attachments)
  attachmentsSavingId.value = ticket.id
  mutationError.value = ''
  payload.value = {
    ...payload.value,
    tickets: payload.value.tickets.map((item) =>
      item.id === ticket.id
        ? {
            ...item,
            attachments: normalized,
            updated_at: new Date().toISOString().slice(0, 10),
            extra: item.extra,
          }
        : item,
    ),
  }

  try {
    const result = await patchTicket(props.project, ticket.id, { attachments: normalized })
    mergeTicketWriteResult(result)
  } catch (err) {
    payload.value = previousPayload
    mutationError.value =
      err instanceof Error
        ? `${err.message}。${t('ticketAttachmentsSaveFailed')}`
        : t('ticketAttachmentsSaveFailed')
  } finally {
    attachmentsSavingId.value = null
  }
}

function agentLabel(id?: string) {
  if (!id) return t('unassigned')
  return id
}

/// Persist a workflow status change initiated from TicketDetailPanel. Mirrors
/// updateTicketAssignee: optimistic in-memory patch, then PATCH /api, then
/// merge the authoritative result. Rolls back and surfaces the backend error
/// string on failure so the user can see why bb-server rejected it.
async function updateTicketStatus(ticket: BlackboardTicket, status: string) {
  if (!payload.value || statusSavingId.value) return
  const nextStatus = status.trim()
  if (!nextStatus || nextStatus === ticket.status) return
  const previousPayload = payload.value
  statusSavingId.value = ticket.id
  mutationError.value = ''
  payload.value = {
    ...payload.value,
    tickets: payload.value.tickets.map((item) =>
      item.id === ticket.id
        ? {
            ...item,
            status: nextStatus,
            updated_at: new Date().toISOString().slice(0, 10),
          }
        : item,
    ),
  }

  try {
    const result = await patchTicket(props.project, ticket.id, { status: nextStatus })
    mergeTicketWriteResult(result)
  } catch (err) {
    payload.value = previousPayload
    mutationError.value =
      err instanceof Error
        ? `${t('workflowStatusUpdateFailed')}: ${err.message}`
        : t('workflowStatusUpdateFailed')
  } finally {
    statusSavingId.value = null
  }
}

async function deprecateSelectedTicket(ticket: BlackboardTicket) {
  if (!payload.value || deprecatingTicketId.value) return
  deprecatingTicketId.value = ticket.id
  mutationError.value = ''
  try {
    await deprecateTicket(props.project, ticket.id)
    selectedTicketId.value = null
    detailReturnWorkspace.value = null
    const [liveData, summaryResult] = await Promise.all([
      loadBlackboardData(props.project),
      loadBoardSummary(props.project),
    ])
    payload.value = liveData
    summary.value = summaryResult.data
    router.push(ticketViewRoute(activeTicketView.value))
  } catch (err) {
    mutationError.value =
      err instanceof Error
        ? `${err.message}。${t('ticketDeprecateFailed')}`
        : t('ticketDeprecateFailed')
  } finally {
    deprecatingTicketId.value = null
  }
}
</script>

<template>
  <div class="bb-dashboard-shell">
    <div class="bb-dashboard-body">
      <aside class="bb-dashboard-sidebar">
        <div class="bb-brand-block">
          <img class="bb-brand-mark" :src="blackboardIconUrl" alt="" aria-hidden="true" />
          <div>
            <strong>{{ t('brandName') }}</strong>
            <span>{{ t('brandSubtitle') }}</span>
          </div>
        </div>

        <ProjectSwitcher :project="project" />

        <nav class="bb-side-nav" :aria-label="t('boardNavAriaLabel')">
          <section
            v-for="group in workspaceTabGroups"
            :key="group.label"
            class="bb-side-nav-group"
          >
            <div class="bb-side-nav-section">{{ group.label }}</div>
            <div class="bb-side-nav-items">
              <button
                v-for="tab in group.tabs"
                :key="tab.key"
                type="button"
                :class="['bb-side-nav-item', { active: activeWorkspace === tab.key }]"
                @click="setWorkspace(tab.key)"
              >
                <span class="bb-side-nav-main">
                  <Badge
                    v-if="tab.count !== null"
                    class="bb-side-nav-badge"
                    :count="tab.count"
                    :max-count="99"
                    :offset="[2, 2]"
                    color="var(--bb-theme-primary)"
                    size="small"
                  >
                    <span class="bb-side-nav-icon" aria-hidden="true">
                      <component :is="workspaceIconComponents[tab.icon]" class="bb-side-nav-glyph" />
                    </span>
                  </Badge>
                  <span v-else class="bb-side-nav-icon" aria-hidden="true">
                    <component :is="workspaceIconComponents[tab.icon]" class="bb-side-nav-glyph" />
                  </span>
                  <span class="bb-side-nav-label">{{ tab.label }}</span>
                </span>
              </button>
            </div>
          </section>
        </nav>
      </aside>

      <section class="bb-dashboard-main">
        <header class="bb-workspace-topbar">
          <label class="bb-top-search">
            <span class="sr-only">{{ t('searchLabel') }}</span>
            <Search class="bb-top-search-icon" aria-hidden="true" />
            <input v-model="query" :placeholder="t('searchPlaceholder')" />
          </label>
          <div class="bb-top-actions">
            <button
              class="bb-top-action-button bb-top-action-button--compact bb-top-action-button--locale"
              type="button"
              @click="toggleLocale"
            >
              <Languages class="bb-top-action-svg" aria-hidden="true" />
              <span>{{ localeLabel }}</span>
            </button>
            <button
              class="bb-top-action-button bb-top-action-button--compact bb-top-action-button--theme"
              type="button"
              @click="toggleTheme"
            >
              <component :is="themeMode === 'dark' ? Moon : Sun" class="bb-top-action-svg" aria-hidden="true" />
              <span>{{ themeLabel }}</span>
            </button>
            <button class="bb-top-action-button bb-top-action-button--lanes" type="button" @click="showLaneManager = true">
              <SlidersHorizontal class="bb-top-action-svg" aria-hidden="true" />
              <span>{{ t('manageLanes') }}</span>
            </button>
            <button class="bb-top-action-button bb-top-action-button--refresh" type="button" @click="reload(project)">
              <RefreshCw class="bb-top-action-svg" aria-hidden="true" />
              <span>{{ t('refresh') }}</span>
            </button>
          </div>
        </header>

        <main class="bb-dashboard-content">
        <div v-if="loading" class="bb-state-panel">{{ t('loadingDashboard', { project }) }}</div>
        <div v-else-if="error" class="bb-state-panel bb-error">{{ error }}</div>
        <template v-else>
          <section v-if="activeWorkspace === 'tickets'" class="bb-tickets-content-view">
            <header class="bb-workspace-head bb-ticket-viewbar">
              <div class="bb-ticket-view-tabs" :aria-label="t('ticketViews')">
                <button
                  v-for="view in ticketViewTabs"
                  :key="view.key"
                  type="button"
                  :class="['bb-ticket-view-tab', { active: activeTicketView === view.key }]"
                  @click="setTicketView(view.key)"
                >
                  <component :is="view.icon" class="bb-ticket-view-icon" aria-hidden="true" />
                  <span>{{ view.label }}</span>
                </button>
              </div>
              <div class="bb-ticket-view-controls">
                <BbDropdown
                  v-model="selectedLane"
                  class="bb-lane-dropdown"
                  :label="t('lanes')"
                  :options="laneDropdownOptions"
                  :min-width="184"
                />
                <span class="bb-toolbar-count">{{ t('visible', { count: ticketList.length }) }}</span>
              </div>
            </header>
            <div class="bb-tickets-content-body">
              <section class="bb-status-visibility" :aria-label="t('statusColumns')">
                <span>{{ t('statusColumns') }}</span>
                <label
                  v-for="status in statusOrder"
                  :key="status"
                  class="bb-status-toggle"
                  :class="{ active: isStatusColumnVisible(status) }"
                  :style="{ '--status-color': `var(--bb-status-${status})` }"
                >
                  <input
                    type="checkbox"
                    :checked="isStatusColumnVisible(status)"
                    :disabled="boardViewSaving || (isStatusColumnVisible(status) && boardStatusOrder.length === 1)"
                    @change="toggleStatusColumn(status)"
                  />
                  <span>{{ ticketStatusLabel(status) }}</span>
                </label>
              </section>
              <div v-if="mutationError" class="bb-mutation-error">{{ mutationError }}</div>

              <section v-if="activeLanes.length > 0" class="bb-lane-strip">
                <button
                  type="button"
                  :class="['bb-lane-filter', { active: selectedLane === 'all' }]"
                  @click="selectedLane = 'all'"
                >
                  <span class="lane-dot all" />
                  <span>{{ t('laneAll') }}</span>
                  <strong>{{ queryMatchedTickets.length }}</strong>
                </button>
                <button
                  v-for="lane in activeLanes"
                  :key="lane.id"
                  type="button"
                  :class="['bb-lane-filter', { active: selectedLane === lane.id }]"
                  :style="{ '--family-color': lane.color || '#64748b' }"
                  @click="toggleLane(lane.id)"
                >
                  <span class="lane-dot" />
                  <span>{{ lane.id }} · {{ lane.label }}</span>
                  <strong>{{ groupedByLane[lane.id]?.length || 0 }}</strong>
                </button>
              </section>
              <section v-else class="bb-state-panel">
                {{ t('laneEmpty') }}
              </section>

            <section v-if="activeTicketView === 'kanban'" class="bb-board-workspace">
            <div
              ref="kanbanTopScroller"
              class="bb-kanban-top-scroll"
              aria-hidden="true"
              @scroll="syncKanbanScroll('top')"
            >
              <div class="bb-kanban-scroll-spacer" :style="{ width: `${kanbanScrollWidth}px` }" />
            </div>
            <div ref="kanbanArea" class="bb-kanban-area" @scroll="syncKanbanScroll('body')">
              <section class="kanban-board">
                <div
                  v-for="status in boardStatusOrder"
                  :key="status"
                  :data-kind="status"
                  :class="[
                    'kanban-column',
                    {
                      'drag-over': dragOverStatus === status,
                      'drop-disabled': movingTicketId !== null,
                    },
                  ]"
                  @dragover="onColumnDragOver(status, $event)"
                  @dragenter="onColumnDragOver(status, $event)"
                  @dragleave="onColumnDragLeave(status, $event)"
                  @drop="onColumnDrop(status, $event)"
                >
                  <div class="kanban-header">
                    <strong>{{ ticketStatusLabel(status) }}</strong>
                    <span>{{ groupedByStatus[status].length }}</span>
                  </div>
                  <div class="kanban-cards">
                    <TicketCard
                      v-for="ticket in groupedByStatus[status]"
                      :key="ticket.id"
                      :project="project"
                      :ticket="ticket"
                      :lanes="lanes"
                      :draggable="movingTicketId === null"
                      :dragging="draggingTicketId === ticket.id"
                      :moving="movingTicketId === ticket.id"
                      @open="openTicket(ticket.id)"
                      @drag-start="onTicketDragStart(ticket, $event)"
                      @drag-end="onTicketDragEnd"
                    />
                    <div v-if="groupedByStatus[status].length === 0" class="empty-column">{{ t('noTicket') }}</div>
                  </div>
                </div>
              </section>
            </div>
          </section>

            <section v-else-if="activeTicketView === 'list'" class="bb-tickets-workspace">
            <div class="bb-ticket-table" role="table" :aria-label="t('ticketListTableAriaLabel')">
              <div class="bb-ticket-table-head" role="row">
                <span>{{ t('ticketTableId') }}</span>
                <span>{{ t('ticketTableTicket') }}</span>
                <span>{{ t('lanes') }}</span>
                <span>{{ t('status') }}</span>
                <span>{{ t('assignee') }}</span>
                <span>{{ t('dependencies') }}</span>
              </div>
              <button
                v-for="ticket in ticketList"
                :key="ticket.id"
                type="button"
                class="bb-ticket-row"
                role="row"
                @click="openTicket(ticket.id)"
              >
                <span class="ticket-id">{{ ticket.id }}</span>
                <span class="bb-ticket-title-cell">
                  <strong>{{ ticket.title }}</strong>
                  <small>{{ ticketPreviewText(ticket) || t('noProgress') }}</small>
                </span>
                <span class="family-pill" :style="{ '--family-color': laneMetaFor(ticket.lane).color }">
                  {{ ticket.lane }} · {{ laneMetaFor(ticket.lane).label }}
                </span>
                <span>{{ ticketStatusLabel(ticket.status) }}</span>
                <span class="ticket-assignee-pill">{{ agentLabel(ticket.extra.assignee) }}</span>
                <span>{{ ticket.dependencies.length ? ticket.dependencies.join(' / ') : '-' }}</span>
              </button>
              <div v-if="ticketList.length === 0" class="bb-empty">{{ t('noTicket') }}</div>
            </div>
          </section>

            <TicketDependencyGraph
              v-else-if="activeTicketView === 'graph'"
              :project="project"
              :tickets="tickets"
              :visible-ticket-ids="ticketListIds"
              :focus-id="focus"
              :saving-id="dependencySavingId"
              @open="openTicket"
              @dependencies-change="updateTicketDependencies"
              @dependenciesChange="updateTicketDependencies"
            />
            </div>
          </section>

          <section v-else-if="activeWorkspace === 'inbox'" class="bb-inbox-workspace">
            <InboxPanel :project="project" variant="full" />
          </section>

          <IdeaCanvasPanel
            v-else-if="activeWorkspace === 'ideaCanvas'"
            :project="project"
          />

          <AgentWorkbench
            v-else-if="activeWorkspace === 'agents'"
            :current-project="project"
          />

          <TaskGraphCatalogPanel
            v-else-if="activeWorkspace === 'taskGraphs'"
            :project="project"
            :scope="taskGraphScope"
            :graph-id="taskGraphId"
            :mode="taskGraphMode"
          />

          <WikiPanel
            v-else-if="activeWorkspace === 'wiki'"
            :project="project"
            :path="path"
          />

          <section v-else class="bb-settings-workspace">
            <header class="bb-workspace-head">
              <div class="bb-workspace-head-main">
                <h2>{{ t('settings') }}</h2>
                <p>{{ t('settingsSubtitle') }}</p>
              </div>
            </header>
            <AgentConnectorPanel :project="project" />
          </section>
        </template>
        </main>
      </section>
    </div>

    <TicketDetailPanel
      v-if="selectedTicket"
      :project="project"
      :ticket="selectedTicket"
      :tickets="tickets"
      :lanes="lanes"
      :agents="projectAgents"
      :assignee-saving="assigneeSavingId === selectedTicket.id"
      :status-saving="statusSavingId === selectedTicket.id"
      :attachments-saving="attachmentsSavingId === selectedTicket.id"
      :deprecating="deprecatingTicketId === selectedTicket.id"
      @close="closeTicket"
      @assignee-change="updateTicketAssignee(selectedTicket, $event)"
      @status-change="updateTicketStatus(selectedTicket, $event)"
      @attachments-change="updateTicketAttachments(selectedTicket, $event)"
      @deprecate="deprecateSelectedTicket(selectedTicket)"
    />

    <LaneManager
      v-if="showLaneManager"
      :project="project"
      :lanes="lanes"
      :resolve-meta="laneMetaFor"
      @close="showLaneManager = false"
      @changed="onLanesChanged"
    />
  </div>
</template>
