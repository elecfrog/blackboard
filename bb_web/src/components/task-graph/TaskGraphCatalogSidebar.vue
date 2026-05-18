<script setup lang="ts">
import {
  AlertCircle,
  ChevronDown,
  ChevronRight,
  CircleCheck,
  CirclePause,
  CircleX,
  Folder,
  FolderOpen,
  GitFork,
  Grid2X2,
  LoaderCircle,
  MoreVertical,
  Play,
  Plus,
  Star,
  Workflow,
} from 'lucide-vue-next'
import { computed, ref } from 'vue'
import BbIconCommand from '@/components/common/BbIconCommand.vue'
import BbObjectItem from '@/components/common/BbObjectItem.vue'
import { t } from '@/i18n'
import {
  type TaskGraphCatalogGroup,
  type TaskGraphCatalogGroupKind,
  type TaskGraphCatalogItem,
} from '@/data/taskGraphs'

const props = defineProps<{
  graphs: TaskGraphCatalogItem[]
  groups: TaskGraphCatalogGroup[]
  selectedRef: { scope: string; id: string } | null
  actionBusy: string
  project: string
}>()

const emit = defineEmits<{
  open: [graph: TaskGraphCatalogItem]
  run: [graph: TaskGraphCatalogItem]
  customize: [graph: TaskGraphCatalogItem]
  'create-group': [kind: TaskGraphCatalogGroupKind]
  'rename-group': [groupId: string, title: string]
  'delete-group': [groupId: string]
  'move-graph': [graph: TaskGraphCatalogItem, groupId: string]
  'toggle-favorite': [graph: TaskGraphCatalogItem]
}>()

const expanded = ref<string[]>(['smart:all', 'system', 'project-ungrouped'])

const favoriteGraphs = computed(() => props.graphs.filter((graph) => graph.favorite))
const systemGroups = computed(() => groupsForKind('system'))
const projectGroups = computed(() => groupsForKind('project'))

function groupsForKind(kind: TaskGraphCatalogGroupKind) {
  return props.groups
    .filter((group) => group.kind === kind)
    .slice()
    .sort((a, b) => a.sort_order - b.sort_order || a.title.localeCompare(b.title))
}

function graphsForGroup(group: TaskGraphCatalogGroup) {
  return props.graphs
    .filter((graph) => graph.scope === group.kind && (graph.group_id ?? '') === group.id)
    .slice()
    .sort((a, b) => (a.sort_order ?? 0) - (b.sort_order ?? 0) || a.title.localeCompare(b.title))
}

function isExpanded(id: string) {
  return expanded.value.includes(id)
}

function toggleExpanded(id: string) {
  expanded.value = isExpanded(id)
    ? expanded.value.filter((item) => item !== id)
    : [...expanded.value, id]
}

function selected(graph: TaskGraphCatalogItem) {
  return props.selectedRef?.scope === graph.scope && props.selectedRef?.id === graph.id
}

function runActionStatus(graph: TaskGraphCatalogItem) {
  if (graph.compile_error) return 'compile_error'
  return graph.last_run?.status ?? 'idle'
}

function runActionIcon(graph: TaskGraphCatalogItem) {
  const status = runActionStatus(graph)
  if (status === 'compile_error') return AlertCircle
  if (status === 'queued' || status === 'pending' || status === 'running') return LoaderCircle
  if (status === 'paused') return CirclePause
  if (status === 'succeeded') return CircleCheck
  if (status === 'failed' || status === 'cancelled') return CircleX
  return Play
}

function graphIcon(graph: TaskGraphCatalogItem) {
  return graph.scope === 'system' ? Workflow : GitFork
}

function onMoveGraph(event: Event, graph: TaskGraphCatalogItem) {
  const groupId = (event.target as HTMLSelectElement).value
  if (groupId && groupId !== graph.group_id) emit('move-graph', graph, groupId)
}

function canDeleteGroup(group: TaskGraphCatalogGroup) {
  return group.id !== 'system' && group.id !== 'project-ungrouped'
}
</script>

<template>
  <aside class="task-graph-catalog">
    <section class="graph-catalog-smart">
      <button type="button" class="catalog-row primary" @click="toggleExpanded('smart:all')">
        <component :is="isExpanded('smart:all') ? ChevronDown : ChevronRight" aria-hidden="true" />
        <Grid2X2 aria-hidden="true" />
        <span>{{ t('taskGraphAll') }}</span>
        <strong>{{ graphs.length }}</strong>
      </button>
      <div v-if="isExpanded('smart:all')" class="catalog-children">
        <BbObjectItem
          v-for="graph in graphs"
          :key="`all:${graph.scope}:${graph.id}`"
          :title="graph.title"
          :active="selected(graph)"
          @select="emit('open', graph)"
        >
          <template #leading>
            <component :is="graphIcon(graph)" />
          </template>
        </BbObjectItem>
      </div>

      <button type="button" class="catalog-row" @click="toggleExpanded('smart:favorites')">
        <component :is="isExpanded('smart:favorites') ? ChevronDown : ChevronRight" aria-hidden="true" />
        <Star aria-hidden="true" />
        <span>{{ t('taskGraphFavoriteTitle') }}</span>
        <strong>{{ favoriteGraphs.length }}</strong>
      </button>
      <div v-if="isExpanded('smart:favorites')" class="catalog-children">
        <BbObjectItem
          v-for="graph in favoriteGraphs"
          :key="`favorite:${graph.scope}:${graph.id}`"
          :title="graph.title"
          :active="selected(graph)"
          @select="emit('open', graph)"
        >
          <template #leading>
            <component :is="graphIcon(graph)" />
          </template>
        </BbObjectItem>
        <div v-if="favoriteGraphs.length === 0" class="catalog-empty">{{ t('taskGraphFavoriteEmpty') }}</div>
      </div>
    </section>

    <section class="graph-group-section">
      <header>
        <span>{{ t('taskGraphSystemGroups') }}</span>
        <BbIconCommand size="mini" variant="ghost" :title="t('taskGraphGroupCreateSystem')" :disabled="!!actionBusy" @click="emit('create-group', 'system')">
          <Plus aria-hidden="true" />
        </BbIconCommand>
      </header>
      <div v-for="group in systemGroups" :key="group.id" class="graph-group">
        <div class="catalog-row group-row">
          <button type="button" class="catalog-row-main" @click="toggleExpanded(group.id)">
            <component :is="isExpanded(group.id) ? ChevronDown : ChevronRight" aria-hidden="true" />
            <component :is="isExpanded(group.id) ? FolderOpen : Folder" aria-hidden="true" />
            <span>{{ group.title }}</span>
            <strong>{{ graphsForGroup(group).length }}</strong>
          </button>
          <BbIconCommand size="mini" variant="ghost" :title="t('taskGraphGroupRename')" @click="emit('rename-group', group.id, group.title)">
            <MoreVertical aria-hidden="true" />
          </BbIconCommand>
        </div>
        <div v-if="isExpanded(group.id)" class="catalog-children">
          <article
            v-for="graph in graphsForGroup(group)"
            :key="`${graph.scope}:${graph.id}`"
            class="graph-item"
          >
            <BbObjectItem
              class="graph-item-main"
              :title="graph.title"
              :active="selected(graph)"
              @select="emit('open', graph)"
            >
              <template #leading>
                <Workflow />
              </template>
            </BbObjectItem>
            <BbIconCommand class="graph-favorite-command" size="mini" variant="ghost" :title="t('taskGraphFavoriteTitle')" @click="emit('toggle-favorite', graph)">
              <Star :class="{ filled: graph.favorite }" aria-hidden="true" />
            </BbIconCommand>
            <BbIconCommand
              class="graph-run-command"
              size="mini"
              variant="ghost"
              :title="t('taskGraphRun')"
              :data-status="runActionStatus(graph)"
              :disabled="!!actionBusy || !!graph.compile_error"
              @click="emit('run', graph)"
            >
              <component :is="runActionIcon(graph)" aria-hidden="true" />
            </BbIconCommand>
            <select class="graph-move-select" :value="graph.group_id ?? ''" @change="onMoveGraph($event, graph)">
              <option v-for="target in systemGroups" :key="target.id" :value="target.id">
                {{ target.title }}
              </option>
            </select>
          </article>
        </div>
      </div>
    </section>

    <section class="graph-group-section">
      <header>
        <span>{{ t('taskGraphProjectGroups') }}</span>
        <BbIconCommand size="mini" variant="ghost" :title="t('taskGraphGroupCreateProject')" :disabled="!!actionBusy" @click="emit('create-group', 'project')">
          <Plus aria-hidden="true" />
        </BbIconCommand>
      </header>
      <div v-for="group in projectGroups" :key="group.id" class="graph-group">
        <div class="catalog-row group-row">
          <button type="button" class="catalog-row-main" @click="toggleExpanded(group.id)">
            <component :is="isExpanded(group.id) ? ChevronDown : ChevronRight" aria-hidden="true" />
            <component :is="isExpanded(group.id) ? FolderOpen : Folder" aria-hidden="true" />
            <span>{{ group.title }}</span>
            <strong>{{ graphsForGroup(group).length }}</strong>
          </button>
          <BbIconCommand size="mini" variant="ghost" :title="t('taskGraphGroupRename')" @click="emit('rename-group', group.id, group.title)">
            <MoreVertical aria-hidden="true" />
          </BbIconCommand>
          <BbIconCommand
            v-if="canDeleteGroup(group)"
            size="mini"
            variant="danger"
            :title="t('taskGraphGroupDelete')"
            @click="emit('delete-group', group.id)"
          >
            ×
          </BbIconCommand>
        </div>
        <div v-if="isExpanded(group.id)" class="catalog-children">
          <article
            v-for="graph in graphsForGroup(group)"
            :key="`${graph.scope}:${graph.id}`"
            class="graph-item"
          >
            <BbObjectItem
              class="graph-item-main"
              :title="graph.title"
              :active="selected(graph)"
              @select="emit('open', graph)"
            >
              <template #leading>
                <GitFork />
              </template>
            </BbObjectItem>
            <BbIconCommand class="graph-favorite-command" size="mini" variant="ghost" :title="t('taskGraphFavoriteTitle')" @click="emit('toggle-favorite', graph)">
              <Star :class="{ filled: graph.favorite }" aria-hidden="true" />
            </BbIconCommand>
            <BbIconCommand
              class="graph-run-command"
              size="mini"
              variant="ghost"
              :title="t('taskGraphRun')"
              :data-status="runActionStatus(graph)"
              :disabled="!!actionBusy || !!graph.compile_error"
              @click="emit('run', graph)"
            >
              <component :is="runActionIcon(graph)" aria-hidden="true" />
            </BbIconCommand>
            <select class="graph-move-select" :value="graph.group_id ?? ''" @change="onMoveGraph($event, graph)">
              <option v-for="target in projectGroups" :key="target.id" :value="target.id">
                {{ target.title }}
              </option>
            </select>
          </article>
          <div v-if="graphsForGroup(group).length === 0" class="catalog-empty">{{ t('taskGraphGroupEmpty') }}</div>
        </div>
      </div>
    </section>
  </aside>
</template>

<style scoped>
.task-graph-catalog {
  display: grid;
  align-content: start;
  gap: 12px;
  min-width: 0;
  min-height: 0;
  padding: 10px;
  overflow: auto;
  border: 1px solid var(--bb-hairline);
  border-radius: 8px;
  background: var(--bb-surface);
}

.graph-catalog-smart,
.graph-group-section,
.graph-group {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.graph-group-section {
  padding-top: 8px;
  border-top: 1px solid var(--bb-hairline);
}

.graph-group-section > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 28px;
  color: var(--bb-text-muted);
  font-size: 12px;
  font-weight: 820;
}

.catalog-row {
  display: grid;
  grid-template-columns: 16px 18px minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-height: 32px;
  padding: 0 8px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--bb-text-muted);
  cursor: pointer;
  font: inherit;
  text-align: left;
}

.catalog-row.primary {
  color: var(--bb-text-strong);
}

.catalog-row:hover,
.catalog-row.group-row:hover {
  background: var(--bb-surface-soft);
}

.catalog-row svg,
.catalog-row-main svg {
  width: 15px;
  height: 15px;
}

.catalog-row span,
.catalog-row-main span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.catalog-row strong,
.catalog-row-main strong {
  min-width: 28px;
  padding: 1px 8px;
  border-radius: 999px;
  background: var(--bb-surface-soft);
  color: var(--bb-text-muted);
  font-size: 11px;
  text-align: center;
}

.catalog-row.group-row {
  grid-template-columns: minmax(0, 1fr) auto auto;
  padding: 0;
}

.catalog-row-main {
  display: grid;
  grid-template-columns: 16px 18px minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  min-width: 0;
  min-height: 32px;
  padding: 0 8px;
  border: 0;
  background: transparent;
  color: inherit;
  cursor: pointer;
  font: inherit;
  text-align: left;
}

.catalog-children {
  display: grid;
  gap: 4px;
  padding-left: 18px;
}

.graph-item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 4px;
  min-width: 0;
  padding: 4px;
  border-radius: 8px;
}

.graph-item-main {
  --bb-object-item-height: 28px;
  padding-inline: 4px;
}

.graph-run-command[data-status='queued'] svg,
.graph-run-command[data-status='running'] svg,
.graph-run-command[data-status='pending'] svg {
  animation: task-graph-spin 0.95s linear infinite;
}

.graph-favorite-command svg.filled {
  fill: currentColor;
  color: var(--bb-warning);
}

.graph-move-select {
  grid-column: 1 / -1;
  width: 100%;
  min-height: 26px;
  border: 1px solid var(--bb-border-warm);
  border-radius: 7px;
  background: var(--bb-surface);
  color: var(--bb-text-muted);
  font-size: 11px;
}

.catalog-empty {
  padding: 8px;
  border-radius: 8px;
  background: var(--bb-surface-soft);
  color: var(--bb-text-muted);
  font-size: 12px;
}

@keyframes task-graph-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
