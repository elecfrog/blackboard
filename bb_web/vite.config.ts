import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

// Dev server proxies /api to bb_server. bb_daemon stays on 3001 for local MCP,
// while bb_server serves the web/API facade on 3002.
const apiTarget = process.env.BLACKBOARD_API_TARGET ?? 'http://127.0.0.1:3002'

function manualChunks(id: string) {
  const normalized = id.replace(/\\/g, '/')
  if (!normalized.includes('/node_modules/')) return undefined

  if (normalized.includes('/node_modules/vue') || normalized.includes('/node_modules/@vue/')) {
    return 'vendor-vue'
  }
  if (normalized.includes('/node_modules/@tdesign-vue-next/chat/')) {
    return 'vendor-chat'
  }
  if (normalized.includes('/node_modules/tdesign-vue-next/')) {
    return 'vendor-ui'
  }
  if (normalized.includes('/node_modules/lucide-vue-next/')) {
    return 'vendor-icons'
  }
  if (normalized.includes('/node_modules/markdown-it')) {
    return 'vendor-md'
  }
  if (normalized.includes('/node_modules/highlight.js/')) {
    if (normalized.includes('/lib/languages/')) return 'vendor-hl-languages'
    return 'vendor-hl-core'
  }
  if (normalized.includes('/node_modules/@mermaid-js/')) {
    return 'vendor-mermaid-parser'
  }
  if (normalized.includes('/node_modules/dagre') || normalized.includes('/node_modules/dagre-d3-es/')) {
    return 'vendor-mermaid-dagre'
  }
  if (normalized.includes('/node_modules/d3')) {
    return 'vendor-mermaid-d3'
  }
  if (normalized.includes('/node_modules/roughjs/')) {
    return 'vendor-mermaid-rough'
  }
  if (normalized.includes('/node_modules/mermaid/')) {
    if (normalized.includes('/dist/chunks/')) return undefined
    return 'vendor-mermaid'
  }
  if (normalized.includes('/node_modules/cytoscape-cose-bilkent/')) {
    return 'vendor-graph-cose-bilkent'
  }
  if (normalized.includes('/node_modules/cytoscape-fcose/')) {
    return 'vendor-graph-fcose'
  }
  if (normalized.includes('/node_modules/cytoscape/')) {
    return 'vendor-graph-cytoscape'
  }
  if (normalized.includes('/node_modules/katex')) {
    return 'vendor-katex'
  }

  return undefined
}

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  server: {
    proxy: {
      '/api': {
        target: apiTarget,
        changeOrigin: true,
      },
    },
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks,
      },
    },
    // Mermaid's lazy-loaded core lands just over Vite's 500 kB default after the
    // app, graph, highlight, and UI vendors are split. Keep the budget close to
    // that ceiling so accidental app/vendor growth still warns.
    chunkSizeWarningLimit: 520,
  },
})
