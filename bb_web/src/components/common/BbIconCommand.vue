<script setup lang="ts">
import { computed } from 'vue'
import BbButton from './BbButton.vue'

const props = withDefaults(
  defineProps<{
    type?: 'button' | 'submit' | 'reset'
    title?: string
    ariaLabel?: string
    disabled?: boolean
    size?: 'sm' | 'md' | 'lg' | 'mini'
    variant?: 'neutral' | 'secondary' | 'ghost' | 'danger'
  }>(),
  {
    type: 'button',
    title: undefined,
    ariaLabel: undefined,
    disabled: false,
    size: 'md',
    variant: 'secondary',
  },
)

const emit = defineEmits<{
  click: [event: MouseEvent]
}>()

const resolvedAriaLabel = computed(() => props.ariaLabel ?? props.title)
</script>

<template>
  <BbButton
    class="bb-icon-command"
    :type="type"
    :variant="variant"
    :size="size"
    icon-only
    :disabled="disabled"
    :title="title"
    :aria-label="resolvedAriaLabel"
    @click="emit('click', $event)"
  >
    <slot />
  </BbButton>
</template>
