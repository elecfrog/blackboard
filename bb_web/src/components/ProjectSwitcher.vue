<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import {
  loadProjects,
  openWorkspaceFolder,
  projectTypeLabels,
  selectWorkspaceFolderFromDialog,
  type ProjectEntry,
  type WorkspaceFolderInspection,
  type WorkspaceFolderOpenResult,
} from '@/data/tickets'
import { BbButton } from '@/components/common'
import UnifiedPopupSelect from '@/components/UnifiedPopupSelect.vue'
import { t } from '@/i18n'
import { projectBadgeTextColor, projectColor } from '@/utils/projectColor'

interface PopupSelectOption {
  value: string
  label: string
  description?: string
  badge?: string
  badgeColor?: string
  badgeTextColor?: string
}

interface WorkspaceFolderDialog {
  root: string
  folderName: string
  error: string
}

const props = defineProps<{
  project: string
}>()

const router = useRouter()
const route = useRoute()
const projects = ref<ProjectEntry[]>([])
const currentProject = ref(props.project)
const projectActionBusy = ref(false)
const projectActionError = ref('')
const workspaceFolderDialog = ref<WorkspaceFolderDialog | null>(null)
const workspaceFolderSubmitting = ref(false)

const projectOptions = computed<PopupSelectOption[]>(() => {
  const source = projects.value.length > 0
    ? projects.value
    : [{
        name: props.project,
        uuid: '',
        meta: { name: props.project, type: '', description: '', repos: [], data_root: undefined },
      }]

  return source.map((entry) => {
    const title = entry.meta.name || entry.name
    const typeLabel = projectTypeLabel(entry.meta.type || '')
    return {
      value: entry.name,
      label: title,
      description: [entry.name !== title ? entry.name : '', typeLabel].filter(Boolean).join(' · '),
      badge: projectBadge(title),
      badgeColor: projectColor(entry.name),
      badgeTextColor: projectBadgeTextColor(entry.name),
    }
  })
})

onMounted(async () => {
  try {
    const payload = await loadProjects()
    projects.value = payload.projects
  } catch (err) {
    console.error('failed to load project catalog', err)
  }
})

watch(
  () => props.project,
  (project) => {
    currentProject.value = project
  },
)

function onProjectChange(next: string) {
  if (!next || next === props.project) return
  router.push(projectRouteForCurrentSection(next))
}

async function onProjectFooterAction(action: string) {
  if (action !== 'open-folder') return
  if (projectActionBusy.value) return
  projectActionBusy.value = true
  projectActionError.value = ''
  try {
    const inspection = await selectWorkspaceFolderFromDialog()
    await handleWorkspaceFolderInspection(inspection)
  } catch (err) {
    projectActionError.value = err instanceof Error ? err.message : String(err)
  } finally {
    projectActionBusy.value = false
  }
}

async function handleWorkspaceFolderInspection(inspection: WorkspaceFolderInspection) {
  if (inspection.status === 'valid') {
    await navigateToWorkspaceResult(await openWorkspaceFolder({
      root: inspection.root,
      initialize: false,
    }))
    return
  }

  if (inspection.status === 'uninitialized') {
    workspaceFolderDialog.value = {
      root: inspection.root,
      folderName: inspection.suggested_display_name,
      error: '',
    }
    return
  }

  projectActionError.value = inspection.message || t('workspaceInvalid')
}

async function submitWorkspaceFolderDialog() {
  const dialog = workspaceFolderDialog.value
  if (!dialog || workspaceFolderSubmitting.value) return

  workspaceFolderSubmitting.value = true
  dialog.error = ''
  try {
    const result = await openWorkspaceFolder({
      root: dialog.root,
      initialize: true,
    })
    workspaceFolderDialog.value = null
    await navigateToWorkspaceResult(result)
  } catch (err) {
    dialog.error = err instanceof Error ? err.message : String(err)
  } finally {
    workspaceFolderSubmitting.value = false
  }
}

async function navigateToWorkspaceResult(result: WorkspaceFolderOpenResult) {
  projects.value = result.projects
  const project = result.active_project || result.projects[0]?.name
  if (!project) {
    router.push('/')
    return
  }
  currentProject.value = project
  router.push(projectRouteForCurrentSection(project))
}

function projectRouteForCurrentSection(project: string) {
  if (route.path.includes('/settings')) return `/projects/${project}/settings`
  if (route.path.includes('/inbox')) return `/projects/${project}/inbox`
  if (route.path.includes('/task-graphs')) return `/projects/${project}/task-graphs`
  if (route.path.includes('/tickets/graph')) return `/projects/${project}/tickets/graph`
  if (route.path.includes('/tickets/list')) return `/projects/${project}/tickets/list`
  if (route.path.includes('/tickets')) return `/projects/${project}/tickets`
  return `/projects/${project}/tickets`
}

function projectTypeLabel(kind: string) {
  return projectTypeLabels[kind] ?? kind
}

function projectBadge(label: string) {
  return (label.trim().charAt(0) || 'P').toUpperCase()
}

</script>

<template>
  <section class="bb-project-switcher">
    <div class="bb-side-nav-section">{{ t('projects') }}</div>
    <UnifiedPopupSelect
      v-model="currentProject"
      :label="t('project')"
      :placeholder="t('project')"
      :options="projectOptions"
      @change="onProjectChange"
      @footer-action="onProjectFooterAction"
    >
      <template #footer="{ action }">
        <div class="bb-project-menu-actions">
          <BbButton size="sm" variant="secondary" :disabled="projectActionBusy" @click.stop="action('open-folder')">
            {{ projectActionBusy ? t('loading') : t('openFolder') }}
          </BbButton>
        </div>
      </template>
    </UnifiedPopupSelect>
    <p v-if="projectActionError" class="bb-project-action-error" role="alert">
      {{ projectActionError }}
    </p>
    <Teleport to="body">
      <div
        v-if="workspaceFolderDialog"
        class="bb-project-dialog-overlay"
        role="dialog"
        aria-modal="true"
        @click.self="workspaceFolderDialog = null"
      >
        <form class="bb-project-dialog" @submit.prevent="submitWorkspaceFolderDialog">
          <header>
            <div>
              <h2>{{ t('workspaceInitializeTitle') }}</h2>
              <p>{{ workspaceFolderDialog.root }}</p>
            </div>
            <BbButton variant="secondary" @click="workspaceFolderDialog = null">
              {{ t('close') }}
            </BbButton>
          </header>
          <p class="bb-project-dialog-hint">
            {{ t('workspaceInitializeBody') }}
          </p>
          <p class="bb-project-dialog-hint">
            {{ t('workspaceInitializeName', { name: workspaceFolderDialog.folderName }) }}
          </p>
          <p class="bb-project-dialog-hint">
            {{ t('workspaceGitignoreHint') }}
          </p>
          <p v-if="workspaceFolderDialog.error" class="bb-project-dialog-error" role="alert">
            {{ workspaceFolderDialog.error }}
          </p>
          <footer>
            <BbButton variant="secondary" @click="workspaceFolderDialog = null">
              {{ t('close') }}
            </BbButton>
            <BbButton type="submit" variant="primary" :disabled="workspaceFolderSubmitting">
              {{ workspaceFolderSubmitting ? t('saving') : t('initialize') }}
            </BbButton>
          </footer>
        </form>
      </div>
    </Teleport>
  </section>
</template>
