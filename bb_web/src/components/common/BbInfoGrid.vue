<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    columns?: string
    variant?: 'cards' | 'rows'
  }>(),
  {
    columns: '',
    variant: 'cards',
  },
)
</script>

<template>
  <dl
    class="bb-info-grid"
    :data-variant="props.variant"
    :style="props.columns ? { '--bb-info-grid-columns': props.columns } : undefined"
  >
    <slot />
  </dl>
</template>
