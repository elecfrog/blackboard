<script setup lang="ts">
withDefaults(
  defineProps<{
    title?: string
    message?: string
    tone?: 'neutral' | 'error'
  }>(),
  {
    title: '',
    message: '',
    tone: 'neutral',
  },
)
</script>

<template>
  <div class="bb-empty-state" :data-tone="tone">
    <strong v-if="title">{{ title }}</strong>
    <span v-if="message">{{ message }}</span>
    <slot />
  </div>
</template>
