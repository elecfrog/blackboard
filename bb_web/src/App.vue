<template>
  <RouterView />
  <AgentChatFab />
</template>

<script setup lang="ts">
import { defineAsyncComponent } from 'vue'

const AgentChatFab = defineAsyncComponent(() => import('@/components/AgentChatFab.vue'))
</script>
