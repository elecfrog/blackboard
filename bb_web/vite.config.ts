import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

// Dev server proxies /api to the locally running bb-server HTTP listener
// (cargo run -p bb-server -- ... http --addr 127.0.0.1:3001). The front end
// always talks to relative paths like `/api/projects/...`; in production the
// same URL is expected to be reverse-proxied by whatever static host is used.
const apiTarget = process.env.BLACKBOARD_API_TARGET ?? 'http://127.0.0.1:3001'

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
        manualChunks: {
          'vendor-vue': ['vue', 'vue-router'],
          'vendor-ui': ['tdesign-vue-next'],
          'vendor-mermaid': ['mermaid'],
          'vendor-md': ['markdown-it', 'markdown-it-anchor', 'markdown-it-task-lists'],
          'vendor-icons': ['lucide-vue-next'],
          'vendor-hl': ['highlight.js'],
        },
      },
    },
    chunkSizeWarningLimit: 500,
  },
})
