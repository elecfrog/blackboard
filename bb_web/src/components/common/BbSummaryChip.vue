<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    type?: 'button' | 'submit' | 'reset'
    title: string
    subtitle?: string
    active?: boolean
    disabled?: boolean
    minWidth?: string
    ariaLabel?: string
  }>(),
  {
    type: 'button',
    subtitle: '',
    active: false,
    disabled: false,
    minWidth: undefined,
    ariaLabel: undefined,
  },
)

const emit = defineEmits<{
  click: [event: MouseEvent]
}>()
</script>

<template>
  <button
    :type="type"
    class="bb-summary-chip"
    :data-active="active ? 'true' : undefined"
    :disabled="disabled"
    :aria-label="ariaLabel"
    :aria-pressed="active"
    :style="props.minWidth ? { '--bb-summary-chip-min-width': props.minWidth } : undefined"
    @click="emit('click', $event)"
  >
    <span v-if="$slots.icon" class="bb-summary-chip-icon" aria-hidden="true">
      <slot name="icon" />
    </span>
    <span class="bb-summary-chip-copy">
      <strong>{{ title }}</strong>
      <small v-if="subtitle">{{ subtitle }}</small>
    </span>
  </button>
</template>
