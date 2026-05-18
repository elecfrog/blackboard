<script setup lang="ts">
import { computed, ref } from 'vue'
import type { BlackboardTicket, ProjectEntry } from '@/data/tickets'
import { ticketRoute } from '@/data/tickets'
import { BbInlineAction, BbSectionHeader } from '@/components/common'
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
    <BbSectionHeader class="aw-section-header" :title="t('activeAssignments')" title-tag="h4" :divider="false">
      <template #actions>
        <label class="aw-group-by">
          {{ t('groupByLabel') }}
          <select v-model="groupBy">
            <option value="project">{{ t('groupByProject') }}</option>
          </select>
        </label>
      </template>
    </BbSectionHeader>
    <p v-if="groupedProjects.length === 0" class="aw-empty-line">
      {{ t('noActiveAssignments') }}
    </p>
    <div v-else class="aw-assignment-groups">
      <div v-for="group in groupedProjects" :key="group.project.name" class="aw-assignment-group">
        <header class="aw-assignment-group-header">
          <h5>{{ group.project.name }}</h5>
          <span class="aw-assignment-count">{{ group.total }}</span>
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
        <BbInlineAction
          v-if="group.hasMore"
          :href="`#/projects/${group.project.name}/board`"
        >
          {{ t('viewAll') }} ({{ group.total }})
        </BbInlineAction>
      </div>
    </div>
  </div>
</template>
