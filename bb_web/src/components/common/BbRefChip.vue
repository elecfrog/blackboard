<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    label?: string
    value?: string
    title?: string
    interactive?: boolean
    disabled?: boolean
    tone?: 'reference' | 'neutral'
    size?: 'sm' | 'md'
  }>(),
  {
    label: '',
    value: '',
    title: undefined,
    interactive: false,
    disabled: false,
    tone: 'reference',
    size: 'md',
  },
)

const emit = defineEmits<{
  click: [event: MouseEvent]
}>()

function handleClick(event: MouseEvent) {
  if (props.disabled) {
    event.preventDefault()
    return
  }
  emit('click', event)
}
</script>

<template>
  <button
    v-if="interactive"
    class="bb-ref-chip"
    type="button"
    :data-tone="tone"
    :data-size="size"
    :data-interactive="interactive ? 'true' : undefined"
    :title="title"
    :disabled="disabled"
    @click="handleClick"
  >
    <slot>{{ label || value }}</slot>
  </button>
  <span
    v-else
    class="bb-ref-chip"
    :data-tone="tone"
    :data-size="size"
    :title="title"
  >
    <slot>{{ label || value }}</slot>
  </span>
</template>
