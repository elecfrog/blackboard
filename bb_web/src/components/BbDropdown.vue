<script setup lang="ts">
import { computed, h, ref, useSlots, watch, type CSSProperties } from 'vue'
import { Dropdown, type DropdownOption } from 'tdesign-vue-next/es/dropdown'
import { ChevronDown } from 'lucide-vue-next'
import { t } from '@/i18n'

export interface BbDropdownOption {
  value: string
  label: string
  description?: string
  badge?: string
  badgeColor?: string
  badgeTextColor?: string
  disabled?: boolean
  divider?: boolean
}

const props = withDefaults(
  defineProps<{
    modelValue: string
    options: BbDropdownOption[]
    label: string
    placeholder?: string
    disabled?: boolean
    placement?: 'bottom-left' | 'bottom-right' | 'top-left' | 'top-right' | 'bottom' | 'top'
    minWidth?: number
    maxHeight?: number
  }>(),
  {
    placeholder: '',
    disabled: false,
    placement: 'bottom-left',
    minWidth: 220,
    maxHeight: 320,
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
  change: [value: string]
  'footer-action': [action: string]
}>()

const slots = useSlots()
const visible = ref(false)

const selectedOption = computed(() =>
  props.options.find((option) => option.value === props.modelValue),
)

const displayPlaceholder = computed(() => props.placeholder || t('popupSelectPlaceholder'))

watch(
  () => props.disabled,
  (disabled) => {
    if (disabled) visible.value = false
  },
)

function optionBadgeStyle(option: BbDropdownOption): CSSProperties | undefined {
  if (!option.badgeColor && !option.badgeTextColor) return undefined
  return {
    backgroundColor: option.badgeColor,
    color: option.badgeTextColor,
  }
}

function renderOptionContent(option: BbDropdownOption) {
  return h('span', { class: 'bb-dropdown-option-content' }, [
    option.badge
      ? h('span', {
        class: 'bb-popup-select-badge bb-dropdown-option-badge',
        style: optionBadgeStyle(option),
      }, option.badge)
      : null,
    h('span', { class: 'bb-popup-select-copy bb-dropdown-option-copy' }, [
      h('strong', option.label),
      option.description ? h('small', option.description) : null,
    ]),
  ])
}

const dropdownOptions = computed<DropdownOption[]>(() =>
  props.options.map((option) => ({
    value: option.value,
    content: () => renderOptionContent(option),
    disabled: option.disabled,
    divider: option.divider,
    active: option.value === props.modelValue,
  })),
)

function close() {
  visible.value = false
}

function onSelect(option: DropdownOption) {
  const value = typeof option.value === 'string' ? option.value : ''
  if (!value) return
  emit('update:modelValue', value)
  emit('change', value)
}

function onFooterAction(action: string) {
  emit('footer-action', action)
  close()
}

const footerContent = computed(() => {
  if (!slots.footer) return undefined
  return () => slots.footer?.({ close, action: onFooterAction })
})

const popupProps = computed(() => ({
  visible: visible.value,
  overlayClassName: 'bb-dropdown-popup',
  overlayInnerClassName: 'bb-dropdown-panel',
  overlayInnerStyle: (triggerElement: HTMLElement) => {
    const width = Math.max(triggerElement.offsetWidth, props.minWidth)
    return { width: `${width}px` }
  },
  onVisibleChange: (nextVisible: boolean) => {
    if (props.disabled) return
    visible.value = nextVisible
  },
}))
</script>

<template>
  <div :class="['bb-dropdown', { disabled }]">
    <Dropdown
      :options="dropdownOptions"
      :disabled="disabled"
      :hide-after-item-click="true"
      :max-height="maxHeight"
      :min-column-width="minWidth"
      :placement="placement"
      :panel-bottom-content="footerContent"
      :popup-props="popupProps"
      trigger="click"
      @click="onSelect"
    >
      <button
        class="bb-popup-select-trigger bb-dropdown-trigger"
        type="button"
        :aria-expanded="visible"
        :aria-label="label"
        :disabled="disabled"
      >
        <span
          v-if="selectedOption?.badge"
          class="bb-popup-select-badge"
          :style="optionBadgeStyle(selectedOption)"
        >
          {{ selectedOption.badge }}
        </span>
        <span class="bb-popup-select-copy">
          <strong>{{ selectedOption?.label || displayPlaceholder }}</strong>
          <small v-if="selectedOption?.description">{{ selectedOption.description }}</small>
        </span>
        <ChevronDown
          :class="['bb-popup-select-chevron', 'bb-dropdown-chevron', { open: visible }]"
          aria-hidden="true"
        />
      </button>
    </Dropdown>
  </div>
</template>
