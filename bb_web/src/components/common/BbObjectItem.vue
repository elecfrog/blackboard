<script setup lang="ts">
withDefaults(
  defineProps<{
    title: string
    active?: boolean
    ariaCurrent?: 'true' | 'false' | 'page' | 'step' | 'location' | 'date' | 'time'
    disabled?: boolean
  }>(),
  {
    active: false,
    ariaCurrent: 'true',
    disabled: false,
  },
)

const emit = defineEmits<{
  select: []
}>()
</script>

<template>
  <button
    type="button"
    class="bb-object-item"
    :class="{ 'is-active': active }"
    :aria-current="active ? ariaCurrent : undefined"
    :disabled="disabled"
    @click="emit('select')"
  >
    <span v-if="$slots.leading" class="bb-object-item-leading" aria-hidden="true">
      <slot name="leading" />
    </span>
    <span class="bb-object-item-title">{{ title }}</span>
    <span v-if="$slots.trailing" class="bb-object-item-trailing" aria-hidden="true">
      <slot name="trailing" />
    </span>
  </button>
</template>
