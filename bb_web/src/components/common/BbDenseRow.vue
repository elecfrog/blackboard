<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    columns?: string
    gap?: string
    padding?: string
    controlHeight?: string
  }>(),
  {
    columns: '',
    gap: '',
    padding: '',
    controlHeight: '',
  },
)

const rowStyle = computed(() => {
  const style: Record<string, string> = {}
  if (props.columns) style['--bb-dense-row-columns'] = props.columns
  if (props.gap) style['--bb-dense-row-gap'] = props.gap
  if (props.padding) style['--bb-dense-row-padding'] = props.padding
  if (props.controlHeight) style['--bb-dense-control-height'] = props.controlHeight
  return style
})
</script>

<template>
  <div class="bb-dense-row" :style="rowStyle">
    <slot />
  </div>
</template>
