<script setup lang="ts">
import BbButton from '@/components/common/BbButton.vue'

withDefaults(
  defineProps<{
    title: string
    subtitle?: string
    role?: 'dialog' | 'alertdialog'
    tone?: 'neutral' | 'danger' | 'warning'
    closeLabel?: string
    closeOnBackdrop?: boolean
  }>(),
  {
    subtitle: '',
    role: 'dialog',
    tone: 'neutral',
    closeLabel: 'Close',
    closeOnBackdrop: true,
  },
)

const emit = defineEmits<{
  close: []
}>()
</script>

<template>
  <div class="bb-dialog-backdrop" @click.self="closeOnBackdrop && emit('close')">
    <section
      class="bb-dialog"
      :data-tone="tone"
      :role="role"
      aria-modal="true"
    >
      <header class="bb-dialog-head">
        <div class="bb-dialog-title-block">
          <slot name="title">
            <h3>{{ title }}</h3>
            <p v-if="subtitle">{{ subtitle }}</p>
          </slot>
        </div>
        <BbButton
          variant="ghost"
          size="sm"
          icon-only
          :aria-label="closeLabel"
          @click="emit('close')"
        >
          <slot name="close-icon">×</slot>
        </BbButton>
      </header>
      <div class="bb-dialog-body">
        <slot />
      </div>
      <footer v-if="$slots.footer" class="bb-dialog-footer">
        <slot name="footer" />
      </footer>
    </section>
  </div>
</template>
