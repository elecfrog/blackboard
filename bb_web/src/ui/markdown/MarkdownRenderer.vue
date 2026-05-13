<script setup lang="ts">
import { computed, ref, onMounted, watch, nextTick, onBeforeUnmount } from 'vue'
import MarkdownIt from 'markdown-it'
import hljs from 'highlight.js/lib/core'
import javascript from 'highlight.js/lib/languages/javascript'
import typescript from 'highlight.js/lib/languages/typescript'
import python from 'highlight.js/lib/languages/python'
import rust from 'highlight.js/lib/languages/rust'
import go from 'highlight.js/lib/languages/go'
import bash from 'highlight.js/lib/languages/bash'
import json from 'highlight.js/lib/languages/json'
import yaml from 'highlight.js/lib/languages/yaml'
import markdown from 'highlight.js/lib/languages/markdown'
import sql from 'highlight.js/lib/languages/sql'
import xml from 'highlight.js/lib/languages/xml'
import css from 'highlight.js/lib/languages/css'
import c from 'highlight.js/lib/languages/c'
import cpp from 'highlight.js/lib/languages/cpp'
import java from 'highlight.js/lib/languages/java'

hljs.registerLanguage('javascript', javascript)
hljs.registerLanguage('js', javascript)
hljs.registerLanguage('typescript', typescript)
hljs.registerLanguage('ts', typescript)
hljs.registerLanguage('python', python)
hljs.registerLanguage('py', python)
hljs.registerLanguage('rust', rust)
hljs.registerLanguage('rs', rust)
hljs.registerLanguage('go', go)
hljs.registerLanguage('bash', bash)
hljs.registerLanguage('sh', bash)
hljs.registerLanguage('shell', bash)
hljs.registerLanguage('json', json)
hljs.registerLanguage('yaml', yaml)
hljs.registerLanguage('yml', yaml)
hljs.registerLanguage('markdown', markdown)
hljs.registerLanguage('md', markdown)
hljs.registerLanguage('sql', sql)
hljs.registerLanguage('html', xml)
hljs.registerLanguage('xml', xml)
hljs.registerLanguage('css', css)
hljs.registerLanguage('c', c)
hljs.registerLanguage('cpp', cpp)
hljs.registerLanguage('java', java)
import anchor from 'markdown-it-anchor'
import type Token from 'markdown-it/lib/token.mjs'

import taskLists from 'markdown-it-task-lists'
import type { MarkdownThemeMode, MarkdownRendererLabels } from './i18n'
import { resolveMarkdownLabels } from './i18n'
import { slugifyHeading } from './slugify'

interface Props {
  content: string
  locale?: string
  labels?: Partial<MarkdownRendererLabels>
  theme?: MarkdownThemeMode
}

const props = defineProps<Props>()
const labels = computed(() => resolveMarkdownLabels(props.locale, props.labels))
const resolvedTheme = ref<'light' | 'dark'>('light')
const renderEpoch = ref(0)

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

function mermaidTheme() {
  return resolvedTheme.value === 'dark' ? 'dark' : 'default'
}

const md: MarkdownIt = new MarkdownIt({
  html: true,
  linkify: true,
  typographer: true,
  highlight: (str: string, lang: string): string => {
    const langLabel = lang ? `<span class="code-lang">${md.utils.escapeHtml(lang)}</span>` : ''
    const safeCopy = md.utils.escapeHtml(labels.value.copyCode)
    const copyBtn = `<button class="code-copy" title="${safeCopy}" aria-label="${safeCopy}" type="button">
      <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <rect width="14" height="14" x="8" y="8" rx="2" ry="2"></rect>
        <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path>
      </svg>
    </button>`
    const toolbar = `<div class="code-toolbar">${langLabel}${copyBtn}</div>`
    if (lang && hljs.getLanguage(lang)) {
      try {
        return `<pre class="hljs">${toolbar}<code>${hljs.highlight(str, { language: lang, ignoreIllegals: true }).value}</code></pre>`
      } catch (err) {
        console.error(err)
      }
    }
    return `<pre class="hljs">${toolbar}<code>${md.utils.escapeHtml(str)}</code></pre>`
  },
})

// Add anchor plugin for heading IDs
md.use(anchor, {
  slugify: (s: string) => slugifyHeading(s),
  permalink: anchor.permalink.ariaHidden({
    placement: 'before',
    class: 'header-anchor',
  }),
  callback: (token: Token, info: { slug: string; title: string }) => {
    // Ensure heading tokens have the correct id
    if (token.attrGet('id') === null) {
      token.attrSet('id', info.slug)
    }
  }
})

// Add task list (checkbox) plugin - readonly mode
md.use(taskLists, {
  enabled: false,
  label: true,
  labelAfter: true,
})

// Custom fence renderer: intercept mermaid code blocks
const defaultFence = md.renderer.rules.fence
md.renderer.rules.fence = (
  tokens: Token[],
  idx: number,
  options: any,
  env: unknown,
  self: any,
): string => {
  const token = tokens[idx]
  const langName = token.info.trim().toLowerCase()
  if (langName === 'mermaid') {
    const encoded = md.utils.escapeHtml(token.content)
    const loading = md.utils.escapeHtml(labels.value.mermaidLoading)
    return `<div class="mermaid-block" data-mermaid="${encoded}"><div class="mermaid-loading">${loading}</div></div>\n`
  }
  if (defaultFence) {
    return defaultFence(tokens, idx, options, env, self)
  }
  return self.renderToken(tokens, idx, options)
}

const markdownBodyRef = ref<HTMLDivElement>()

// Mermaid lightbox state
const showLightbox = ref(false)
const lightboxSvg = ref('')  // Original SVG string
const lightboxPngUrl = ref('')  // PNG data URL
const lightboxZoom = ref(1.0)
const lightboxSvgStyle = ref({ transform: 'scale(1)', transformOrigin: 'center center' })
let lightboxSvgEl: HTMLElement | null = null

// Drag state
const isDragging = ref(false)
const dragStartX = ref(0)
const dragStartY = ref(0)
const imagePosition = ref({ x: 0, y: 0 })

const renderedContent = computed(() => {
  const currentLabels = labels.value
  try {
    return md.render(props.content)
  } catch (err) {
    console.error('Failed to render markdown:', err)
    return `<div class="error">${md.utils.escapeHtml(currentLabels.renderError)}</div>`
  }
})

const renderKey = computed(() =>
  `${renderEpoch.value}-${resolvedTheme.value}-${props.locale ?? 'auto'}`,
)

// Mermaid rendering: dynamically load and render mermaid diagrams
let mermaidInstance: any = null
let mermaidIdCounter = 0

async function loadMermaid() {
  if (mermaidInstance) return mermaidInstance
  const mermaid = (await import('mermaid')).default
  mermaidInstance = mermaid
  return mermaid
}

function configureMermaid(mermaid: any) {
  mermaid.initialize({
    startOnLoad: false,
    theme: mermaidTheme(),
    fontFamily: 'system-ui, -apple-system, sans-serif',
    flowchart: { useMaxWidth: true, htmlLabels: true },
    sequence: { useMaxWidth: true },
    gantt: { useMaxWidth: true },
  })
}

async function renderMermaidBlocks() {
  const container = markdownBodyRef.value
  if (!container) return
  const blocks = container.querySelectorAll<HTMLElement>('.mermaid-block[data-mermaid]')
  if (blocks.length === 0) return

  let mermaid: any
  try {
    mermaid = await loadMermaid()
    configureMermaid(mermaid)
  } catch (loadErr) {
    console.error('Failed to load mermaid:', loadErr)
    // Fallback: show an inline load error inside every mermaid block so
    // the page doesn't just sit on "Loading diagram..." forever.
    for (const block of blocks) {
      block.innerHTML = renderMermaidErrorHtml(labels.value.mermaidUnavailable)
      block.classList.add('mermaid-error-block')
      block.removeAttribute('data-mermaid')
    }
    return
  }

  for (const block of blocks) {
    const decoded = decodeMermaidSource(block.getAttribute('data-mermaid'))
    if (!decoded) continue

    // Pre-parse so mermaid can report syntax errors *without* injecting
    // the giant red error <svg> into <body> that it normally does when
    // render() throws. `suppressErrors: true` makes parse return false
    // on failure instead of throwing, so we can fall through to our own
    // inline error box and never call render() on bad source.
    let parseError: unknown = null
    try {
      const ok = await mermaid.parse(decoded, { suppressErrors: true })
      if (ok === false) parseError = new Error('Mermaid syntax error')
    } catch (e) {
      parseError = e
    }
    if (parseError) {
      block.innerHTML = renderMermaidErrorHtml(
        labels.value.mermaidSyntaxError,
      )
      block.classList.add('mermaid-error-block')
      block.removeAttribute('data-mermaid')
      cleanupMermaidStrayNodes(undefined, container)
      continue
    }

    const id = `mermaid-svg-${++mermaidIdCounter}`
    try {
      const { svg } = await mermaid.render(id, decoded)
      block.innerHTML = svg
      block.classList.add('mermaid-rendered')
      block.removeAttribute('data-mermaid')
      block.style.cursor = 'pointer'
      block.title = labels.value.mermaidClickToEnlarge

      block.addEventListener('click', () => {
        openMermaidLightbox(svg)
      })
    } catch (renderErr: any) {
      console.error('Mermaid render error:', renderErr)
      block.innerHTML = renderMermaidErrorHtml(
        labels.value.mermaidSyntaxError,
      )
      block.classList.add('mermaid-error-block')
      block.removeAttribute('data-mermaid')
    } finally {
      // Defensive: mermaid >=10 sometimes leaves a measurement DOM node
      // attached to <body> with id `d${id}` or `${id}`. Remove them so
      // the page is never visually polluted by a stray diagram.
      cleanupMermaidStrayNodes(id, container)
    }
  }
}

function decodeMermaidSource(source: string | null): string {
  if (!source) return ''
  return source
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
}

function renderMermaidErrorHtml(title: string): string {
  const safeTitle = md.utils.escapeHtml(title)
  return `<div class="mermaid-error">
    <span class="mermaid-error-icon" aria-hidden="true">⚠️</span>
    <span class="mermaid-error-title">${safeTitle}</span>
  </div>`
}

function cleanupMermaidStrayNodes(id?: string, owner?: HTMLElement) {
  // Mermaid occasionally attaches measurement artefacts directly to
  // <body> under ids derived from the one we passed in. On syntax
  // errors it can also leave a wrapper <div> containing the visible red
  // error SVG. Remove the wrapper too so the failure stays inside the
  // markdown renderer's own mermaid block.
  const selectors = id
    ? [`#${id}`, `#d${id}`, `[id^="${id}-"]`]
    : ['svg[id^="mermaid-svg-"]']
  for (const sel of selectors) {
    document.querySelectorAll(sel).forEach((node) => {
      if (owner?.contains(node)) return
      if ((node as Element).closest('.markdown-body, .mermaid-lightbox-overlay, #app')) return

      let removable: Element = node
      while (removable.parentElement && removable.parentElement !== document.body) {
        removable = removable.parentElement
      }

      if (removable.parentElement === document.body) {
        removable.remove()
      }
    })
  }
}

// Convert SVG string to PNG data URL (4K Ultra HD quality)
async function svgToPng(svgString: string, backgroundColor: string = '#ffffff'): Promise<string> {
  return new Promise((resolve, reject) => {
    // Parse SVG to get intrinsic dimensions from viewBox or width/height attributes
    const parser = new DOMParser()
    const svgDoc = parser.parseFromString(svgString, 'image/svg+xml')
    const svgEl = svgDoc.querySelector('svg')
    
    if (!svgEl) {
      reject(new Error('Invalid SVG'))
      return
    }
    
    // Determine the base dimensions from viewBox or explicit width/height
    let baseWidth = 800
    let baseHeight = 600
    
    const viewBox = svgEl.getAttribute('viewBox')
    if (viewBox) {
      const parts = viewBox.split(/[\s,]+/).map(Number)
      if (parts.length === 4 && parts[2] > 0 && parts[3] > 0) {
        baseWidth = parts[2]
        baseHeight = parts[3]
      }
    }
    
    // Check explicit width/height (may override viewBox)
    const widthAttr = svgEl.getAttribute('width')
    const heightAttr = svgEl.getAttribute('height')
    if (widthAttr && heightAttr) {
      const w = parseFloat(widthAttr)
      const h = parseFloat(heightAttr)
      if (w > 0 && h > 0) {
        baseWidth = w
        baseHeight = h
      }
    }
    
    // Force set explicit width/height on SVG for correct rendering
    const scale = 4  // 4x for 4K Ultra HD quality
    const renderWidth = baseWidth * scale
    const renderHeight = baseHeight * scale
    
    // Clone SVG and set explicit dimensions for high-res rendering
    svgEl.setAttribute('width', String(renderWidth))
    svgEl.setAttribute('height', String(renderHeight))
    
    const serializer = new XMLSerializer()
    const modifiedSvg = serializer.serializeToString(svgEl)
    
    const svgBlob = new Blob([modifiedSvg], { type: 'image/svg+xml;charset=utf-8' })
    const url = URL.createObjectURL(svgBlob)
    const img = new Image()
    img.onload = () => {
      const canvas = document.createElement('canvas')
      canvas.width = renderWidth
      canvas.height = renderHeight
      const ctx = canvas.getContext('2d')
      if (!ctx) {
        reject(new Error('Failed to get canvas context'))
        return
      }
      // Enable high-quality rendering
      ctx.imageSmoothingEnabled = true
      ctx.imageSmoothingQuality = 'high'
      // Fill white background
      ctx.fillStyle = backgroundColor
      ctx.fillRect(0, 0, canvas.width, canvas.height)
      // Draw SVG at full resolution
      ctx.drawImage(img, 0, 0, renderWidth, renderHeight)
      URL.revokeObjectURL(url)
      resolve(canvas.toDataURL('image/png', 1.0))
    }
    img.onerror = (err) => {
      URL.revokeObjectURL(url)
      reject(err)
    }
    img.src = url
  })
}

// Mermaid lightbox functionality
async function openMermaidLightbox(svg: string) {
  lightboxSvg.value = svg
  lightboxZoom.value = 1.0
  imagePosition.value = { x: 0, y: 0 }
  lightboxSvgStyle.value = { transform: 'scale(1) translate(0px, 0px)', transformOrigin: 'center center' }
  showLightbox.value = true
  document.body.style.overflow = 'hidden' // Prevent background scrolling
  
  // Convert SVG to PNG
  try {
    const pngUrl = await svgToPng(svg, resolvedTheme.value === 'dark' ? '#111827' : '#ffffff')
    lightboxPngUrl.value = pngUrl
  } catch (err) {
    console.error('Failed to convert SVG to PNG:', err)
    lightboxPngUrl.value = ''
  }
  
  // Bind wheel event after DOM update
  nextTick(() => {
    const container = document.querySelector('.mermaid-lightbox-svg-container') as HTMLElement
    if (container) {
      lightboxSvgEl = container
      container.addEventListener('wheel', handleLightboxWheel, { passive: false })
    }
  })
}

function closeMermaidLightbox() {
  showLightbox.value = false
  document.body.style.overflow = ''
  lightboxPngUrl.value = ''
  if (lightboxSvgEl) {
    lightboxSvgEl.removeEventListener('wheel', handleLightboxWheel)
    lightboxSvgEl = null
  }
}

// Mouse drag handlers
function handleMouseDown(e: MouseEvent) {
  if (e.button !== 0) return // Only left mouse button
  isDragging.value = true
  dragStartX.value = e.clientX - imagePosition.value.x
  dragStartY.value = e.clientY - imagePosition.value.y
}

function handleMouseMove(e: MouseEvent) {
  if (!isDragging.value) return
  e.preventDefault()
  const newX = e.clientX - dragStartX.value
  const newY = e.clientY - dragStartY.value
  imagePosition.value = { x: newX, y: newY }
  lightboxSvgStyle.value = {
    transform: `scale(${lightboxZoom.value}) translate(${newX}px, ${newY}px)`,
    transformOrigin: 'center center'
  }
}

function handleMouseUp() {
  isDragging.value = false
}

function handleLightboxWheel(e: WheelEvent) {
  e.preventDefault()
  const delta = e.deltaY > 0 ? -0.1 : 0.1
  lightboxZoom.value = Math.min(5.0, Math.max(0.1, lightboxZoom.value + delta))
  lightboxSvgStyle.value = {
    transform: `scale(${lightboxZoom.value}) translate(${imagePosition.value.x}px, ${imagePosition.value.y}px)`,
    transformOrigin: 'center center'
  }
}

function resetLightboxZoom() {
  lightboxZoom.value = 1.0
  imagePosition.value = { x: 0, y: 0 }
  lightboxSvgStyle.value = { 
    transform: 'scale(1) translate(0px, 0px)', 
    transformOrigin: 'center center' 
  }
}

function downloadPng() {
  const pngUrl = lightboxPngUrl.value
  if (!pngUrl) return
  
  const a = document.createElement('a')
  a.href = pngUrl
  a.download = `mermaid-diagram-${Date.now()}.png`
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
}

function downloadSvg() {
  const svgContent = lightboxSvg.value
  if (!svgContent) return
  
  // Create a Blob from SVG string
  const blob = new Blob([svgContent], { type: 'image/svg+xml' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `mermaid-diagram-${Date.now()}.svg`
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}

let themeObserver: MutationObserver | null = null
let mediaQuery: MediaQueryList | null = null

function bumpRenderEpoch() {
  renderEpoch.value += 1
}

function handleThemeChange() {
  const previous = resolvedTheme.value
  syncResolvedTheme()
  if (resolvedTheme.value !== previous) bumpRenderEpoch()
}

watch(
  () => props.theme,
  handleThemeChange,
  { immediate: true },
)

watch(
  () => props.locale,
  bumpRenderEpoch,
)

// Clean up event listeners on unmount
onBeforeUnmount(() => {
  if (lightboxSvgEl) {
    lightboxSvgEl.removeEventListener('wheel', handleLightboxWheel)
  }
  themeObserver?.disconnect()
  if (mediaQuery) {
    mediaQuery.removeEventListener?.('change', handleThemeChange)
  }
  document.body.style.overflow = ''
})

// Bind copy buttons for code blocks
function bindCopyButtons() {
  const container = markdownBodyRef.value
  if (!container) return
  const buttons = container.querySelectorAll<HTMLButtonElement>('.code-copy')
  buttons.forEach((btn) => {
    // Avoid re-binding
    if (btn.getAttribute('data-bound')) return
    btn.setAttribute('data-bound', 'true')
    btn.addEventListener('click', async () => {
      const pre = btn.closest('pre')
      if (!pre) return
      const code = pre.querySelector('code')
      if (!code) return
      try {
        await navigator.clipboard.writeText(code.textContent || '')
        const originalTitle = btn.title
        const originalLabel = btn.getAttribute('aria-label') || labels.value.copyCode
        btn.title = labels.value.copied
        btn.setAttribute('aria-label', labels.value.copied)
        btn.classList.add('copied')
        setTimeout(() => {
          btn.title = originalTitle
          btn.setAttribute('aria-label', originalLabel)
          btn.classList.remove('copied')
        }, 2000)
      } catch (err) {
        console.error('Copy failed:', err)
      }
    })
  })
}

onMounted(() => {
  syncResolvedTheme()
  if (typeof document !== 'undefined') {
    themeObserver = new MutationObserver(handleThemeChange)
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme'],
    })
  }
  if (typeof window !== 'undefined') {
    mediaQuery = window.matchMedia?.('(prefers-color-scheme: dark)') ?? null
    mediaQuery?.addEventListener?.('change', handleThemeChange)
  }
  nextTick(() => {
    renderMermaidBlocks()
    bindCopyButtons()
  })
})

watch(
  [renderedContent, resolvedTheme, labels],
  () => {
    nextTick(() => {
      renderMermaidBlocks()
      bindCopyButtons()
    })
  },
)
</script>

<template>
  <div
    :key="renderKey"
    ref="markdownBodyRef"
    class="markdown-body"
    :data-theme="resolvedTheme"
    v-html="renderedContent"
  ></div>
  
  <!-- Mermaid Lightbox Modal -->
  <Teleport to="body">
    <div v-if="showLightbox" class="mermaid-lightbox-overlay" @click.self="closeMermaidLightbox">
      <div class="mermaid-lightbox-toolbar">
        <span class="mermaid-lightbox-zoom">{{ Math.round(lightboxZoom * 100) }}%</span>
        <button class="mermaid-lightbox-btn" @click="resetLightboxZoom" :title="labels.mermaidResetTitle">{{ labels.mermaidReset }}</button>
        <button class="mermaid-lightbox-btn" @click="downloadPng" :title="labels.downloadPng">PNG</button>
        <button class="mermaid-lightbox-btn" @click="downloadSvg" :title="labels.downloadSvg">SVG</button>
        <button class="mermaid-lightbox-close" @click="closeMermaidLightbox" :title="labels.close">✕</button>
      </div>
      <div 
        class="mermaid-lightbox-content"
        @mousedown="handleMouseDown"
        @mousemove="handleMouseMove"
        @mouseup="handleMouseUp"
        @mouseleave="handleMouseUp"
      >
        <div 
          class="mermaid-lightbox-svg-container" 
          :style="lightboxSvgStyle"
          v-if="lightboxPngUrl"
        >
          <img 
            :src="lightboxPngUrl" 
            :alt="labels.mermaidImageAlt" 
            style="display: block;"
            @dragstart.prevent
          />
        </div>
        <!-- Fallback to SVG if PNG conversion fails -->
        <div 
          class="mermaid-lightbox-svg-container" 
          :style="lightboxSvgStyle" 
          v-html="lightboxSvg"
          v-else
        ></div>
      </div>
      <div class="mermaid-lightbox-hint">{{ labels.mermaidZoomHint }}</div>
    </div>
  </Teleport>
</template>

<style>
/* Markdown body styles */
.markdown-body {
  color: var(--bb-md-text, var(--text-primary));
  line-height: 1.8;
  font-size: 15px;
  max-width: 860px;
  word-wrap: break-word;
}

.markdown-body h1 {
  font-size: 28px;
  font-weight: 700;
  margin: 40px 0 20px;
  padding-bottom: 10px;
  border-bottom: 2px solid var(--bb-md-border, var(--border-color));
  letter-spacing: 0;
  position: relative;
}

.markdown-body h2 {
  font-size: 22px;
  font-weight: 600;
  margin: 32px 0 16px;
  padding-bottom: 8px;
  border-bottom: 1px dashed var(--bb-md-border, var(--border-color));
  position: relative;
}

.markdown-body h3 {
  font-size: 18px;
  font-weight: 600;
  margin: 24px 0 12px;
  position: relative;
}

.markdown-body h4 {
  font-size: 16px;
  font-weight: 600;
  margin: 20px 0 10px;
  position: relative;
}

.markdown-body h5 {
  font-size: 15px;
  font-weight: 600;
  margin: 16px 0 8px;
  position: relative;
}

.markdown-body h6 {
  font-size: 14px;
  font-weight: 600;
  margin: 14px 0 6px;
  color: var(--bb-md-text-muted, var(--text-secondary));
  position: relative;
}

.markdown-body p {
  margin-bottom: 16px;
  line-height: 1.8;
}

.markdown-body ul,
.markdown-body ol {
  margin-bottom: 16px;
  padding-left: 28px;
}

.markdown-body ul {
  list-style: disc;
}

.markdown-body ul ul {
  list-style: circle;
}

.markdown-body ul ul ul {
  list-style: square;
}

.markdown-body ol {
  list-style: decimal;
}

.markdown-body li {
  margin-bottom: 6px;
  line-height: 1.7;
}

.markdown-body li > p {
  margin-bottom: 8px;
}

.markdown-body a {
  color: var(--bb-md-link);
  text-decoration: none;
  border-bottom: 1px solid transparent;
  transition: border-color 0.2s ease, color 0.2s ease;
}

.markdown-body a:hover {
  color: var(--bb-md-link-hover);
  border-bottom-color: var(--bb-md-link-hover);
}

.markdown-body blockquote {
  border-left: 4px solid var(--bb-md-link);
  padding: 12px 20px;
  margin: 20px 0;
  background-color: var(--bb-md-blockquote-bg);
  color: var(--bb-md-text-muted);
  font-style: italic;
  border-radius: 0 6px 6px 0;
}

.markdown-body blockquote blockquote {
  border-left-color: var(--bb-md-border-strong);
  background-color: var(--bb-md-surface-muted);
  margin: 12px 0;
}

.markdown-body blockquote blockquote blockquote {
  border-left-color: var(--bb-md-border);
  background-color: var(--bb-md-surface-soft);
}

.markdown-body blockquote p:last-child {
  margin-bottom: 0;
}

.markdown-body code:not(.hljs code) {
  background-color: var(--bb-md-inline-code-bg);
  color: var(--bb-md-inline-code-text);
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 0.9em;
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  border: 1px solid var(--bb-md-inline-code-border);
}

/* Image styles */
.markdown-body img {
  max-width: 100%;
  height: auto;
  display: block;
  margin: 20px auto;
  border-radius: 8px;
  box-shadow: var(--bb-md-shadow-soft);
}

/* Horizontal rule styles */
.markdown-body hr {
  border: none;
  height: 1px;
  background: linear-gradient(to right, transparent, var(--bb-md-border), transparent);
  margin: 32px 0;
}

.markdown-body pre {
  margin: 16px 0;
  border-radius: 8px;
  overflow: hidden;
  position: relative;
}

/* Code block toolbar */
.markdown-body .code-toolbar {
  position: absolute;
  top: 8px;
  left: 12px;
  right: 12px;
  z-index: 1;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  padding: 0;
  background: transparent;
  min-height: 16px;
  pointer-events: none;
}

.markdown-body .code-lang {
  font-size: 11px;
  line-height: 1;
  color: var(--bb-md-code-toolbar);
  text-transform: uppercase;
  font-weight: 600;
  letter-spacing: 0;
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
}

.markdown-body .code-copy {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  color: var(--bb-md-code-toolbar);
  background: transparent;
  border: 0;
  border-radius: 0;
  padding: 0;
  cursor: pointer;
  transition: color 0.15s ease;
  font-family: system-ui, -apple-system, sans-serif;
  pointer-events: auto;
}

.markdown-body .code-copy svg {
  width: 14px;
  height: 14px;
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.markdown-body .code-copy:hover {
  color: var(--bb-md-code-text);
}

.markdown-body .code-copy.copied {
  color: var(--bb-success);
}

/* Highlight.js theme */
.hljs {
  background-color: var(--bb-md-code-bg);
  color: var(--bb-md-code-text);
  padding: 32px 16px 16px;
  overflow-x: auto;
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 13px;
  line-height: 1.6;
}

.hljs-comment,
.hljs-quote {
  color: var(--bb-hljs-comment);
}

.hljs-keyword,
.hljs-selector-tag,
.hljs-addition {
  color: var(--bb-hljs-keyword);
}

.hljs-number,
.hljs-string,
.hljs-meta .hljs-meta-string,
.hljs-literal,
.hljs-doctag,
.hljs-regexp {
  color: var(--bb-hljs-string);
}

.hljs-title,
.hljs-section,
.hljs-name,
.hljs-selector-id,
.hljs-selector-class {
  color: var(--bb-hljs-title);
}

.hljs-attribute,
.hljs-attr,
.hljs-variable,
.hljs-template-variable,
.hljs-class .hljs-title,
.hljs-type {
  color: var(--bb-hljs-attr);
}

.hljs-symbol,
.hljs-bullet,
.hljs-subst,
.hljs-meta,
.hljs-meta .hljs-keyword,
.hljs-selector-attr,
.hljs-selector-pseudo,
.hljs-link {
  color: var(--bb-hljs-symbol);
}

.hljs-built_in,
.hljs-deletion {
  color: var(--bb-hljs-deletion);
}

.hljs-formula {
  color: var(--bb-hljs-formula);
}

.hljs-emphasis {
  font-style: italic;
}

.hljs-strong {
  font-weight: bold;
}

/* Table styles */
.markdown-body table {
  width: 100%;
  border-collapse: collapse;
  margin: 20px 0;
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid var(--bb-md-border, var(--border-color));
}

.markdown-body th,
.markdown-body td {
  border: 1px solid var(--bb-md-border, var(--border-color));
  padding: 10px 16px;
  text-align: left;
  font-size: 14px;
}

.markdown-body th {
  background-color: var(--bb-md-table-head-bg);
  font-weight: 600;
  color: var(--bb-md-text-strong);
}

.markdown-body tbody tr:nth-child(even) {
  background-color: var(--bb-md-table-row-alt);
}

.markdown-body tbody tr:hover {
  background-color: var(--bb-md-table-row-hover);
  transition: background-color 0.15s ease;
}

/* Anchor link styles */
.markdown-body .header-anchor {
  color: var(--bb-md-text-muted);
  text-decoration: none;
  opacity: 0;
  visibility: hidden;
  transition: opacity 0.2s, visibility 0.2s;
  position: absolute;
  left: -20px;
  padding-right: 4px;
}

.markdown-body .header-anchor:hover {
  color: var(--bb-md-link, var(--color-primary));
}

.markdown-body h1:hover .header-anchor,
.markdown-body h2:hover .header-anchor,
.markdown-body h3:hover .header-anchor,
.markdown-body h4:hover .header-anchor,
.markdown-body h5:hover .header-anchor,
.markdown-body h6:hover .header-anchor {
  opacity: 1;
  visibility: visible;
}

/* Task list (checkbox) styles */
.markdown-body .task-list-item {
  list-style: none;
  position: relative;
  padding-left: 6px;
  margin-bottom: 8px;
}

.markdown-body .task-list-item-checkbox {
  appearance: none;
  -webkit-appearance: none;
  width: 18px;
  height: 18px;
  border: 2px solid var(--bb-md-border);
  border-radius: 4px;
  background-color: transparent;
  cursor: default;
  position: relative;
  top: 3px;
  margin-right: 8px;
  transition: all 0.2s ease;
  pointer-events: none;
}

.markdown-body .task-list-item-checkbox:checked {
  background-color: var(--bb-md-link);
  border-color: var(--bb-md-link);
}

.markdown-body .task-list-item-checkbox:checked::after {
  content: '';
  position: absolute;
  left: 4px;
  top: 1px;
  width: 6px;
  height: 10px;
  border: solid white;
  border-width: 0 2px 2px 0;
  transform: rotate(45deg);
}

.markdown-body .task-list-item-checkbox:checked + .task-list-item-label {
  text-decoration: line-through;
  color: var(--bb-md-text-muted);
}

.markdown-body ul.contains-task-list {
  padding-left: 8px;
}

.markdown-body ul.contains-task-list ul.contains-task-list {
  padding-left: 24px;
}

/* Mermaid diagram styles */
.markdown-body .mermaid-block {
  margin: 20px 0;
  padding: 20px;
  background-color: var(--bb-md-surface-soft);
  border: 1px solid var(--bb-md-border);
  border-radius: 8px;
  overflow: auto;
  cursor: pointer;
  transition: box-shadow 0.2s ease, border-color 0.2s ease;
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 100px;
}

.markdown-body .mermaid-block:hover {
  box-shadow: 0 4px 16px var(--bb-md-shadow-soft);
  border-color: var(--bb-md-link);
}

.markdown-body .mermaid-block.mermaid-rendered {
  background-color: transparent;
  border-color: var(--bb-md-border);
  display: block;
  text-align: center;
}

.markdown-body .mermaid-block.mermaid-rendered:hover {
  border-color: var(--bb-md-link);
}

.markdown-body .mermaid-block:hover {
  box-shadow: 0 4px 16px var(--bb-md-shadow-soft);
  border-color: var(--bb-md-link);
}

.markdown-body .mermaid-block.mermaid-rendered {
  background-color: transparent;
  border-color: var(--bb-md-border);
  display: block;
  text-align: center;
}

.markdown-body .mermaid-block.mermaid-rendered:hover {
  border-color: var(--bb-md-link);
}

.markdown-body .mermaid-block svg {
  height: auto;
  max-width: none;
}

/* Mermaid Lightbox Overlay */
.mermaid-lightbox-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background-color: var(--mermaid-lightbox-overlay-bg, rgba(0, 0, 0, 0.85));
  z-index: 9999;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  animation: lightboxFadeIn 0.2s ease;
}

@keyframes lightboxFadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

/* Lightbox Toolbar */
.mermaid-lightbox-toolbar {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
  padding: 12px 20px;
  background: var(--mermaid-lightbox-toolbar-gradient, linear-gradient(to bottom, rgba(0,0,0,0.5), transparent));
  z-index: 10;
}

.mermaid-lightbox-zoom {
  color: var(--mermaid-lightbox-text, white);
  font-size: 14px;
  font-weight: 500;
  min-width: 50px;
  text-align: center;
}

.mermaid-lightbox-btn {
  padding: 6px 16px;
  border: 1px solid var(--mermaid-lightbox-btn-border, rgba(255, 255, 255, 0.3));
  border-radius: 6px;
  background-color: var(--mermaid-lightbox-btn-bg, rgba(255, 255, 255, 0.1));
  color: var(--mermaid-lightbox-text, white);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.15s ease;
  backdrop-filter: blur(10px);
}

.mermaid-lightbox-btn:hover {
  background-color: var(--mermaid-lightbox-btn-hover-bg, rgba(255, 255, 255, 0.2));
  border-color: var(--mermaid-lightbox-btn-hover-border, rgba(255, 255, 255, 0.5));
}

.mermaid-lightbox-close {
  width: 36px;
  height: 36px;
  border: 1px solid var(--mermaid-lightbox-close-border, rgba(255, 255, 255, 0.3));
  border-radius: 50%;
  background-color: var(--mermaid-lightbox-close-bg, rgba(255, 255, 255, 0.1));
  color: var(--mermaid-lightbox-text, white);
  font-size: 18px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s ease;
  backdrop-filter: blur(10px);
}

.mermaid-lightbox-close:hover {
  background-color: var(--mermaid-lightbox-close-hover-bg, rgba(255, 255, 255, 0.25));
  border-color: var(--mermaid-lightbox-close-hover-border, rgba(255, 255, 255, 0.5));
}

/* Lightbox Content */
.mermaid-lightbox-content {
  flex:1;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  overflow: hidden;
  padding: 60px 20px 40px;
}

.mermaid-lightbox-svg-container {
  transition: transform 0.1s ease-out;
  display: flex;
  justify-content: center;
  align-items: center;
  transform-origin: center center;
  cursor: grab;
}

.mermaid-lightbox-svg-container:active {
  cursor: grabbing;
}

.mermaid-lightbox-svg-container svg {
  max-width: 90vw;
  max-height: 85vh;
  width: auto;
  height: auto;
}

.mermaid-lightbox-svg-container img {
  max-width: 90vw;
  max-height: 85vh;
  width: auto;
  height: auto;
  object-fit: contain;
}

/* Lightbox Hint */
.mermaid-lightbox-hint {
  position: absolute;
  bottom: 20px;
  left: 50%;
  transform: translateX(-50%);
  color: var(--mermaid-lightbox-hint-text, rgba(255, 255, 255, 0.5));
  font-size: 13px;
  padding: 6px 16px;
  background-color: var(--mermaid-lightbox-hint-bg, rgba(0, 0, 0, 0.3));
  border-radius: 20px;
  pointer-events: none;
  animation: hintFade 3s ease 2s forwards;
}

@keyframes hintFade {
  to { opacity: 0; }
}

.markdown-body .mermaid-loading {
  color: var(--bb-md-text-muted);
  font-size: 13px;
  padding: 24px 0;
  animation: mermaidPulse 1.5s ease-in-out infinite;
}

@keyframes mermaidPulse {
  0%, 100% { opacity: 0.5; }
  50% { opacity: 1; }
}

.markdown-body .mermaid-error-block {
  border-color: var(--bb-md-error-border) !important;
  background-color: var(--bb-md-error-bg) !important;
  text-align: left;
  justify-content: flex-start;
  align-items: center;
  min-height: 0;
  padding: 12px 16px;
}

.markdown-body .mermaid-error {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 8px;
}

.markdown-body .mermaid-error-icon {
  flex-shrink: 0;
  font-size: 15px;
  line-height: 1;
}

.markdown-body .mermaid-error-title {
  color: var(--bb-md-error-text);
  font-weight: 600;
  font-size: 14px;
  line-height: 1.4;
  margin: 0;
}

.markdown-body .mermaid-error-message {
  color: var(--bb-md-error-text);
  font-size: 12px;
  line-height: 1.5;
  margin-bottom: 12px;
  white-space: pre-wrap;
  word-break: break-word;
}

.markdown-body .mermaid-error-source {
  margin: 0;
  background-color: var(--bb-md-code-bg);
  border-radius: 6px;
  overflow-x: auto;
}

.markdown-body .mermaid-error-source code {
  display: block;
  padding: 12px 16px;
  color: var(--bb-md-code-text);
  font-size: 12px;
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  line-height: 1.5;
  white-space: pre;
}

</style>
