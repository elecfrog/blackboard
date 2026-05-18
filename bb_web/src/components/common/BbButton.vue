<script setup lang="ts">
withDefaults(
  defineProps<{
    type?: 'button' | 'submit' | 'reset'
    variant?: 'neutral' | 'primary' | 'secondary' | 'ghost' | 'danger'
    size?: 'sm' | 'md' | 'lg' | 'mini'
    iconOnly?: boolean
    disabled?: boolean
    title?: string
    ariaLabel?: string
  }>(),
  {
    type: 'button',
    variant: 'neutral',
    size: 'md',
    iconOnly: false,
    disabled: false,
    title: undefined,
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
    class="bb-button"
    :data-variant="variant"
    :data-size="size"
    :data-icon-only="iconOnly ? 'true' : undefined"
    :disabled="disabled"
    :title="title"
    :aria-label="ariaLabel || (iconOnly ? title : undefined)"
    @click="emit('click', $event)"
  >
    <span v-if="$slots.leading" class="bb-button-icon" aria-hidden="true">
      <slot name="leading" />
    </span>
    <span v-if="!iconOnly && $slots.default" class="bb-button-label">
      <slot />
    </span>
    <span v-if="$slots.trailing" class="bb-button-icon" aria-hidden="true">
      <slot name="trailing" />
    </span>
    <slot v-if="iconOnly" />
  </button>
</template>
