<script setup lang="ts">
withDefaults(
  defineProps<{
    tone?: 'info' | 'warning' | 'error' | 'success'
  }>(),
  {
    tone: 'info',
  },
)
</script>

<template>
  <p class="bb-inline-alert" :data-tone="tone" role="alert">
    <slot />
  </p>
</template>
