<script setup lang="ts">
import { computed } from 'vue'
import { t } from '@/i18n'
import type {
  GraphMutationBatchResult,
  GraphMutationSummary,
  TaskGraphRunEvent,
  TaskGraphSuperstepCheckpoint,
  TopologyMutationEventPayload,
} from '@/data/taskGraphs'

const props = defineProps<{
  events: TaskGraphRunEvent[]
  checkpoints: TaskGraphSuperstepCheckpoint[]
}>()

interface MutationTimelineItem {
  id: string
  superstep: number
  batchId: string
  revisionBefore: number
  revisionAfter: number
  result: GraphMutationBatchResult
  message: string
  createdAt: string
}

const latestCheckpoint = computed(() =>
  [...props.checkpoints].sort((left, right) => {
    const leftTime = Date.parse(left.completed_at ?? left.created_at)
    const rightTime = Date.parse(right.completed_at ?? right.created_at)
    return right.superstep - left.superstep || rightTime - leftTime
  })[0] ?? null,
)

const timelineItems = computed<MutationTimelineItem[]>(() =>
  props.events
    .filter((event) => event.kind === 'topology_mutation')
    .map((event) => {
      const payload = topologyMutationPayload(event.payload)
      if (!payload) return null
      return {
        id: event.id,
        superstep: event.superstep,
        batchId: payload.batch_id,
        revisionBefore: payload.graph_revision_before,
        revisionAfter: payload.graph_revision_after,
        result: payload.result,
        message: event.message,
        createdAt: event.created_at,
      }
    })
    .filter((item): item is MutationTimelineItem => item !== null)
    .sort((left, right) => right.superstep - left.superstep || Date.parse(right.createdAt) - Date.parse(left.createdAt)),
)

function topologyMutationPayload(value: unknown): TopologyMutationEventPayload | null {
  if (!value || typeof value !== 'object') return null
  const payload = value as Partial<TopologyMutationEventPayload>
  if (typeof payload.batch_id !== 'string') return null
  if (typeof payload.graph_revision_before !== 'number' || typeof payload.graph_revision_after !== 'number') return null
  if (!payload.result || typeof payload.result !== 'object') return null
  const result = payload.result as Partial<GraphMutationBatchResult>
  if (result.status !== 'applied' && result.status !== 'rejected') return null
  return payload as TopologyMutationEventPayload
}

function summaryEntries(summary: GraphMutationSummary) {
  return [
    { key: 'added_nodes', label: t('taskGraphMutationAddedNodes'), value: summary.added_nodes },
    { key: 'removed_nodes', label: t('taskGraphMutationRemovedNodes'), value: summary.removed_nodes },
    { key: 'added_edges', label: t('taskGraphMutationAddedEdges'), value: summary.added_edges },
    { key: 'removed_edges', label: t('taskGraphMutationRemovedEdges'), value: summary.removed_edges },
    { key: 'patched_nodes', label: t('taskGraphMutationPatchedNodes'), value: summary.patched_nodes },
  ].filter((entry) => entry.value.length > 0)
}
</script>

<template>
  <section class="task-graph-mutation-timeline">
    <header>
      <h5>{{ t('taskGraphRunTimeline') }}</h5>
      <span v-if="latestCheckpoint">
        {{ t('taskGraphLatestCheckpoint') }} {{ latestCheckpoint.superstep }}
        · {{ latestCheckpoint.graph_revision_before }} -> {{ latestCheckpoint.graph_revision_after }}
      </span>
      <span v-else>{{ t('taskGraphNoCheckpoint') }}</span>
    </header>

    <p v-if="timelineItems.length === 0" class="task-graph-mutation-empty">
      {{ t('taskGraphNoTopologyMutation') }}
    </p>

    <div v-else class="task-graph-mutation-list">
      <article
        v-for="item in timelineItems"
        :key="item.id"
        class="task-graph-mutation-item"
        :data-status="item.result.status"
      >
        <header>
          <span>{{ t('taskGraphSuperstep') }} {{ item.superstep }}</span>
          <code>{{ item.batchId }}</code>
        </header>
        <div class="task-graph-mutation-meta">
          <strong>{{ item.revisionBefore }} -> {{ item.revisionAfter }}</strong>
          <span>{{ item.result.status === 'applied' ? t('taskGraphMutationApplied') : t('taskGraphMutationRejected') }}</span>
        </div>
        <div v-if="item.result.status === 'applied'" class="task-graph-mutation-summary">
          <span
            v-for="entry in summaryEntries(item.result.summary)"
            :key="entry.key"
          >
            {{ entry.label }} {{ entry.value.join(', ') }}
          </span>
        </div>
        <div v-else class="task-graph-mutation-conflicts">
          <p
            v-for="conflict in item.result.conflicts"
            :key="`${item.id}:${conflict.code}:${conflict.request_ids.join('-')}`"
          >
            <strong>{{ conflict.code }}</strong>
            <span>{{ conflict.message }}</span>
          </p>
        </div>
      </article>
    </div>
  </section>
</template>

<style scoped>
.task-graph-mutation-timeline {
  display: grid;
  gap: 7px;
  min-width: 0;
}

.task-graph-mutation-timeline > header,
.task-graph-mutation-item > header,
.task-graph-mutation-meta,
.task-graph-mutation-summary {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  min-width: 0;
}

.task-graph-mutation-timeline h5 {
  margin: 0;
  color: var(--bb-text-strong);
}

.task-graph-mutation-timeline > header span,
.task-graph-mutation-empty,
.task-graph-mutation-item code,
.task-graph-mutation-meta span,
.task-graph-mutation-summary span,
.task-graph-mutation-conflicts span {
  color: var(--bb-text-muted);
  font-size: 12px;
}

.task-graph-mutation-empty {
  margin: 0;
}

.task-graph-mutation-list {
  display: grid;
  gap: 6px;
}

.task-graph-mutation-item {
  display: grid;
  gap: 5px;
  min-width: 0;
  padding: 8px;
  border: 1px solid color-mix(in srgb, var(--bb-hairline) 88%, transparent);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bb-surface-muted) 56%, var(--bb-surface));
}

.task-graph-mutation-item[data-status='applied'] {
  border-color: color-mix(in srgb, var(--bb-accent) 20%, transparent);
}

.task-graph-mutation-item[data-status='rejected'] {
  border-color: color-mix(in srgb, var(--bb-error) 24%, transparent);
}

.task-graph-mutation-item > header {
  justify-content: space-between;
  color: var(--bb-text-strong);
  font-size: 12px;
  font-weight: 760;
}

.task-graph-mutation-meta strong {
  color: var(--bb-text-strong);
  font-size: 12px;
}

.task-graph-mutation-summary span {
  display: inline-flex;
  min-width: 0;
  max-width: 100%;
  padding: 2px 6px;
  border-radius: 6px;
  background: var(--bb-surface-soft);
  overflow-wrap: anywhere;
}

.task-graph-mutation-conflicts {
  display: grid;
  gap: 4px;
}

.task-graph-mutation-conflicts p {
  display: grid;
  gap: 2px;
  margin: 0;
  overflow-wrap: anywhere;
}

.task-graph-mutation-conflicts strong {
  color: var(--bb-error);
  font-size: 12px;
}
</style>
