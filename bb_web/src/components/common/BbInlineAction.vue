<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    href?: string
    target?: string
    rel?: string
    type?: 'button' | 'submit' | 'reset'
    disabled?: boolean
  }>(),
  {
    href: '',
    target: undefined,
    rel: undefined,
    type: 'button',
    disabled: false,
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
  <a
    v-if="href && !disabled"
    class="bb-inline-action"
    :href="href"
    :target="target"
    :rel="rel"
    @click="handleClick"
  >
    <slot />
  </a>
  <button
    v-else
    class="bb-inline-action"
    :type="type"
    :disabled="disabled"
    :aria-disabled="disabled ? 'true' : undefined"
    @click="handleClick"
  >
    <slot />
  </button>
</template>
