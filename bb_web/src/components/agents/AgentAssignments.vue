<script setup lang="ts">
import { computed, ref } from 'vue'
import type { BlackboardTicket, ProjectEntry } from '@/data/tickets'
import { ticketRoute } from '@/data/tickets'
import { t } from '@/i18n'

export interface ProjectTickets {
  project: ProjectEntry
  tickets: BlackboardTicket[]
}

const props = defineProps<{
  projects: ProjectTickets[]
}>()

const groupBy = ref<'project'>('project')
const maxTicketsPerProject = 3

function statusDotClass(status: string): string {
  if (status === 'in_progress') return 'in-progress'
  if (status === 'review') return 'review'
  return 'todo'
}

function statusLabel(status: string): string {
  const labels: Record<string, string> = {
    in_progress: t('statusInProgress'),
    todo: t('statusTodo'),
    review: t('statusReview'),
    blocked: t('statusBlocked'),
    done: t('statusDone'),
  }
  return labels[status] || status
}

interface ProjectGroup {
  project: ProjectEntry
  tickets: BlackboardTicket[]
  hasMore: boolean
  total: number
}

const groupedProjects = computed<ProjectGroup[]>(() =>
  props.projects.map((entry) => ({
    project: entry.project,
    tickets: entry.tickets.slice(0, maxTicketsPerProject),
    hasMore: entry.tickets.length > maxTicketsPerProject,
    total: entry.tickets.length,
  })),
)
</script>

<template>
  <div class="aw-section">
    <div class="aw-section-header">
      <h4>{{ t('activeAssignments') }}</h4>
      <div class="aw-section-actions">
          <label class="aw-group-by">
          {{ t('groupByLabel') }}
          <select v-model="groupBy">
            <option value="project">{{ t('groupByProject') }}</option>
          </select>
        </label>
      </div>
    </div>
    <div v-if="groupedProjects.length === 0" class="aw-assignments-empty">
      {{ t('noActiveAssignments') }}
    </div>
    <div v-else class="aw-assignments-grid">
      <div v-for="group in groupedProjects" :key="group.project.name" class="aw-assignment-card">
        <header class="aw-assignment-card-header">
          <h5>{{ group.project.name }}</h5>
          <span class="aw-badge">{{ group.total }}</span>
        </header>
        <div class="aw-assignment-tickets">
          <a
            v-for="ticket in group.tickets"
            :key="ticket.id"
            class="aw-assignment-ticket"
            :href="`#${ticketRoute(group.project.name, ticket.id)}`"
          >
            <span class="aw-ticket-id">{{ ticket.id }}</span>
            <span class="aw-ticket-title">{{ ticket.title }}</span>
            <span class="aw-ticket-status">
              <span :class="['aw-status-dot', statusDotClass(ticket.status)]" />
              {{ statusLabel(ticket.status) }}
            </span>
          </a>
        </div>
        <a
          v-if="group.hasMore"
          class="aw-view-all"
          :href="`#/projects/${group.project.name}/board`"
        >
          {{ t('viewAll') }} ({{ group.total }})
        </a>
      </div>
    </div>
  </div>
</template>
