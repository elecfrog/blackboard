<script setup lang="ts">
withDefaults(
  defineProps<{
    label: string
    value?: string | number
    variant?: 'default' | 'metric' | 'mono'
    overflow?: 'wrap' | 'truncate'
  }>(),
  {
    value: '',
    variant: 'default',
    overflow: 'wrap',
  },
)
</script>

<template>
  <div class="bb-info-item" :data-variant="variant" :data-overflow="overflow">
    <dt>{{ label }}</dt>
    <dd>
      <slot>{{ value }}</slot>
    </dd>
  </div>
</template>
