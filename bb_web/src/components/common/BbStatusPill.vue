<script setup lang="ts">
withDefaults(
  defineProps<{
    status: string
    label?: string
  }>(),
  {
    label: '',
  },
)
</script>

<template>
  <span class="bb-status-pill" :data-status="status">
    <slot>{{ label || status }}</slot>
  </span>
</template>
