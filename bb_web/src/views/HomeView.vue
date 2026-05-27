<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import {
  loadProjects,
  openWorkspaceFolder,
  selectWorkspaceFolderFromDialog,
  type WorkspaceFolderInspection,
  type WorkspaceFolderOpenResult,
} from '@/data/tickets'
import { BbButton } from '@/components/common'
import { t } from '@/i18n'

interface WorkspaceFolderDialog {
  root: string
  folderName: string
  error: string
}

const router = useRouter()
const loading = ref(true)
const busy = ref(false)
const error = ref('')
const workspaceFolderDialog = ref<WorkspaceFolderDialog | null>(null)
const workspaceFolderSubmitting = ref(false)

onMounted(async () => {
  try {
    const payload = await loadProjects()
    const firstProject = payload.projects[0]?.name
    if (firstProject) {
      router.replace(`/projects/${firstProject}/tickets`)
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
})

async function openFolder() {
  if (busy.value) return
  busy.value = true
  error.value = ''
  try {
    const inspection = await selectWorkspaceFolderFromDialog()
    await handleWorkspaceFolderInspection(inspection)
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busy.value = false
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

  error.value = inspection.message || t('workspaceInvalid')
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
  const project = result.active_project || result.projects[0]?.name
  if (project) {
    await router.replace(`/projects/${project}/tickets`)
  }
}
</script>

<template>
  <main class="bb-home-shell">
    <section class="bb-home-empty">
      <div>
        <p class="bb-home-kicker">{{ t('workspace') }}</p>
        <h1>{{ t('workspaceEmptyTitle') }}</h1>
        <p>{{ loading ? t('loading') : t('workspaceEmptySubtitle') }}</p>
        <BbButton
          class="bb-home-primary-action"
          size="lg"
          variant="primary"
          :disabled="busy || loading"
          @click="openFolder"
        >
          {{ busy ? t('loading') : t('openFolder') }}
        </BbButton>
        <p v-if="error" class="bb-home-error" role="alert">{{ error }}</p>
        <p class="bb-home-hint">{{ t('workspaceGitignoreHint') }}</p>
      </div>
    </section>
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
  </main>
</template>

<style scoped>
.bb-home-shell {
  min-height: 100vh;
  display: grid;
  place-items: center;
  padding: 32px;
  color: var(--bb-text-strong);
  background: var(--bb-canvas);
}

.bb-home-empty {
  width: min(760px, 100%);
}

.bb-home-empty > div {
  display: grid;
  gap: 14px;
}

.bb-home-kicker {
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.bb-home-empty h1 {
  margin: 0;
  font-size: 56px;
  line-height: 1.02;
  letter-spacing: 0;
}

.bb-home-empty p {
  max-width: 620px;
  margin: 0;
  color: var(--bb-text-muted);
  font-size: 16px;
  line-height: 1.65;
}

.bb-home-primary-action {
  justify-self: flex-start;
}

.bb-home-error {
  padding: 10px 12px;
  border: 1px solid var(--bb-error);
  border-radius: 8px;
  background: var(--bb-md-error-bg);
  color: var(--bb-error) !important;
  font-size: 13px !important;
}

.bb-home-hint {
  font-size: 13px !important;
}

@media (max-width: 720px) {
  .bb-home-shell {
    padding: 24px;
    place-items: start;
  }

  .bb-home-empty h1 {
    font-size: 38px;
  }
}
</style>
