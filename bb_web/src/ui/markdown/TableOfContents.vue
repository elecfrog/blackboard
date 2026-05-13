<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import type { MarkdownThemeMode, TableOfContentsLabels } from './i18n'
import { resolveTableOfContentsLabels } from './i18n'

interface TocItem {
  id: string
  text: string
  level: number
}

interface Props {
  content: string
  locale?: string
  labels?: Partial<TableOfContentsLabels>
  theme?: MarkdownThemeMode
}

const props = defineProps<Props>()

const activeId = ref('')
const tocItems = ref<TocItem[]>([])
const resolvedLabels = computed(() => resolveTableOfContentsLabels(props.locale, props.labels))
const resolvedTheme = ref<'light' | 'dark'>('light')

function readDocumentTheme(): 'light' | 'dark' {
  if (props.theme === 'light' || props.theme === 'dark') return props.theme
  if (typeof document !== 'undefined') {
    const theme = document.documentElement.dataset.theme
    if (theme === 'dark' || theme === 'light') return theme
  }
  if (typeof window !== 'undefined' && window.matchMedia?.('(prefers-color-scheme: dark)').matches) {
    return 'dark'
  }
  return 'light'
}

function syncResolvedTheme() {
  resolvedTheme.value = readDocumentTheme()
}

// Parse markdown content to extract headings
const parseHeadings = (content: string): TocItem[] => {
  const headings: TocItem[] = []

  // Remove code blocks (``` or ``) to avoid false heading matches inside code
  // This regex matches fenced code blocks with optional language tag
  const strippedContent = content.replace(/^`{3,}.*$[\s\S]*?^`{3,}$/gm, '')

  const headingRegex = /^(#{1,6})\s+(.+)$/gm
  let match

  while ((match = headingRegex.exec(strippedContent)) !== null) {
    const level = match[1].length
    const text = match[2].trim()
    // Generate ID from text (same logic as markdown-it-anchor)
    const id = text
      .toLowerCase()
      .replace(/[^\w\u4e00-\u9fa5]+/g, '-')
      .replace(/^-+|-+$/g, '')

    headings.push({ id, text, level })
  }

  return headings
}

const visibleTocItems = computed(() => {
  return tocItems.value.length >= 3 ? tocItems.value : []
})

const scrollToHeading = (id: string) => {
  const element = document.getElementById(id)
  if (element) {
    element.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }
}

const handleScroll = () => {
  const headings = tocItems.value.map(item => ({
    id: item.id,
    element: document.getElementById(item.id),
  }))

  // Find the heading that's currently in view
  const scrollPosition = window.scrollY + 100

  for (let i = headings.length - 1; i >= 0; i--) {
    const heading = headings[i]
    if (heading.element && heading.element.offsetTop <= scrollPosition) {
      activeId.value = heading.id
      return
    }
  }

  if (headings.length > 0 && headings[0].element) {
    activeId.value = headings[0].id
  }
}

let themeObserver: MutationObserver | null = null
let mediaQuery: MediaQueryList | null = null

onMounted(() => {
  syncResolvedTheme()
  if (typeof document !== 'undefined') {
    themeObserver = new MutationObserver(syncResolvedTheme)
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme'],
    })
  }
  if (typeof window !== 'undefined') {
    mediaQuery = window.matchMedia?.('(prefers-color-scheme: dark)') ?? null
    mediaQuery?.addEventListener?.('change', syncResolvedTheme)
  }
  tocItems.value = parseHeadings(props.content)
  window.addEventListener('scroll', handleScroll)
  handleScroll()
})

onUnmounted(() => {
  themeObserver?.disconnect()
  mediaQuery?.removeEventListener?.('change', syncResolvedTheme)
  window.removeEventListener('scroll', handleScroll)
})

watch(
  () => props.content,
  (newContent) => {
    tocItems.value = parseHeadings(newContent)
    handleScroll()
  }
)

watch(
  () => props.theme,
  syncResolvedTheme,
)
</script>

<template>
  <div v-if="visibleTocItems.length > 0" class="toc-container" :data-markdown-theme="resolvedTheme">
    <div class="toc-title">{{ resolvedLabels.title }}</div>
    <nav class="toc-nav">
      <ul class="toc-list">
        <li
          v-for="item in visibleTocItems"
          :key="item.id"
          :class="['toc-item', `toc-level-${item.level}`, { active: activeId === item.id }]"
          @click="scrollToHeading(item.id)"
        >
          <a :href="`#${item.id}`" @click.prevent>{{ item.text }}</a>
        </li>
      </ul>
    </nav>
  </div>
</template>

<style scoped>
.toc-container {
  background-color: var(--bb-md-surface, var(--bg-card));
  border: 1px solid var(--bb-md-border, var(--border-color));
  border-radius: 8px;
  box-shadow: none;
  padding: 16px;
  max-height: 400px;
  overflow-y: auto;
}

.toc-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--bb-md-text-strong, var(--text-primary));
  margin-bottom: 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--bb-md-border, var(--border-color));
}

.toc-nav {
  font-size: 13px;
}

.toc-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.toc-item {
  cursor: pointer;
  color: var(--bb-md-text-muted, var(--text-secondary));
  transition: all 0.2s;
  padding: 4px 0;
  line-height: 1.5;
}

.toc-item:hover {
  color: var(--bb-md-link, var(--color-primary));
}

.toc-item a {
  text-decoration: none;
  color: inherit;
}

.toc-item.active {
  color: var(--bb-md-link, var(--color-primary));
  font-weight: 500;
}

.toc-level-1 { padding-left: 0; }
.toc-level-2 { padding-left: 12px; }
.toc-level-3 { padding-left: 24px; }
.toc-level-4 { padding-left: 36px; }
.toc-level-5 { padding-left: 48px; }
.toc-level-6 { padding-left: 60px; }
</style>
