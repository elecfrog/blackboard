<script setup lang="ts">
withDefaults(
  defineProps<{
    align?: 'start' | 'end' | 'between'
    gap?: 'xs' | 'sm' | 'md'
    wrap?: boolean
  }>(),
  {
    align: 'end',
    gap: 'sm',
    wrap: true,
  },
)
</script>

<template>
  <div
    class="bb-action-group"
    :data-align="align"
    :data-gap="gap"
    :data-wrap="wrap ? 'true' : 'false'"
  >
    <slot />
  </div>
</template>
