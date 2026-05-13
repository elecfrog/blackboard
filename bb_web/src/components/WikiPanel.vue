<script setup lang="ts">
/**
 * Three-pane wiki workspace:
 *   - left    : file tree (`WikiTreeItem`), resizable / collapsible.
 *   - center  : MarkdownRenderer for the selected document.
 *   - right   : Table of contents for the selected document.
 *
 * Contract with the router / BoardView:
 *   - when the URL carries a `path` (e.g. `/projects/foo/wiki/modules/x.md`),
 *     we preselect that document on mount and on path changes.
 *   - without a path, we fall back to the tree's `index.md`, then to the
 *     first file node; empty wikis show a `wikiEmpty` prompt.
 *
 * Relative links and images inside rendered Markdown are rewritten so they
 * stay inside the app (for `.md` links → `router.replace`) or hit the
 * server's `/wiki/asset` endpoint (for images / svg / pdf).
 */

import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { Columns2, PanelLeft, PanelLeftClose, Rows2, Upload } from 'lucide-vue-next'
import { MarkdownRenderer, TableOfContents } from '@/ui/markdown'
import {
  buildCodeMarkdown,
  fileExtension,
  pickWikiRenderKind,
  type WikiRenderKind,
} from '@/ui/wiki/render'
import type { WikiTreeNode } from '@/data/wiki'
import { loadWikiContent, loadWikiTree, uploadWikiFiles, wikiAssetUrl } from '@/data/wiki'
import WikiTreeItem from '@/components/WikiTreeItem.vue'
import { locale, t } from '@/i18n'

const props = defineProps<{
  project: string
  path?: string
}>()

const router = useRouter()

const wikiTree = ref<WikiTreeNode[]>([])
const loading = ref(true)
const error = ref('')
const selectedPath = ref<string | null>(null)
const wikiContent = ref('')
/** Lowercased extension hint returned by the server for the currently
 *  loaded document. Drives which renderer the content pane picks. */
const wikiContentType = ref<string>('')
const contentLoading = ref(false)
const contentError = ref('')

const uploading = ref(false)
const uploadError = ref('')
const uploadSuccess = ref('')
const fileInputRef = ref<HTMLInputElement | null>(null)
const documentRef = ref<HTMLElement | null>(null)
const svgLayout = ref<'side' | 'stack'>('side')

const STORAGE_KEY_TREE_WIDTH = 'blackboard.wiki.treeWidth'
const STORAGE_KEY_TREE_COLLAPSED = 'blackboard.wiki.treeCollapsed'

const treeWidth = ref(260)
const treeCollapsed = ref(false)
const isResizing = ref(false)
let resizeStartX = 0
let resizeStartWidth = 0

// Paths of directories to auto-expand on mount — used so the ancestors of
// the pre-selected document are unfolded when the panel first appears.
const expandedAncestorPaths = ref<string[]>([])

function loadSettings() {
  try {
    const storedWidth = localStorage.getItem(`${STORAGE_KEY_TREE_WIDTH}.${props.project}`)
    if (storedWidth) {
      const w = parseInt(storedWidth, 10)
      if (w >= 140 && w <= 600) treeWidth.value = w
    }
    const storedCollapsed = localStorage.getItem(
      `${STORAGE_KEY_TREE_COLLAPSED}.${props.project}`,
    )
    if (storedCollapsed === 'true') treeCollapsed.value = true
  } catch {
    /* localStorage can throw in privacy modes; ignore */
  }
}

function saveTreeWidth(width: number) {
  try {
    localStorage.setItem(`${STORAGE_KEY_TREE_WIDTH}.${props.project}`, String(width))
  } catch {
    /* ignore */
  }
}

function saveTreeCollapsed(collapsed: boolean) {
  try {
    localStorage.setItem(`${STORAGE_KEY_TREE_COLLAPSED}.${props.project}`, String(collapsed))
  } catch {
    /* ignore */
  }
}

function toggleTreeCollapse() {
  treeCollapsed.value = !treeCollapsed.value
  saveTreeCollapsed(treeCollapsed.value)
}

function startResize(event: MouseEvent) {
  isResizing.value = true
  resizeStartX = event.clientX
  resizeStartWidth = treeWidth.value
  document.addEventListener('mousemove', onResize)
  document.addEventListener('mouseup', stopResize)
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
}

function onResize(event: MouseEvent) {
  if (!isResizing.value) return
  const delta = event.clientX - resizeStartX
  treeWidth.value = Math.max(140, Math.min(600, resizeStartWidth + delta))
}

function stopResize() {
  if (!isResizing.value) return
  isResizing.value = false
  document.removeEventListener('mousemove', onResize)
  document.removeEventListener('mouseup', stopResize)
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  saveTreeWidth(treeWidth.value)
}

onMounted(() => {
  loadSettings()
})

onBeforeUnmount(() => {
  document.removeEventListener('mousemove', onResize)
  document.removeEventListener('mouseup', stopResize)
})

function findNode(nodes: WikiTreeNode[], path: string): WikiTreeNode | null {
  for (const node of nodes) {
    if (node.path === path) return node
    if (node.children) {
      const found = findNode(node.children, path)
      if (found) return found
    }
  }
  return null
}

/** Compute ancestor directory paths for a given file path so the tree can
 *  auto-expand to reveal it. */
function ancestorDirs(filePath: string): string[] {
  const parts = filePath.split('/')
  const dirs: string[] = []
  for (let i = 1; i < parts.length; i++) {
    dirs.push(parts.slice(0, i).join('/'))
  }
  return dirs
}

/** Pick the best default document to display when the URL does not carry
 *  one: prefer top-level `index.md`, else the first file anywhere in the
 *  tree. Returns null when the tree is empty. */
function pickDefaultFile(nodes: WikiTreeNode[]): string | null {
  const topIndex = nodes.find((n) => n.kind === 'file' && n.name === 'index.md')
  if (topIndex) return topIndex.path
  // Depth-first search for the first file node.
  for (const node of nodes) {
    if (node.kind === 'file') return node.path
    if (node.children) {
      const found = pickDefaultFile(node.children)
      if (found) return found
    }
  }
  return null
}

async function reload(project: string, preferPath?: string) {
  loading.value = true
  error.value = ''
  wikiTree.value = []
  selectedPath.value = null
  wikiContent.value = ''
  wikiContentType.value = ''
  try {
    const result = await loadWikiTree(project)
    wikiTree.value = result.tree

    // Decide which document to open first.
    let target: string | null = null
    if (preferPath && findNode(result.tree, preferPath)) {
      target = preferPath
    } else {
      target = pickDefaultFile(result.tree)
    }
    if (target) {
      expandedAncestorPaths.value = ancestorDirs(target)
      await selectPath(target)
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

watch(
  () => props.project,
  (project) => {
    if (project) reload(project, props.path)
  },
  { immediate: true },
)

watch(
  () => props.path,
  (path) => {
    if (!path) return
    // Tree is still loading — reload will honour `preferPath`.
    if (loading.value) return
    if (path === selectedPath.value) return
    if (findNode(wikiTree.value, path)) {
      expandedAncestorPaths.value = ancestorDirs(path)
      selectPath(path)
    }
  },
)

async function selectPath(path: string) {
  if (selectedPath.value === path) return
  selectedPath.value = path
  contentLoading.value = true
  contentError.value = ''
  wikiContent.value = ''
  wikiContentType.value = ''

  const ext = fileExtension(path)
  const kind = pickWikiRenderKind(ext)

  // Binary formats are served by <img>/<iframe> pointing at the asset
  // endpoint. We never hit /wiki/file for them — the document endpoint
  // (intentionally) 400s on binary extensions.
  if (kind === 'image' || kind === 'pdf') {
    wikiContentType.value = ext
    contentLoading.value = false
    return
  }

  try {
    const { content, contentType } = await loadWikiContent(props.project, path)
    wikiContent.value = content
    wikiContentType.value = contentType || ext
  } catch (err) {
    contentError.value = err instanceof Error ? err.message : String(err)
  } finally {
    contentLoading.value = false
  }
}

function onTreeSelect(node: WikiTreeNode) {
  if (node.kind !== 'file') return
  selectPath(node.path)
  router.replace(`/projects/${props.project}/wiki/${node.path}`)
}

/** Resolve a relative wiki link (from inside rendered Markdown) into an
 *  absolute wiki-relative POSIX path, or null when the input is external
 *  (http://, mailto:, #anchor, ...). Pure function — no router access. */
function resolveRelativeWikiPath(base: string, href: string): string | null {
  if (!href) return null
  if (/^[a-z]+:\/\//i.test(href)) return null // absolute URL
  if (href.startsWith('mailto:')) return null
  if (href.startsWith('#')) return null // in-page anchor
  if (href.startsWith('/')) return null // site-absolute; let the browser handle it

  // Split into path and anchor; anchors are preserved for router.replace.
  const [rawPath] = href.split('#')

  const baseDir = base.includes('/') ? base.slice(0, base.lastIndexOf('/')) : ''
  const segments = (baseDir ? baseDir.split('/') : []).concat(rawPath.split('/'))

  const resolved: string[] = []
  for (const segment of segments) {
    if (segment === '' || segment === '.') continue
    if (segment === '..') {
      if (resolved.length === 0) return null // escapes wiki root
      resolved.pop()
      continue
    }
    resolved.push(segment)
  }
  return resolved.join('/')
}

function handleContentLinkClick(event: MouseEvent) {
  const target = event.target as HTMLElement
  const link = target.closest('a') as HTMLAnchorElement | null
  if (!link) return
  const href = link.getAttribute('href')
  if (!href) return

  // Only intercept relative links; everything else falls through to the
  // browser's default handling (external links open in a new tab, etc.).
  if (/^[a-z]+:\/\//i.test(href) || href.startsWith('/') || href.startsWith('mailto:')) {
    return
  }
  if (href.startsWith('#')) {
    // In-page anchors — let browser handle smooth scroll.
    return
  }

  const base = selectedPath.value ?? ''
  const resolved = resolveRelativeWikiPath(base, href)
  if (!resolved) return

  // Try the resolved path directly, then a `.md` fallback for authors who
  // write `./modules/session` without the extension.
  const node =
    findNode(wikiTree.value, resolved) ||
    findNode(wikiTree.value, `${resolved}.md`) ||
    findNode(wikiTree.value, `${resolved}/index.md`)
  if (node && node.kind === 'file') {
    event.preventDefault()
    expandedAncestorPaths.value = ancestorDirs(node.path)
    selectPath(node.path)
    router.replace(`/projects/${props.project}/wiki/${node.path}`)
  }
}

/** After the Markdown is (re-)rendered, rewrite relative <img src> so the
 *  browser pulls assets from the server's `/wiki/asset` endpoint instead
 *  of the current page URL. */
function rewriteRelativeImages() {
  const root = documentRef.value
  if (!root || !selectedPath.value) return
  const images = root.querySelectorAll('img')
  const base = selectedPath.value
  images.forEach((img) => {
    const raw = img.getAttribute('src')
    if (!raw) return
    if (/^[a-z]+:\/\//i.test(raw) || raw.startsWith('/') || raw.startsWith('data:')) return
    const resolved = resolveRelativeWikiPath(base, raw)
    if (!resolved) return
    img.setAttribute('src', wikiAssetUrl(props.project, resolved))
  })
}

watch(
  () => wikiContent.value,
  async () => {
    await nextTick()
    rewriteRelativeImages()
  },
)

async function handleFileUpload(event: Event) {
  const input = event.target as HTMLInputElement
  if (!input.files || input.files.length === 0) return
  uploading.value = true
  uploadError.value = ''
  uploadSuccess.value = ''
  try {
    const files = Array.from(input.files)
    const result = await uploadWikiFiles(props.project, files)
    if (result.uploaded.length > 0) {
      uploadSuccess.value = `${result.uploaded.length}`
      await reload(props.project, selectedPath.value ?? undefined)
    }
    if (result.errors.length > 0) {
      uploadError.value = result.errors.join('; ')
    }
  } catch (err) {
    uploadError.value = err instanceof Error ? err.message : String(err)
  } finally {
    uploading.value = false
    if (input) input.value = ''
  }
}

function openUploadDialog() {
  fileInputRef.value?.click()
}

const isEmpty = computed(() => !loading.value && !error.value && wikiTree.value.length === 0)

const renderKind = computed<WikiRenderKind | null>(() => {
  if (!selectedPath.value) return null
  return pickWikiRenderKind(wikiContentType.value || fileExtension(selectedPath.value))
})

const markdownForCodeView = computed(() => {
  const ext = wikiContentType.value || fileExtension(selectedPath.value ?? '')
  return buildCodeMarkdown(wikiContent.value, ext)
})

const selectedAssetUrl = computed(() =>
  selectedPath.value ? wikiAssetUrl(props.project, selectedPath.value) : '',
)
</script>

<template>
  <section class="bb-wiki-workspace">
    <header class="bb-workspace-head">
      <div class="bb-workspace-head-main">
        <h2>{{ t('wiki') }}</h2>
        <p>{{ t('wikiSubtitle') }}</p>
      </div>
      <div class="bb-workspace-head-actions">
        <span v-if="uploadError" class="bb-wiki-upload-msg error">{{ uploadError }}</span>
        <span v-else-if="uploadSuccess" class="bb-wiki-upload-msg ok">
          {{ t('wikiUploadSuccess') }}
        </span>
        <button
          type="button"
          class="bb-btn-upload"
          :disabled="uploading"
          @click="openUploadDialog"
        >
          <Upload :size="14" />
          <span>{{ uploading ? t('saving') : t('upload') }}</span>
        </button>
      </div>
    </header>

    <input
      ref="fileInputRef"
      type="file"
      multiple
      webkitdirectory
      class="bb-hidden-file-input"
      @change="handleFileUpload"
    />

    <div class="bb-wiki-body">
      <aside
        v-show="!treeCollapsed"
        class="bb-wiki-tree"
        :style="{ width: treeWidth + 'px' }"
      >
        <button
          type="button"
          class="bb-wiki-tree-collapse"
          :title="t('close')"
          @click="toggleTreeCollapse"
        >
          <PanelLeftClose :size="14" />
        </button>
        <div v-if="loading" class="bb-empty">{{ t('loadingDashboard', { project }) }}</div>
        <div v-else-if="error" class="bb-error">{{ error }}</div>
        <div v-else-if="isEmpty" class="bb-empty bb-wiki-empty-hint">
          {{ t('wikiEmpty') }}
        </div>
        <ul v-else class="bb-wiki-tree-list">
          <li v-for="node in wikiTree" :key="node.path">
            <WikiTreeItem
              :node="node"
              :selected-path="selectedPath"
              :depth="0"
              :default-expanded-paths="expandedAncestorPaths"
              @select="onTreeSelect"
            />
          </li>
        </ul>
      </aside>

      <div
        v-if="!treeCollapsed"
        class="bb-wiki-resize-handle"
        @mousedown="startResize"
      />

      <button
        v-if="treeCollapsed"
        type="button"
        class="bb-wiki-tree-expand"
        :title="t('wiki')"
        @click="toggleTreeCollapse"
      >
        <PanelLeft :size="14" />
      </button>

      <main class="bb-wiki-content">
        <div v-if="isEmpty" class="bb-empty bb-wiki-empty-hint">{{ t('wikiEmpty') }}</div>
        <div v-else-if="contentLoading" class="bb-empty">{{ t('loading') }}</div>
        <div v-else-if="contentError" class="bb-error">{{ contentError }}</div>
        <div v-else-if="!selectedPath" class="bb-empty">{{ t('wikiSelectFile') }}</div>
        <template v-else>
          <!-- Markdown: full MarkdownRenderer + TOC side rail -->
          <template v-if="renderKind === 'markdown'">
            <article
              ref="documentRef"
              class="bb-wiki-document"
              @click="handleContentLinkClick"
            >
              <MarkdownRenderer :content="wikiContent" :locale="locale" />
            </article>
            <aside class="bb-wiki-toc">
              <TableOfContents :content="wikiContent" :locale="locale" />
            </aside>
          </template>

          <!-- SVG: side-by-side rendered preview + source code view -->
          <template v-else-if="renderKind === 'svg'">
            <div class="bb-wiki-dual" :class="`bb-wiki-dual-${svgLayout}`">
              <div class="bb-wiki-svg-source">
                <div class="bb-wiki-preview-head">
                  <span>{{ t('wikiSource') }}</span>
                  <div class="bb-wiki-layout-toggle" :aria-label="t('wikiLayout')">
                    <button
                      type="button"
                      class="bb-wiki-layout-btn"
                      :class="{ active: svgLayout === 'side' }"
                      :title="t('wikiLayoutSide')"
                      @click="svgLayout = 'side'"
                    >
                      <Columns2 :size="14" />
                    </button>
                    <button
                      type="button"
                      class="bb-wiki-layout-btn"
                      :class="{ active: svgLayout === 'stack' }"
                      :title="t('wikiLayoutStack')"
                      @click="svgLayout = 'stack'"
                    >
                      <Rows2 :size="14" />
                    </button>
                  </div>
                </div>
                <MarkdownRenderer :content="buildCodeMarkdown(wikiContent, 'xml')" :locale="locale" />
              </div>
              <div class="bb-wiki-svg-preview">
                <div class="bb-wiki-preview-head">{{ t('wikiPreview') }}</div>
                <div class="bb-wiki-svg-canvas" v-html="wikiContent" />
              </div>
            </div>
          </template>

          <!-- Other text formats: syntax-highlighted code block -->
          <template v-else-if="renderKind === 'code'">
            <article class="bb-wiki-document bb-wiki-code-view">
              <div class="bb-wiki-preview-head">
                {{ selectedPath }} · {{ wikiContentType || fileExtension(selectedPath || '') }}
              </div>
              <MarkdownRenderer :content="markdownForCodeView" :locale="locale" />
            </article>
          </template>

          <!-- Raster image: <img> via asset endpoint -->
          <template v-else-if="renderKind === 'image'">
            <div class="bb-wiki-binary">
              <img
                :src="selectedAssetUrl"
                :alt="selectedPath || ''"
                class="bb-wiki-binary-image"
              />
            </div>
          </template>

          <!-- PDF: <iframe> via asset endpoint -->
          <template v-else-if="renderKind === 'pdf'">
            <div class="bb-wiki-binary">
              <iframe
                :src="selectedAssetUrl"
                class="bb-wiki-binary-pdf"
                :title="selectedPath || t('wikiFileTypePdf')"
              />
            </div>
          </template>
        </template>
      </main>
    </div>
  </section>
</template>

<style scoped>
.bb-wiki-workspace {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.bb-wiki-body {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  gap: 12px;
  padding-top: 12px;
}

.bb-wiki-tree {
  position: relative;
  flex-shrink: 0;
  overflow-y: auto;
  border: 1px solid var(--bb-border-warm);
  border-radius: 8px;
  background: var(--bb-canvas);
  padding: 8px;
}

.bb-wiki-tree-collapse {
  position: sticky;
  top: 0;
  margin-left: auto;
  margin-bottom: 8px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: 1px solid var(--bb-border-warm);
  background: var(--bb-surface);
  cursor: pointer;
  color: var(--bb-text-muted);
  border-radius: 6px;
  z-index: 1;
}
.bb-wiki-tree-collapse:hover {
  border-color: var(--bb-theme-primary-border);
  background: var(--bb-theme-primary-soft);
  color: var(--bb-text-strong);
}

.bb-wiki-resize-handle {
  width: 2px;
  flex-shrink: 0;
  cursor: col-resize;
  background: transparent;
  transition: background 0.15s;
}
.bb-wiki-resize-handle:hover {
  background: var(--bb-hairline-strong);
}

.bb-wiki-tree-expand {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  flex-shrink: 0;
  border: none;
  background: transparent;
  cursor: pointer;
  color: var(--bb-text-muted);
  border-radius: 0.25rem;
  margin: 0.25rem;
}
.bb-wiki-tree-expand:hover {
  background: var(--bb-surface-soft);
  color: var(--bb-text-strong);
}

.bb-wiki-tree-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.bb-wiki-content {
  flex: 1;
  display: flex;
  min-width: 0;
  overflow: hidden;
  gap: 1rem;
  padding: 1rem;
  background: var(--bb-canvas);
}

.bb-wiki-document {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding-right: 0.75rem;
}

.bb-wiki-document :deep(img) {
  max-width: 100%;
  height: auto;
}

.bb-wiki-toc {
  width: 220px;
  flex-shrink: 0;
  overflow-y: auto;
  border-left: 1px solid var(--bb-hairline);
  padding-left: 1rem;
}

.bb-wiki-dual {
  flex: 1;
  display: grid;
  gap: 1rem;
  min-width: 0;
  overflow: hidden;
}

.bb-wiki-dual-side {
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
}

.bb-wiki-dual-stack {
  grid-template-rows: minmax(0, 1fr) minmax(0, 1fr);
}

.bb-wiki-dual > * {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.bb-wiki-preview-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--bb-text-muted);
  text-transform: uppercase;
  letter-spacing: 0;
  padding-bottom: 0.5rem;
  border-bottom: 1px solid var(--bb-hairline);
  margin-bottom: 0.5rem;
  flex-shrink: 0;
}

.bb-wiki-layout-toggle {
  display: inline-flex;
  align-items: center;
  gap: 0.125rem;
  padding: 0.125rem;
  border: 1px solid var(--bb-hairline);
  border-radius: 0.375rem;
  background: var(--bb-surface);
}

.bb-wiki-layout-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: 0;
  border-radius: 0.25rem;
  color: var(--bb-text-muted);
  background: transparent;
  cursor: pointer;
}

.bb-wiki-layout-btn:hover,
.bb-wiki-layout-btn.active {
  color: var(--bb-text-strong);
  background: var(--bb-surface-soft);
}

.bb-wiki-svg-canvas {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: flex;
  align-items: center;
  justify-content: center;
  background:
    linear-gradient(45deg, color-mix(in srgb, var(--bb-text-faint) 8%, transparent) 25%, transparent 25%) 0 0 / 16px 16px,
    linear-gradient(-45deg, color-mix(in srgb, var(--bb-text-faint) 8%, transparent) 25%, transparent 25%) 0 8px / 16px 16px,
    linear-gradient(45deg, transparent 75%, color-mix(in srgb, var(--bb-text-faint) 8%, transparent) 75%) 8px 0 / 16px 16px,
    linear-gradient(-45deg, transparent 75%, color-mix(in srgb, var(--bb-text-faint) 8%, transparent) 75%) 8px 8px / 16px 16px,
    var(--bb-surface-soft);
  border-radius: 6px;
  padding: 0.75rem;
}

.bb-wiki-svg-canvas :deep(svg) {
  max-width: 100%;
  max-height: 100%;
  height: auto;
}

.bb-wiki-svg-source {
  overflow: auto;
}

.bb-wiki-code-view {
  flex: 1;
  min-width: 0;
  overflow: auto;
}

.bb-wiki-binary {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 0;
  min-height: 0;
  overflow: auto;
  padding: 1rem;
}

.bb-wiki-binary-image {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}

.bb-wiki-binary-pdf {
  width: 100%;
  height: 100%;
  border: 0;
  background: var(--bb-surface);
}

.bb-empty {
  color: var(--bb-text-muted);
  padding: 1rem;
}

.bb-wiki-empty-hint {
  line-height: 1.6;
}

.bb-error {
  color: var(--bb-error);
  padding: 1rem;
}

.bb-workspace-head-actions {
  max-width: 100%;
}

.bb-wiki-upload-msg {
  font-size: 0.8rem;
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.bb-wiki-upload-msg.error {
  color: var(--bb-error);
}
.bb-wiki-upload-msg.ok {
  color: var(--bb-success);
}

.bb-btn-upload {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  padding: 0.375rem 0.75rem;
  border: 1px solid var(--bb-hairline);
  background: var(--bb-project-blackboard-bg);
  color: var(--bb-project-blackboard-fg);
  cursor: pointer;
  border-radius: 0.375rem;
  font-size: 0.85rem;
  white-space: nowrap;
}
.bb-btn-upload:hover:not(:disabled) {
  background: color-mix(in srgb, var(--bb-project-blackboard-bg) 86%, var(--bb-surface));
}
.bb-btn-upload:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.bb-hidden-file-input {
  display: none;
}
</style>
