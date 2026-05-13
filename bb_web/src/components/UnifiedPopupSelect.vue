<script setup lang="ts">
import BbDropdown, { type BbDropdownOption } from '@/components/BbDropdown.vue'

interface PopupSelectOption extends BbDropdownOption {
  value: string
  label: string
  description?: string
  badge?: string
  badgeColor?: string
  badgeTextColor?: string
  disabled?: boolean
}

const props = withDefaults(
  defineProps<{
    modelValue: string
    options: PopupSelectOption[]
    label: string
    placeholder?: string
    disabled?: boolean
  }>(),
  {
    placeholder: '',
    disabled: false,
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
  change: [value: string]
  'footer-action': [action: string]
}>()

function onChange(value: string) {
  emit('update:modelValue', value)
  emit('change', value)
}

function onFooterAction(action: string) {
  emit('footer-action', action)
}
</script>

<template>
  <BbDropdown
    class="bb-popup-select"
    :model-value="modelValue"
    :options="options"
    :label="label"
    :placeholder="placeholder"
    :disabled="disabled"
    @change="onChange"
    @footer-action="onFooterAction"
  >
    <template #footer="{ close, action }">
      <slot name="footer" :close="close" :action="action" />
    </template>
  </BbDropdown>
</template>
