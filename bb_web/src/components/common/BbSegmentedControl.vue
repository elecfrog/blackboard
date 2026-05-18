<script setup lang="ts">
export interface BbSegmentedOption {
  value: string
  label: string
  disabled?: boolean
}

const props = withDefaults(
  defineProps<{
    modelValue: string
    options: BbSegmentedOption[]
    ariaLabel?: string
    disabled?: boolean
  }>(),
  {
    ariaLabel: '',
    disabled: false,
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
  change: [value: string]
}>()

function selectOption(option: BbSegmentedOption) {
  if (props.disabled || option.disabled || option.value === props.modelValue) return
  emit('update:modelValue', option.value)
  emit('change', option.value)
}
</script>

<template>
  <div class="bb-segmented-control" role="group" :aria-label="ariaLabel">
    <button
      v-for="option in options"
      :key="option.value"
      type="button"
      class="bb-segmented-option"
      :data-active="option.value === modelValue ? 'true' : undefined"
      :disabled="disabled || option.disabled"
      @click="selectOption(option)"
    >
      {{ option.label }}
    </button>
  </div>
</template>
