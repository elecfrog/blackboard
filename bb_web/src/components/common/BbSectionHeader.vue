<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    title: string
    subtitle?: string
    eyebrow?: string
    count?: string | number
    as?: 'header' | 'div'
    titleTag?: 'h2' | 'h3' | 'h4' | 'h5'
    density?: 'default' | 'compact'
    divider?: boolean
  }>(),
  {
    subtitle: '',
    eyebrow: '',
    count: undefined,
    as: 'header',
    titleTag: 'h3',
    density: 'default',
    divider: true,
  },
)
</script>

<template>
  <component
    :is="props.as"
    class="bb-section-header"
    :data-density="props.density"
    :data-divider="props.divider ? 'true' : 'false'"
  >
    <div class="bb-section-header-copy">
      <span v-if="props.eyebrow" class="bb-section-header-eyebrow">{{ props.eyebrow }}</span>
      <div class="bb-section-header-title-row">
        <component :is="props.titleTag" class="bb-section-header-title">
          <slot name="title">{{ props.title }}</slot>
        </component>
        <span
          v-if="props.count !== undefined && props.count !== null && String(props.count).length > 0"
          class="bb-section-header-count"
        >
          {{ props.count }}
        </span>
        <slot name="meta" />
      </div>
      <p v-if="props.subtitle || $slots.subtitle" class="bb-section-header-subtitle">
        <slot name="subtitle">{{ props.subtitle }}</slot>
      </p>
    </div>

    <div v-if="$slots.actions" class="bb-section-header-actions">
      <slot name="actions" />
    </div>
  </component>
</template>
