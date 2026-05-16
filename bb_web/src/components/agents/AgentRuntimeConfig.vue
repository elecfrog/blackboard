<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { AgentProfile } from '@/data/agents'
import { upsertAgent } from '@/data/agents'
import { t } from '@/i18n'

const props = defineProps<{
  agent: AgentProfile
}>()

const emit = defineEmits<{
  updated: [agent: AgentProfile]
}>()

const editMode = ref(false)
const saving = ref(false)
const saveStatus = ref<'' | 'saved' | 'error'>('')
const saveError = ref('')
const editForm = ref({
  runtime: '',
  model: '',
  variant: '',
})

const runtimeDisplay = computed(() => props.agent.runtime || props.agent.kind || '—')
const modelDisplay = computed(() => props.agent.model || '—')
const variantDisplay = computed(() => props.agent.variant || '—')

function resetForm() {
  editForm.value = {
    runtime: props.agent.runtime ?? '',
    model: props.agent.model ?? '',
    variant: props.agent.variant ?? '',
  }
  saveStatus.value = ''
  saveError.value = ''
}

function openEdit() {
  resetForm()
  editMode.value = true
}

function cancelEdit() {
  editMode.value = false
  resetForm()
}

function optionalText(value: string): string | undefined {
  const trimmed = value.trim()
  return trimmed || undefined
}

async function saveEdit() {
  saving.value = true
  saveStatus.value = ''
  saveError.value = ''
  try {
    const updated = await upsertAgent({
      ...props.agent,
      runtime: optionalText(editForm.value.runtime),
      model: optionalText(editForm.value.model),
      variant: optionalText(editForm.value.variant),
    })
    emit('updated', updated)
    saveStatus.value = 'saved'
    setTimeout(() => {
      editMode.value = false
      saveStatus.value = ''
    }, 600)
  } catch (err) {
    saveStatus.value = 'error'
    saveError.value = err instanceof Error ? err.message : String(err)
  } finally {
    saving.value = false
  }
}

watch(() => props.agent.id, () => {
  editMode.value = false
  resetForm()
})

watch(
  () => [props.agent.runtime, props.agent.model, props.agent.variant],
  () => {
    if (!editMode.value) resetForm()
  },
)
</script>

<template>
  <div class="aw-section aw-runtime-section">
    <div class="aw-section-header">
      <h4>{{ t('runtimeConfigTitle') }}</h4>
      <div class="aw-section-actions">
        <template v-if="!editMode">
          <button type="button" class="aw-add-btn" @click="openEdit">{{ t('agentProfileEditInline') }}</button>
        </template>
        <template v-else>
          <span v-if="saveStatus === 'saved'" class="aw-save-ok">{{ t('agentProfileSaveOk') }}</span>
          <span v-if="saveStatus === 'error'" class="aw-save-err">{{ t('agentProfileSaveErr') }}</span>
          <button type="button" class="aw-btn secondary" @click="cancelEdit">{{ t('agentEditCancel') }}</button>
          <button type="button" class="aw-btn primary" :disabled="saving" @click="saveEdit">
            {{ saving ? t('agentEditSaving') : t('agentEditSave') }}
          </button>
        </template>
      </div>
    </div>

    <div class="aw-runtime-grid">
      <label class="aw-runtime-field">
        <span class="aw-meta-label">{{ t('runtimeConfigRuntimeLabel') }}</span>
        <span
          v-if="!editMode"
          :class="['aw-meta-value', { 'is-empty': !agent.runtime && !agent.kind }]"
          :title="runtimeDisplay"
        >
          {{ runtimeDisplay }}
        </span>
        <input
          v-else
          v-model="editForm.runtime"
          type="text"
          class="aw-meta-input"
          :placeholder="t('runtimeConfigRuntimePlaceholder')"
        />
      </label>

      <label class="aw-runtime-field">
        <span class="aw-meta-label">{{ t('runtimeConfigModelLabel') }}</span>
        <span
          v-if="!editMode"
          :class="['aw-meta-value', { 'is-empty': !agent.model }]"
          :title="agent.model || undefined"
        >
          {{ modelDisplay }}
        </span>
        <input
          v-else
          v-model="editForm.model"
          type="text"
          class="aw-meta-input"
          :placeholder="t('runtimeConfigModelPlaceholder')"
        />
      </label>

      <label class="aw-runtime-field">
        <span class="aw-meta-label">{{ t('runtimeConfigVariantLabel') }}</span>
        <span
          v-if="!editMode"
          :class="['aw-meta-value', { 'is-empty': !agent.variant }]"
          :title="agent.variant || undefined"
        >
          {{ variantDisplay }}
        </span>
        <input
          v-else
          v-model="editForm.variant"
          type="text"
          class="aw-meta-input"
          :placeholder="t('agentVariantPlaceholder')"
        />
      </label>
    </div>

    <p v-if="saveError" class="aw-inline-error">{{ saveError }}</p>
  </div>
</template>
