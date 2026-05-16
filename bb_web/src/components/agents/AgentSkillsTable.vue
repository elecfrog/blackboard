<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Trash2, X } from 'lucide-vue-next'
import type { SkillInfo } from '@/data/agents'
import { t } from '@/i18n'

const props = defineProps<{
  skills: string[]
  availableSkills: SkillInfo[]
  saving?: boolean
  saveError?: string
}>()

const emit = defineEmits<{
  'update:skills': [skills: string[]]
}>()

const showAll = ref(false)
const adding = ref(false)
const pendingSkill = ref('')
const maxVisible = 4

const configuredSkills = computed(() => new Set(props.skills))

const availableOptions = computed(() =>
  props.availableSkills.filter((skill) => !configuredSkills.value.has(skill.name)),
)

const visibleSkills = computed(() => {
  const entries = props.skills.map((skill, index) => ({ skill, index }))
  if (showAll.value || entries.length <= maxVisible) return entries
  return entries.slice(0, maxVisible)
})

function addSkill() {
  if (props.saving) return
  adding.value = true
  pendingSkill.value = ''
  showAll.value = true
}

function removeSkill(index: number) {
  if (props.saving) return
  emit('update:skills', props.skills.filter((_, i) => i !== index))
}

function cancelAdd() {
  adding.value = false
  pendingSkill.value = ''
}

function addSelectedSkill(value: string) {
  if (!value || props.saving || configuredSkills.value.has(value)) return
  emit('update:skills', [...props.skills, value])
  cancelAdd()
}

watch(() => props.skills, cancelAdd)
</script>

<template>
  <div class="aw-section">
    <div class="aw-section-header">
      <h4>{{ t('skillsSectionTitle') }}</h4>
      <div class="aw-section-actions">
        <button
          type="button"
          class="aw-add-btn"
          :disabled="saving || adding"
          @click="addSkill"
        >
          + {{ saving ? t('saving') : t('skillsAdd') }}
        </button>
      </div>
    </div>
    <template v-if="skills.length > 0 || adding">
      <table class="aw-table">
        <thead>
          <tr>
            <th>{{ t('skillsSkill') }}</th>
            <th class="aw-table-action-col" />
          </tr>
        </thead>
        <tbody>
          <tr v-for="item in visibleSkills" :key="item.index">
            <td>
              {{ item.skill }}
            </td>
            <td class="aw-table-action-cell">
              <button
                type="button"
                class="aw-remove-btn"
                :disabled="saving"
                :aria-label="t('skillsRemove')"
                @click="removeSkill(item.index)"
              >
                <Trash2 aria-hidden="true" />
              </button>
            </td>
          </tr>
          <tr v-if="adding" class="aw-skill-draft-row">
            <td>
              <select
                v-model="pendingSkill"
                class="aw-table-select aw-skill-select"
                :disabled="saving || availableOptions.length === 0"
                @change="addSelectedSkill(($event.target as HTMLSelectElement).value)"
              >
                <option value="" disabled>{{ t('skillsSelectPlaceholder') }}</option>
                <option v-if="availableOptions.length === 0" value="" disabled>
                  {{ t('skillsNoAvailable') }}
                </option>
                <option v-for="skill in availableOptions" :key="skill.name" :value="skill.name">
                  {{ skill.name }}
                </option>
              </select>
            </td>
            <td class="aw-table-action-cell">
              <button
                type="button"
                class="aw-remove-btn"
                :disabled="saving"
                :aria-label="t('cancel')"
                @click="cancelAdd"
              >
                <X aria-hidden="true" />
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </template>
    <p v-else class="aw-empty-line">{{ t('skillsNoConfigured') }}</p>
    <p v-if="saveError" class="aw-inline-error">{{ saveError }}</p>
    <button
      v-if="!showAll && skills.length > maxVisible"
      type="button"
      class="aw-view-all"
      @click="showAll = true"
    >
      {{ t('skillsViewAll') }} ({{ skills.length }})
    </button>
  </div>
</template>
