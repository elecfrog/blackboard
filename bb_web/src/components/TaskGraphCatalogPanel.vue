<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Plus } from 'lucide-vue-next'
import TaskGraphEditorPanel from '@/components/TaskGraphEditorPanel.vue'
import TaskGraphRunPanel from '@/components/TaskGraphRunPanel.vue'
import TaskGraphCatalogCreate from '@/components/task-graph/TaskGraphCatalogCreate.vue'
import TaskGraphCatalogAlert from '@/components/task-graph/TaskGraphCatalogAlert.vue'
import TaskGraphCatalogSidebar from '@/components/task-graph/TaskGraphCatalogSidebar.vue'
import TaskGraphPreviewPanel from '@/components/task-graph/TaskGraphPreviewPanel.vue'
import BbDropdown, { type BbDropdownOption } from '@/components/BbDropdown.vue'
import {
  createTaskGraphSchedule,
  createProjectTaskGraph,
  deleteTaskGraphSchedule,
  forkSystemTaskGraph,
  listTaskGraphSchedules,
  listTaskGraphRuns,
  loadTaskGraphCatalog,
  patchTaskGraphSchedule,
  readTaskGraph,
  runTaskGraphScheduleNow,
  startTaskGraphRun,
  taskGraphDefaultInput,
  type TaskGraphCatalogItem,
  type TaskGraphDefinition,
  type TaskGraphRef,
  type TaskGraphRunSummary,
  type TaskGraphSchedule,
  type TaskGraphScheduleCreateInput,
  type TaskGraphSchedulePatchInput,
  type TaskGraphScope,
} from '@/data/taskGraphs'
import { t } from '@/i18n'

const props = defineProps<{
  project: string
  scope?: string
  graphId?: string
  mode?: string
}>()

type TaskGraphFilter = 'all' | TaskGraphScope

const route = useRoute()
const router = useRouter()
const graphs = ref<TaskGraphCatalogItem[]>([])
const selectedGraph = ref<TaskGraphDefinition | null>(null)
const selectedRef = ref<TaskGraphRef | null>(null)
const loading = ref(true)
const graphLoading = ref(false)
const error = ref('')
const actionAlert = ref<{ tone: 'error' | 'warning'; message: string } | null>(null)
const actionBusy = ref('')
const catalogSource = ref<'rest' | 'mock'>('mock')
const filter = ref<TaskGraphFilter>('all')
const createTitle = ref('')
const createDialogOpen = ref(false)
const createError = ref('')
const createDialogRef = ref<InstanceType<typeof TaskGraphCatalogCreate> | null>(null)
const activeRunId = ref('')
const runInputValues = ref<Record<string, unknown>>({})
const runHistory = ref<TaskGraphRunSummary[]>([])
const runHistorySource = ref<'rest' | 'mock'>('mock')
const runHistoryLoading = ref(false)
const schedules = ref<TaskGraphSchedule[]>([])
const schedulesLoading = ref(false)

const selectedCatalogItem = computed(() =>
  selectedRef.value
    ? graphs.value.find((graph) => graph.scope === selectedRef.value?.scope && graph.id === selectedRef.value.id) ?? null
    : null,
)

const isRouteDetail = computed(() => validScope(props.scope) && Boolean(props.graphId))
const isEditorPanelActive = computed(() =>
  Boolean(selectedGraph.value && props.mode === 'edit' && (selectedGraph.value.scope === 'project' || props.project === 'blackboard')),
)
const isRunPanelActive = computed(() => Boolean(selectedGraph.value && activeRunId.value))
const hasEmbeddedDetailPanel = computed(() => isEditorPanelActive.value || isRunPanelActive.value)
const selectedGraphSchedules = computed(() =>
  selectedRef.value
    ? schedules.value.filter((schedule) =>
        schedule.graph_ref.scope === selectedRef.value?.scope && schedule.graph_ref.id === selectedRef.value.id,
      )
    : [],
)

const catalogStats = computed(() => ({
  system: graphs.value.filter((graph) => graph.scope === 'system').length,
  project: graphs.value.filter((graph) => graph.scope === 'project').length,
}))

const filterOptions = computed<BbDropdownOption[]>(() => [
  {
    value: 'all',
    label: t('taskGraphAll'),
    badge: String(graphs.value.length),
  },
  {
    value: 'system',
    label: t('taskGraphSystem'),
    badge: String(catalogStats.value.system),
  },
  {
    value: 'project',
    label: t('taskGraphProject'),
    badge: String(catalogStats.value.project),
  },
])

function showActionError(err: unknown) {
  actionAlert.value = { tone: 'error', message: err instanceof Error ? err.message : String(err) }
}

function clearActionAlert() {
  actionAlert.value = null
}

function setFilter(value: string) {
  if (value === 'all' || validScope(value)) {
    filter.value = value
  }
}

function validScope(value: string | undefined): value is TaskGraphScope {
  return value === 'system' || value === 'project'
}

function graphRoute(graph: TaskGraphRef, mode: 'view' | 'edit' = 'view') {
  const suffix = mode === 'edit' ? '/edit' : ''
  return `/projects/${props.project}/task-graphs/${graph.scope}/${graph.id}${suffix}`
}

async function reloadCatalog(options: { refreshSelectedGraph?: boolean } = {}) {
  const refreshSelectedGraph = options.refreshSelectedGraph ?? !isEditorPanelActive.value
  const showLoading = refreshSelectedGraph || !selectedGraph.value
  if (showLoading) loading.value = true
  error.value = ''
  try {
    const result = await loadTaskGraphCatalog(props.project)
    graphs.value = result.graphs
    catalogSource.value = result.source
    await reloadRunHistory()
    await reloadSchedules()
    if (refreshSelectedGraph || !selectedGraph.value) {
      await selectFromRouteOrDefault()
    }
  } catch (err) {
    if (showLoading) {
      error.value = err instanceof Error ? err.message : String(err)
    } else {
      showActionError(err)
    }
  } finally {
    if (showLoading) loading.value = false
  }
}

async function reloadRunHistory() {
  runHistoryLoading.value = true
  try {
    const result = await listTaskGraphRuns(props.project)
    runHistory.value = result.runs
    runHistorySource.value = result.source
  } catch (err) {
    showActionError(err)
  } finally {
    runHistoryLoading.value = false
  }
}

async function reloadSchedules() {
  schedulesLoading.value = true
  try {
    const result = await listTaskGraphSchedules(props.project)
    schedules.value = result.schedules
  } catch (err) {
    showActionError(err)
  } finally {
    schedulesLoading.value = false
  }
}

async function selectFromRouteOrDefault() {
  if (props.mode === 'edit') activeRunId.value = ''
  if (validScope(props.scope) && props.graphId) {
    await loadGraph({ scope: props.scope, id: props.graphId })
    applyRouteRunSelection()
    return
  }
  const first = graphs.value[0]
  if (first) await loadGraph(first)
}

async function loadGraph(ref: TaskGraphRef) {
  selectedRef.value = { scope: ref.scope, id: ref.id }
  selectedGraph.value = null
  graphLoading.value = true
  try {
    const result = await readTaskGraph(props.project, ref)
    selectedGraph.value = result.graph
    runInputValues.value = taskGraphDefaultInput(result.graph)
  } catch (err) {
    showActionError(err)
  } finally {
    graphLoading.value = false
  }
}

function openGraph(graph: TaskGraphCatalogItem) {
  activeRunId.value = ''
  router.push(graphRoute(graph))
}

function editProjectGraph(graph: TaskGraphCatalogItem) {
  if (graph.scope === 'system' && props.project !== 'blackboard') {
    void customizeGraph(graph)
    return
  }
  router.push(graphRoute(graph, 'edit'))
}

function openCreateDialog() {
  if (actionBusy.value) return
  createError.value = ''
  createDialogOpen.value = true
  void nextTick(() => createDialogRef.value?.focusInput())
}

function closeCreateDialog() {
  if (actionBusy.value === 'create') return
  createDialogOpen.value = false
  createError.value = ''
  createTitle.value = ''
}

async function createGraph() {
  if (actionBusy.value) return
  const title = createTitle.value.trim()
  if (!title) {
    createError.value = t('taskGraphCreateTitleRequired')
    return
  }
  actionBusy.value = 'create'
  clearActionAlert()
  createError.value = ''
  try {
    const result = await createProjectTaskGraph(props.project, title)
    createTitle.value = ''
    createDialogOpen.value = false
    await reloadCatalog()
    router.push(graphRoute(result.graph, 'edit'))
  } catch (err) {
    createError.value = err instanceof Error ? err.message : String(err)
  } finally {
    actionBusy.value = ''
  }
}

async function customizeGraph(graph: TaskGraphCatalogItem) {
  if (actionBusy.value) return
  actionBusy.value = `customize:${graph.id}`
  clearActionAlert()
  try {
    const result = await forkSystemTaskGraph(props.project, graph.id)
    await reloadCatalog()
    router.push(graphRoute(result.graph, 'edit'))
  } catch (err) {
    showActionError(err)
  } finally {
    actionBusy.value = ''
  }
}

async function runGraph(graph: TaskGraphCatalogItem) {
  if (actionBusy.value || graph.compile_error) return
  actionBusy.value = `run:${graph.scope}:${graph.id}`
  clearActionAlert()
  try {
    if (selectedRef.value?.scope !== graph.scope || selectedRef.value.id !== graph.id) {
      await loadGraph(graph)
    }
    const input = selectedGraph.value && selectedRef.value?.scope === graph.scope && selectedRef.value.id === graph.id
      ? { ...runInputValues.value }
      : {}
    const result = await startTaskGraphRun(props.project, graph, input)
    activeRunId.value = result.run.id
    router.push(graphRoute(graph))
    await reloadRunHistory()
    graphs.value = graphs.value.map((item) =>
      item.scope === graph.scope && item.id === graph.id
        ? {
            ...item,
            last_run: {
              run_id: result.run.id,
              status: result.run.status,
              updated_at: result.run.updated_at,
            },
          }
        : item,
    )
  } catch (err) {
    showActionError(err)
  } finally {
    actionBusy.value = ''
  }
}

function openRun(run: TaskGraphRunSummary) {
  const ref = (run.graph_ref ?? run.graph) as TaskGraphRef & { version?: number }
  selectedRef.value = { scope: ref.scope, id: ref.id }
  activeRunId.value = run.id
  router.push(graphRoute(ref))
}

function updateRunInput(inputId: string, value: unknown) {
  runInputValues.value = { ...runInputValues.value, [inputId]: value }
}

async function runSelectedGraph() {
  const graph = selectedCatalogItem.value
  if (!graph) return
  await runGraph(graph)
}

async function createSchedule(input: TaskGraphScheduleCreateInput) {
  if (actionBusy.value) return
  actionBusy.value = 'schedule:create'
  clearActionAlert()
  try {
    const result = await createTaskGraphSchedule(props.project, input)
    schedules.value = [result.schedule, ...schedules.value.filter((item) => item.id !== result.schedule.id)]
  } catch (err) {
    showActionError(err)
  } finally {
    actionBusy.value = ''
  }
}

async function patchSchedule(id: string, patch: TaskGraphSchedulePatchInput) {
  if (actionBusy.value) return
  actionBusy.value = `schedule:patch:${id}`
  clearActionAlert()
  try {
    const result = await patchTaskGraphSchedule(props.project, id, patch)
    schedules.value = schedules.value.map((item) => item.id === id ? result.schedule : item)
  } catch (err) {
    showActionError(err)
  } finally {
    actionBusy.value = ''
  }
}

async function deleteSchedule(id: string) {
  if (actionBusy.value) return
  actionBusy.value = `schedule:delete:${id}`
  clearActionAlert()
  try {
    await deleteTaskGraphSchedule(props.project, id)
    schedules.value = schedules.value.filter((item) => item.id !== id)
  } catch (err) {
    showActionError(err)
  } finally {
    actionBusy.value = ''
  }
}

async function runScheduleNow(id: string) {
  if (actionBusy.value) return
  const schedule = schedules.value.find((item) => item.id === id)
  actionBusy.value = `schedule:run:${id}`
  clearActionAlert()
  try {
    const result = await runTaskGraphScheduleNow(props.project, id)
    activeRunId.value = result.run.id
    await reloadRunHistory()
    await reloadSchedules()
    if (schedule) {
      const ref = schedule.graph_ref
      selectedRef.value = { scope: ref.scope, id: ref.id }
      router.push(graphRoute(ref))
      graphs.value = graphs.value.map((item) =>
        item.scope === ref.scope && item.id === ref.id
          ? {
              ...item,
              last_run: {
                run_id: result.run.id,
                status: result.run.status,
                updated_at: result.run.updated_at,
              },
            }
          : item,
      )
    }
  } catch (err) {
    showActionError(err)
  } finally {
    actionBusy.value = ''
  }
}

async function handleEditorSaved(graph: TaskGraphDefinition) {
  selectedGraph.value = graph
  await reloadCatalog()
}

function closeEditor() {
  if (!selectedRef.value) return
  router.push(graphRoute(selectedRef.value))
}

function closeRunPanel() {
  activeRunId.value = ''
  void reloadRunHistory()
}

function navigateToRun(runId: string) {
  activeRunId.value = runId
}

function applyRouteRunSelection() {
  if (props.mode === 'edit') return
  const value = route.query.run
  const runId = Array.isArray(value) ? value[0] : value
  if (typeof runId === 'string' && runId.trim()) {
    activeRunId.value = runId.trim()
  }
}

watch(
  () => props.project,
  () => reloadCatalog({ refreshSelectedGraph: true }),
)

watch(
  () => [props.scope, props.graphId, props.mode],
  () => {
    if (!loading.value) void selectFromRouteOrDefault()
  },
)

watch(
  () => route.query.run,
  () => applyRouteRunSelection(),
)

watch(filter, () => {
  if (!selectedCatalogItem.value) void selectFromRouteOrDefault()
})

onMounted(reloadCatalog)
</script>

<template>
  <section :class="['task-graph-workspace', { 'route-detail': isRouteDetail }]">
    <header class="bb-workspace-head">
      <div class="bb-workspace-head-main">
        <h2>{{ t('taskGraphs') }}</h2>
        <p>{{ t('taskGraphSubtitle') }}</p>
      </div>
      <div class="bb-workspace-head-actions">
        <button type="button" class="bb-top-action-button" :disabled="!!actionBusy" @click="openCreateDialog">
          <Plus class="bb-top-action-svg" aria-hidden="true" />
          <span>{{ t('taskGraphCreate') }}</span>
        </button>
      </div>
    </header>

<TaskGraphCatalogCreate
      v-if="createDialogOpen"
      ref="createDialogRef"
      :busy="actionBusy === 'create'"
      :error="createError"
      @close="closeCreateDialog"
      @create="createGraph"
      @update:title="createTitle = $event"
    />

    <TaskGraphCatalogAlert
      v-if="actionAlert"
      :tone="actionAlert.tone"
      :message="actionAlert.message"
      @close="clearActionAlert"
    />
    <div v-if="loading" class="bb-state-panel">{{ t('loading') }}</div>
    <div v-else-if="error" class="bb-state-panel bb-error">{{ error }}</div>

    <div v-else :class="['task-graph-layout', { 'route-detail': isRouteDetail }]">
<TaskGraphCatalogSidebar
        :graphs="graphs"
        :filter="filter"
        :selected-ref="selectedRef"
        :filter-options="filterOptions"
        :action-busy="actionBusy"
        :project="project"
        @update:filter="setFilter"
        @open="openGraph"
        @run="runGraph"
        @customize="customizeGraph"
      />

      <TaskGraphEditorPanel
          v-if="selectedGraph && props.mode === 'edit' && (selectedGraph.scope === 'project' || props.project === 'blackboard')"
          :project="project"
          :graph="selectedGraph"
          @saved="handleEditorSaved"
          @run="runSelectedGraph"
          @close="closeEditor"
        />
        <TaskGraphRunPanel
          v-else-if="selectedGraph && activeRunId"
          :project="project"
          :run-id="activeRunId"
          @close="closeRunPanel"
          @navigate-run="navigateToRun"
        />
        <TaskGraphPreviewPanel
          v-else
          :selected-catalog-item="selectedCatalogItem"
          :selected-graph="selectedGraph"
          :selected-ref="selectedRef"
          :graph-loading="graphLoading"
          :active-run-id="activeRunId"
          :run-input-values="runInputValues"
          :run-history="runHistory"
          :run-history-source="runHistorySource"
          :run-history-loading="runHistoryLoading"
          :schedules="selectedGraphSchedules"
          :schedules-loading="schedulesLoading"
          :action-busy="actionBusy"
          :mode="props.mode ?? ''"
          :project="project"
          @update:run-input="updateRunInput"
          @run="runSelectedGraph"
          @edit="selectedCatalogItem && editProjectGraph(selectedCatalogItem)"
          @open="selectedCatalogItem && openGraph(selectedCatalogItem)"
          @customize="selectedCatalogItem && customizeGraph(selectedCatalogItem)"
          @open-run="openRun"
          @reload-history="reloadRunHistory"
          @navigate-run="navigateToRun"
          @create-schedule="createSchedule"
          @patch-schedule="patchSchedule"
          @delete-schedule="deleteSchedule"
          @run-schedule-now="runScheduleNow"
          @reload-schedules="reloadSchedules"
        />
    </div>
  </section>
</template>

<style scoped>
.task-graph-workspace {
  display: grid;
  gap: 14px;
}

.task-graph-workspace.route-detail {
  grid-template-rows: auto minmax(0, 1fr);
  height: calc(100dvh - 124px);
  min-height: 640px;
}

.task-graph-filters {
  display: block;
}

.task-graph-filter-dropdown {
  width: 100%;
}

.task-graph-filter-dropdown :deep(.bb-popup-select-trigger) {
  min-height: 34px;
  padding: 4px 8px;
  border-radius: 8px;
}

.task-graph-filter-dropdown :deep(.bb-popup-select-badge) {
  flex-basis: 22px;
  width: 22px;
  height: 22px;
  border-radius: 6px;
  font-size: 11px;
}

.task-graph-filter-dropdown :deep(.bb-popup-select-copy strong) {
  font-size: 12px;
}

.task-graph-source {
  color: var(--bb-text-muted);
  font-size: 12px;
  font-weight: 760;
}

.task-graph-layout {
  display: grid;
  grid-template-columns: minmax(220px, 272px) minmax(0, 1fr);
  gap: 12px;
  min-height: 0;
}

.task-graph-layout.route-detail {
  align-items: stretch;
  min-height: 0;
}

.task-graph-catalog,
.task-graph-preview {
  min-width: 0;
  min-height: 0;
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-catalog {
  display: grid;
  align-content: start;
  gap: 14px;
  padding: 12px;
}

.task-graph-layout.route-detail .task-graph-catalog {
  height: 100%;
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-gutter: stable;
}

.task-graph-section {
  display: grid;
  gap: 8px;
}

.task-graph-section > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--bb-text-muted);
}

.task-graph-section h3 {
  margin: 0;
  color: var(--bb-text-strong);
  font-size: 14px;
}

.task-graph-card {
  display: grid;
  gap: 8px;
  padding: 9px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-card.selected {
  border-color: var(--bb-theme-primary-border-strong);
  background: var(--bb-theme-primary-soft-strong);
  box-shadow: inset 0 0 0 1px var(--bb-theme-primary-border);
}

.task-graph-card-head {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 36px;
  align-items: center;
  gap: 8px;
}

.task-graph-card-main {
  display: grid;
  grid-template-columns: 36px minmax(0, 1fr);
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  cursor: pointer;
  text-align: left;
}

.task-graph-card-main > span:last-child {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.task-graph-card-main strong {
  display: block;
  overflow-wrap: anywhere;
  color: var(--bb-text-strong);
  font-size: 12px;
  font-weight: 780;
  line-height: 1.22;
}

.task-graph-preview h3 {
  display: block;
  overflow-wrap: anywhere;
  color: var(--bb-text-strong);
}

.task-graph-card-main small,
.task-graph-card p,
.task-graph-preview p,
.task-graph-preview span,
.task-graph-card dt {
  color: var(--bb-text-muted);
  font-size: 12px;
}

.task-graph-card p {
  display: none;
  margin: 0;
  line-height: 1.45;
}

.task-graph-card-icon {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  border-radius: 8px;
}

.task-graph-card-icon svg,
.task-graph-run-action svg,
.task-graph-card-state svg {
  width: 16px;
  height: 16px;
}

.task-graph-card-icon.system {
  background: color-mix(in srgb, var(--bb-focus) 12%, var(--bb-surface));
  color: var(--bb-focus);
}

.task-graph-card-icon.project {
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
}

.task-graph-card dl {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 6px;
  margin: 0;
}

.task-graph-card dl > div {
  min-width: 0;
  padding: 8px;
  border-radius: 8px;
  background: var(--bb-surface-soft);
}

.task-graph-card dd {
  margin: 2px 0 0;
  overflow-wrap: anywhere;
  color: var(--bb-text-strong);
  font-size: 12px;
  font-weight: 760;
}

.task-graph-run-action {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  padding: 0;
  border: 1px solid var(--task-graph-accent-border-light);
  border-radius: 8px;
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
  cursor: pointer;
}

.task-graph-run-action:hover {
  border-color: var(--task-graph-accent-border-medium);
  color: var(--bb-accent);
}

.task-graph-run-action:disabled {
  cursor: progress;
  opacity: 0.56;
}

.task-graph-run-action svg {
  color: currentColor;
}

.task-graph-run-action[data-status='queued'] svg,
.task-graph-run-action[data-status='running'] svg,
.task-graph-run-action[data-status='pending'] svg {
  animation: task-graph-spin 0.95s linear infinite;
}

.task-graph-run-action[data-status='queued'],
.task-graph-run-action[data-status='running'],
.task-graph-run-action[data-status='pending'] {
  border-color: var(--task-graph-focus-border);
  background: color-mix(in srgb, var(--bb-focus) 12%, var(--bb-surface));
  color: var(--bb-focus);
}

.task-graph-run-action[data-status='paused'] {
  border-color: color-mix(in srgb, var(--bb-warning) 34%, var(--bb-hairline));
  background: color-mix(in srgb, var(--bb-warning) 12%, var(--bb-surface));
  color: var(--bb-warning);
}

.task-graph-run-action[data-status='succeeded'] {
  border-color: var(--task-graph-accent-border-medium);
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
}

.task-graph-run-action[data-status='failed'],
.task-graph-run-action[data-status='cancelled'] {
  border-color: var(--task-graph-error-border);
  background: color-mix(in srgb, var(--bb-error) 12%, var(--bb-surface));
  color: var(--bb-error);
}

.task-graph-run-action[data-status='failed']:hover,
.task-graph-run-action[data-status='cancelled']:hover {
  border-color: color-mix(in srgb, var(--bb-error) 34%, var(--bb-hairline));
  color: var(--bb-error);
}

@keyframes task-graph-spin {
  to {
    transform: rotate(360deg);
  }
}

.task-graph-card-state {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  width: max-content;
  max-width: 100%;
  min-height: 20px;
  padding: 0 7px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 820;
}

.task-graph-badge,
.task-graph-edit-ready {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-height: 24px;
  padding: 0 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 820;
}

.task-graph-card-state.readonly,
.task-graph-badge.readonly {
  background: color-mix(in srgb, var(--bb-focus) 12%, var(--bb-surface));
  color: var(--bb-focus);
}

.task-graph-card-state.editable,
.task-graph-badge.editable,
.task-graph-edit-ready {
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
}

.task-graph-card-state.compile-error {
  background: color-mix(in srgb, var(--bb-error) 14%, var(--bb-surface));
  color: var(--bb-error);
}

.task-graph-run-action[data-status='compile_error'] {
  border-color: var(--task-graph-error-border);
  background: color-mix(in srgb, var(--bb-error) 10%, var(--bb-surface));
  color: var(--bb-error);
  cursor: not-allowed;
}

.task-graph-compile-error-panel {
  display: grid;
  place-items: center;
  gap: 8px;
  padding: 32px 24px;
  text-align: center;
}

.task-graph-compile-error-panel h4 {
  margin: 0;
  color: var(--bb-error);
  font-size: 16px;
  font-weight: 780;
}

.task-graph-compile-error-panel p {
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 13px;
}

.task-graph-compile-error-icon {
  width: 32px;
  height: 32px;
  color: var(--bb-error);
}

.task-graph-compile-error-detail {
  max-width: 100%;
  margin: 8px 0 0;
  padding: 12px 16px;
  overflow-x: auto;
  border: 1px solid var(--task-graph-error-border);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-error) 6%, var(--bb-surface-soft));
  color: var(--bb-text-strong);
  font-family: var(--bb-font-mono, monospace);
  font-size: 12px;
  line-height: 1.5;
  text-align: left;
  white-space: pre-wrap;
  word-break: break-word;
}

.task-graph-empty {
  padding: 14px;
  border: 1px dashed var(--bb-border-warm-dashed);
  border-radius: 8px;
  background: var(--bb-surface-soft);
  color: var(--bb-text-muted);
  font-size: 12px;
  text-align: center;
}

.task-graph-preview {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  overflow: hidden;
}

.task-graph-preview.embedded {
  grid-template-rows: minmax(0, 1fr);
}

.task-graph-preview > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px;
  border-bottom: 1px solid var(--bb-border-warm);
}

.task-graph-preview-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
}

.task-graph-preview h3 {
  margin: 3px 0;
  font-size: 18px;
}

.task-graph-preview p {
  margin: 0;
}

.task-graph-preview-body {
  display: grid;
  align-content: start;
  gap: 12px;
  min-height: 0;
  padding: 12px;
}

.task-graph-preview-canvas {
  min-height: 420px;
}

.task-graph-run-history {
  display: grid;
  align-content: start;
  gap: 8px;
  align-self: start;
  min-width: 0;
  padding: 10px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface);
}

.task-graph-run-history > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.task-graph-run-history h4 {
  margin: 0;
  color: var(--bb-text-strong);
  font-size: 13px;
}

.task-graph-run-history header button,
.task-graph-run-history-table button {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-height: 28px;
  padding: 0 8px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-muted);
  cursor: pointer;
  font-size: 12px;
  font-weight: 760;
}

.task-graph-run-history header button svg {
  width: 14px;
  height: 14px;
}

.task-graph-run-history-empty {
  padding: 12px;
  border-radius: 8px;
  background: var(--bb-surface-soft);
  color: var(--bb-text-muted);
  font-size: 12px;
  text-align: center;
}

.task-graph-run-history-table {
  overflow-x: auto;
}

.task-graph-run-history-table table {
  width: 100%;
  min-width: 780px;
  border-collapse: collapse;
  font-size: 12px;
}

.task-graph-run-history-table th,
.task-graph-run-history-table td {
  padding: 9px 8px;
  border-bottom: 1px solid var(--bb-border-warm);
  color: var(--bb-text-muted);
  text-align: left;
  white-space: nowrap;
}

.task-graph-run-history-table th {
  color: var(--bb-text-muted);
  font-weight: 760;
}

.task-graph-run-history-table tr.active {
  background: var(--bb-accent-soft);
}

.task-graph-run-status-pill {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 8px;
  border-radius: 999px;
  background: var(--bb-surface-soft);
  color: var(--bb-text-muted);
  font-weight: 820;
}

.task-graph-run-status-pill[data-status='queued'],
.task-graph-run-status-pill[data-status='running'],
.task-graph-run-status-pill[data-status='pending'] {
  background: color-mix(in srgb, var(--bb-focus) 12%, var(--bb-surface));
  color: var(--bb-focus);
}

.task-graph-run-status-pill[data-status='paused'] {
  background: color-mix(in srgb, var(--bb-warning) 12%, var(--bb-surface));
  color: var(--bb-warning);
}

.task-graph-run-status-pill[data-status='succeeded'] {
  background: var(--bb-accent-soft);
  color: var(--bb-accent);
}

.task-graph-run-status-pill[data-status='failed'],
.task-graph-run-status-pill[data-status='cancelled'] {
  background: color-mix(in srgb, var(--bb-error) 12%, var(--bb-surface));
  color: var(--bb-error);
}

.task-graph-preview-canvas :deep(svg) {
  width: 100%;
  min-width: 0;
  height: clamp(360px, 50vh, 620px);
}

.task-graph-preview-canvas :deep(.graph-node-accent) {
  fill: var(--graph-status-color);
}

.task-graph-edge-label {
  fill: var(--bb-text-muted);
  font-size: 12px;
  font-weight: 760;
  paint-order: stroke;
  stroke: var(--bb-surface);
  stroke-width: 4px;
  text-anchor: middle;
}

.task-graph-create-modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 90;
  display: grid;
  place-items: start center;
  padding: clamp(72px, 14vh, 132px) 18px 24px;
  background: var(--task-graph-backdrop-bg);
  backdrop-filter: blur(2px);
}

.task-graph-create-modal {
  box-sizing: border-box;
  display: grid;
  gap: 18px;
  width: min(560px, 100%);
  min-width: 0;
  padding: 20px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text);
  box-shadow: var(--bb-shadow-popover);
}

.task-graph-create-modal header,
.task-graph-create-modal footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
}

.task-graph-create-modal header > div {
  min-width: 0;
}

.task-graph-create-modal h3 {
  margin: 0;
  color: var(--bb-text-strong);
  font-size: 16px;
  line-height: 1.25;
}

.task-graph-create-modal header p {
  margin: 5px 0 0;
  color: var(--bb-text-muted);
  font-size: 12px;
  line-height: 1.45;
}

.task-graph-create-modal header button {
  display: grid;
  flex: 0 0 auto;
  place-items: center;
  width: 30px;
  height: 30px;
  border: 1px solid var(--bb-border-warm-medium);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-muted);
  cursor: pointer;
}

.task-graph-create-modal header button svg {
  width: 15px;
  height: 15px;
}

.task-graph-create-modal-fields {
  display: grid;
  gap: 12px;
  min-width: 0;
}

.task-graph-create-modal-fields label {
  display: grid;
  gap: 6px;
  min-width: 0;
}

.task-graph-create-modal-fields label span {
  color: var(--bb-text-muted);
  font-size: 12px;
  font-weight: 760;
}

.task-graph-create-modal-fields input {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  min-height: 38px;
  padding: 7px 10px;
  border: 1px solid var(--bb-border-warm-medium-strong);
  border-radius: 8px;
  background: var(--bb-surface-soft);
  color: var(--bb-text-strong);
  font: inherit;
  font-size: 13px;
}

.task-graph-create-modal-error {
  margin: 0;
  padding: 8px 10px;
  border: 1px solid var(--task-graph-error-border);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-error) 8%, var(--bb-surface));
  color: var(--bb-error);
  font-size: 12px;
  line-height: 1.45;
}

.task-graph-create-modal footer {
  justify-content: flex-end;
}

.task-graph-create-submit {
  border-color: var(--bb-text-strong);
  background: var(--bb-text-strong);
  color: var(--bb-surface);
}

.task-graph-alert-backdrop {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: grid;
  place-items: start center;
  padding: 18vh 18px 18px;
  background: var(--task-graph-strip-bg);
}

.task-graph-alert-dialog {
  width: min(520px, 100%);
  overflow: hidden;
  border: 1px solid color-mix(in srgb, var(--bb-error) 28%, transparent);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text);
  box-shadow: var(--bb-shadow-popover);
}

.task-graph-alert-dialog[data-tone='warning'] {
  border-color: color-mix(in srgb, var(--bb-warning) 34%, transparent);
}

.task-graph-alert-dialog > header {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  padding: 12px;
  border-bottom: 1px solid var(--bb-hairline);
  color: var(--bb-error);
}

.task-graph-alert-dialog[data-tone='warning'] > header {
  color: var(--bb-warning);
}

.task-graph-alert-dialog h3 {
  margin: 0;
  color: var(--bb-text-strong);
  font-size: 15px;
}

.task-graph-alert-dialog p {
  margin: 0;
  padding: 12px;
  color: var(--bb-text);
  font-size: 13px;
  line-height: 1.55;
  overflow-wrap: anywhere;
}

.task-graph-alert-dialog svg {
  width: 18px;
  height: 18px;
}

.task-graph-alert-dialog button {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  background: var(--bb-surface);
  color: var(--bb-text-muted);
  cursor: pointer;
}

@media (max-width: 980px) {
  .task-graph-workspace.route-detail {
    height: auto;
    min-height: 0;
  }

  .task-graph-layout.route-detail .task-graph-catalog {
    height: auto;
  }

  .task-graph-layout {
    grid-template-columns: 1fr;
  }

  .task-graph-preview > header {
    align-items: stretch;
    flex-direction: column;
  }

  .task-graph-preview-actions {
    justify-content: flex-start;
  }
}

/* 预览模式节点配置气泡 */
.task-graph-node-popover {
  padding: 12px 14px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 10px;
  background: var(--bb-surface);
  box-shadow: var(--bb-md-shadow-soft);
}

.task-graph-node-popover header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.task-graph-node-popover header h4 {
  margin: 0;
  font-size: 14px;
  font-weight: 700;
  color: var(--bb-text-strong);
}

.task-graph-node-popover header button {
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--bb-text-muted);
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
}

.task-graph-node-popover header button:hover {
  background: var(--bb-surface-muted);
  color: var(--bb-text);
}

.task-graph-node-popover-fields {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 4px 12px;
  margin: 0;
  font-size: 12px;
}

.task-graph-node-popover-fields dt {
  color: var(--bb-text-muted);
  font-weight: 600;
  white-space: nowrap;
}

.task-graph-node-popover-fields dd {
  margin: 0;
  color: var(--bb-text);
  word-break: break-all;
  white-space: pre-wrap;
  max-height: 120px;
  overflow-y: auto;
}

/* 气泡滑入动画 */
.node-popover-enter-active {
  transition: all 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

.node-popover-leave-active {
  transition: all 0.15s cubic-bezier(0.4, 0, 1, 1);
}

.node-popover-enter-from,
.node-popover-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
</style>
