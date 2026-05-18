<script setup lang="ts">
withDefaults(
  defineProps<{
    label: string
    checked?: boolean
    disabled?: boolean
  }>(),
  {
    checked: false,
    disabled: false,
  },
)

const emit = defineEmits<{
  'update:checked': [checked: boolean]
  change: [checked: boolean]
}>()

function handleChange(event: Event) {
  const checked = event.target instanceof HTMLInputElement ? event.target.checked : false
  emit('update:checked', checked)
  emit('change', checked)
}
</script>

<template>
  <label class="bb-checkbox-field">
    <input
      type="checkbox"
      :checked="checked"
      :disabled="disabled"
      @change="handleChange"
    />
    <span>{{ label }}</span>
  </label>
</template>
