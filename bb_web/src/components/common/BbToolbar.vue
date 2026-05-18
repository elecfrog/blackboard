<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    as?: 'header' | 'div' | 'section'
    variant?: 'bar' | 'inline'
    density?: 'default' | 'compact'
    sticky?: boolean
  }>(),
  {
    as: 'div',
    variant: 'inline',
    density: 'default',
    sticky: false,
  },
)
</script>

<template>
  <component
    :is="props.as"
    class="bb-toolbar"
    :data-variant="props.variant"
    :data-density="props.density"
    :data-sticky="props.sticky ? 'true' : 'false'"
  >
    <div v-if="$slots.leading" class="bb-toolbar-leading">
      <slot name="leading" />
    </div>
    <div v-if="$slots.search" class="bb-toolbar-search">
      <slot name="search" />
    </div>
    <div v-if="$slots.meta" class="bb-toolbar-meta">
      <slot name="meta" />
    </div>
    <div v-if="$slots.actions" class="bb-toolbar-actions">
      <slot name="actions" />
    </div>
    <slot />
  </component>
</template>
