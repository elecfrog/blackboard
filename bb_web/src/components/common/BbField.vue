<script setup lang="ts">
withDefaults(
  defineProps<{
    label?: string
    note?: string
    error?: string
    inline?: boolean
  }>(),
  {
    label: '',
    note: '',
    error: '',
    inline: false,
  },
)
</script>

<template>
  <label class="bb-field" :data-inline="inline ? 'true' : undefined">
    <span v-if="label || $slots.label" class="bb-field-label">
      <slot name="label">{{ label }}</slot>
    </span>
    <slot />
    <small v-if="error" class="bb-field-error">{{ error }}</small>
    <small v-else-if="note || $slots.note" class="bb-field-note">
      <slot name="note">{{ note }}</slot>
    </small>
  </label>
</template>
