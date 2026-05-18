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
import BbObjectItem from '@/components/common/BbObjectItem.vue'
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
    <BbObjectItem
      :title="node.name"
      class="bb-wiki-tree-row"
      :active="node.kind === 'file' && selectedPath === node.path"
      :style="{ paddingLeft: depth * 14 + 8 + 'px' }"
      @select="onRowClick"
    >
      <template #leading>
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
        <Folder v-if="node.kind === 'dir'" :size="14" />
      </template>
    </BbObjectItem>
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
  gap: 4px;
}

.bb-wiki-tree-row {
  --bb-object-item-height: 32px;
}

.bb-wiki-tree-row :deep(.bb-object-item-leading) {
  gap: 6px;
}

.bb-wiki-tree-children {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 4px;
}
</style>
