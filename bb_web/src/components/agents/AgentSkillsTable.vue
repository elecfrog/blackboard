<script setup lang="ts">
import { computed, ref } from 'vue'
import { t } from '@/i18n'

const props = defineProps<{
  skills: string[]
  editMode: boolean
}>()

const emit = defineEmits<{
  'update:skills': [skills: string[]]
}>()

const showAll = ref(false)
const maxVisible = 4

interface SkillDisplay {
  name: string
  category: string
  level: string
  levelPercent: number
}

function parseSkill(skill: string): SkillDisplay {
  // Skills are stored as simple strings; derive category/level heuristically
  const categories: Record<string, string> = {
    triage: 'Operations',
    'code-review': 'Engineering',
    coding: 'Engineering',
    planning: 'Management',
    testing: 'QA',
    documentation: 'Knowledge',
    architecture: 'Engineering',
    devops: 'Operations',
    design: 'Design',
  }
  const cat = categories[skill.toLowerCase()] || 'General'
  // Assign level based on position (first skills = expert)
  const idx = props.skills.indexOf(skill)
  const levels = ['Expert', 'Advanced', 'Intermediate', 'Beginner']
  const level = levels[Math.min(idx, levels.length - 1)]
  const percents: Record<string, number> = { Expert: 95, Advanced: 75, Intermediate: 55, Beginner: 30 }
  return { name: skill, category: cat, level, levelPercent: percents[level] }
}

const displaySkills = computed(() => props.skills.map(parseSkill))

const visibleSkills = computed(() => {
  if (showAll.value || displaySkills.value.length <= maxVisible) return displaySkills.value
  return displaySkills.value.slice(0, maxVisible)
})

function addSkill() {
  emit('update:skills', [...props.skills, ''])
}

function removeSkill(index: number) {
  emit('update:skills', props.skills.filter((_, i) => i !== index))
}

function updateSkill(index: number, value: string) {
  const updated = [...props.skills]
  updated[index] = value
  emit('update:skills', updated)
}

function levelColor(level: string): string {
  if (level === 'Expert') return '#22c55e'
  if (level === 'Advanced') return '#3b82f6'
  if (level === 'Intermediate') return '#f59e0b'
  return '#94a3b8'
}
</script>

<template>
  <div class="aw-section">
    <div class="aw-section-header">
      <h4>{{ t('skillsSectionTitle') }}</h4>
      <div class="aw-section-actions">
        <button v-if="editMode" type="button" class="aw-add-btn" @click="addSkill">+ {{ t('agentProfileAdd') }}</button>
      </div>
    </div>
    <template v-if="skills.length > 0">
      <table class="aw-table">
        <thead>
          <tr>
            <th>{{ t('skillsSkill') }}</th>
            <th>{{ t('skillsCategory') }}</th>
            <th>{{ t('skillsLevel') }}</th>
            <th v-if="editMode" />
          </tr>
        </thead>
        <tbody>
          <tr v-for="(skill, idx) in visibleSkills" :key="idx">
            <td>
              <template v-if="!editMode">{{ skill.name }}</template>
              <input v-else :value="skills[idx]" type="text" class="aw-table-input" :placeholder="t('skillsSkillPlaceholder')" @input="updateSkill(idx, ($event.target as HTMLInputElement).value)" />
            </td>
            <td>{{ skill.category }}</td>
            <td>
              <div class="aw-level-cell">
                <div class="aw-level-bar">
                  <div class="aw-level-fill" :style="{ width: skill.levelPercent + '%', background: levelColor(skill.level) }" />
                </div>
                <span class="aw-level-label" :style="{ color: levelColor(skill.level) }">{{ skill.level }}</span>
              </div>
            </td>
            <td v-if="editMode">
              <button type="button" class="aw-remove-btn" @click="removeSkill(idx)">✕</button>
            </td>
          </tr>
        </tbody>
      </table>
    </template>
    <div v-else class="aw-empty-panel">
      <div class="aw-empty-panel-icon">S</div>
      <div class="aw-empty-panel-body">
        <strong>{{ t('skillsNoConfigured') }}</strong>
        <p>{{ t('skillsAddHint') }}</p>
      </div>
    </div>
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
