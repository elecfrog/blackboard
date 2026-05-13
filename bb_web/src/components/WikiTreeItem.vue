<script setup lang="ts">
/**
 * One row in the wiki file tree. Recursive: a directory renders its
 * children via another `WikiTreeItem`.
 *
 * Behaviour contract (intentionally narrow, to avoid the selection/toggle
 * collision the earlier implementation had):
 *   - clicking a directory toggles expand/collapse; it never selects the
 *     directory or navigates away.
 *   - clicking a file emits `select` upward; the parent is responsible for
 *     loading its content and updating the route.
 */
import { ref } from 'vue'
import { ChevronDown, ChevronRight, FileText, Folder } from 'lucide-vue-next'
import type { WikiTreeNode } from '@/data/wiki'

const props = withDefaults(
  defineProps<{
    node: WikiTreeNode
    selectedPath: string | null
    depth?: number
    /** Paths to auto-expand when the component mounts (e.g. ancestors of
     * the currently selected document). */
    defaultExpandedPaths?: string[]
  }>(),
  { depth: 0, defaultExpandedPaths: () => [] },
)

const emit = defineEmits<{
  (event: 'select', node: WikiTreeNode): void
}>()

// Directories default to collapsed unless explicitly flagged as an ancestor
// of the selected document. This keeps deep trees compact.
const expanded = ref<boolean>(
  props.node.kind === 'dir' && props.defaultExpandedPaths.includes(props.node.path),
)

function onRowClick() {
  if (props.node.kind === 'dir') {
    expanded.value = !expanded.value
  } else {
    emit('select', props.node)
  }
}

function onChildSelect(child: WikiTreeNode) {
  emit('select', child)
}
</script>

<template>
  <div class="bb-wiki-tree-node">
    <button
      type="button"
      class="bb-wiki-tree-row"
      :class="{ selected: node.kind === 'file' && selectedPath === node.path }"
      :style="{ paddingLeft: depth * 14 + 8 + 'px' }"
      @click="onRowClick"
    >
      <span class="bb-wiki-tree-icon">
        <component
          :is="
            node.kind === 'dir'
              ? expanded
                ? ChevronDown
                : ChevronRight
              : FileText
          "
          :size="14"
        />
      </span>
      <span v-if="node.kind === 'dir'" class="bb-wiki-tree-icon bb-wiki-tree-folder">
        <Folder :size="14" />
      </span>
      <span class="bb-wiki-tree-name">{{ node.name }}</span>
    </button>
    <ul
      v-if="node.kind === 'dir' && expanded && node.children && node.children.length > 0"
      class="bb-wiki-tree-children"
    >
      <li v-for="child in node.children" :key="child.path">
        <WikiTreeItem
          :node="child"
          :selected-path="selectedPath"
          :depth="depth + 1"
          :default-expanded-paths="defaultExpandedPaths"
          @select="onChildSelect"
        />
      </li>
    </ul>
  </div>
</template>

<style scoped>
.bb-wiki-tree-node {
  display: grid;
  gap: 6px;
}

.bb-wiki-tree-row {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  min-height: 32px;
  padding: 7px 10px;
  border: 1px solid var(--bb-border-warm);
  background: var(--bb-surface);
  cursor: pointer;
  text-align: left;
  border-radius: 8px;
  font-size: 12px;
  color: var(--bb-text);
  transition: border-color 120ms ease, background 120ms ease, color 120ms ease;
}

.bb-wiki-tree-row:hover {
  border-color: var(--bb-theme-primary-border);
  background: var(--bb-theme-primary-soft);
  color: var(--bb-text-strong);
}

.bb-wiki-tree-row.selected {
  border-color: var(--bb-theme-primary-border-strong);
  background: var(--bb-theme-primary-soft);
  color: var(--bb-theme-primary);
  font-weight: 720;
}

.bb-wiki-tree-icon {
  display: inline-flex;
  align-items: center;
  flex-shrink: 0;
  color: var(--bb-text-muted);
}

.bb-wiki-tree-folder {
  color: var(--bb-text-muted);
}

.bb-wiki-tree-row:hover .bb-wiki-tree-icon,
.bb-wiki-tree-row:hover .bb-wiki-tree-folder,
.bb-wiki-tree-row.selected .bb-wiki-tree-icon,
.bb-wiki-tree-row.selected .bb-wiki-tree-folder {
  color: currentColor;
}

.bb-wiki-tree-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bb-wiki-tree-children {
  list-style: none;
  margin: 0 0 0 10px;
  padding: 0;
  display: grid;
  gap: 6px;
}
</style>
