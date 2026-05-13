<script setup lang="ts">
defineProps<{
  title: string
  subtitle?: string
}>()
</script>

<template>
  <header class="bb-nav">
    <div class="bb-nav-inner">
      <div>
        <h1>{{ title }}</h1>
        <p v-if="subtitle">{{ subtitle }}</p>
      </div>
      <div class="bb-nav-actions">
        <slot name="actions" />
      </div>
    </div>
  </header>
</template>
