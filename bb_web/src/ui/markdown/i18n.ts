export type MarkdownLocale = 'zh' | 'en'
export type MarkdownThemeMode = 'light' | 'dark' | 'auto'

export interface MarkdownRendererLabels {
  copyCode: string
  copied: string
  close: string
  downloadPng: string
  downloadSvg: string
  mermaidClickToEnlarge: string
  mermaidImageAlt: string
  mermaidLoading: string
  mermaidReset: string
  mermaidResetTitle: string
  mermaidSyntaxError: string
  mermaidUnavailable: string
  mermaidZoomHint: string
  renderError: string
}

export interface TableOfContentsLabels {
  title: string
}

const markdownRendererMessages = {
  zh: {
    copyCode: '复制代码',
    copied: '已复制',
    close: '关闭',
    downloadPng: '下载 PNG',
    downloadSvg: '下载 SVG',
    mermaidClickToEnlarge: '点击放大',
    mermaidImageAlt: 'Mermaid 图表',
    mermaidLoading: '正在渲染图表...',
    mermaidReset: '重置',
    mermaidResetTitle: '重置位置和缩放',
    mermaidSyntaxError: 'Mermaid 图表语法错误',
    mermaidUnavailable: 'Mermaid 图表暂不可用',
    mermaidZoomHint: '滚轮缩放，拖拽移动',
    renderError: 'Markdown 渲染失败',
  },
  en: {
    copyCode: 'Copy code',
    copied: 'Copied',
    close: 'Close',
    downloadPng: 'Download PNG',
    downloadSvg: 'Download SVG',
    mermaidClickToEnlarge: 'Click to enlarge',
    mermaidImageAlt: 'Mermaid diagram',
    mermaidLoading: 'Rendering diagram...',
    mermaidReset: 'Reset',
    mermaidResetTitle: 'Reset position and zoom',
    mermaidSyntaxError: 'Mermaid diagram syntax error',
    mermaidUnavailable: 'Mermaid diagram unavailable',
    mermaidZoomHint: 'Scroll to zoom, drag to move',
    renderError: 'Failed to render markdown',
  },
} satisfies Record<MarkdownLocale, MarkdownRendererLabels>

const tableOfContentsMessages = {
  zh: {
    title: '目录',
  },
  en: {
    title: 'Table of Contents',
  },
} satisfies Record<MarkdownLocale, TableOfContentsLabels>

export function resolveMarkdownLocale(locale?: string): MarkdownLocale {
  if (locale === 'zh' || locale === 'en') return locale
  if (typeof document === 'undefined') return 'en'
  const lang = document.documentElement.lang.toLowerCase()
  return lang.startsWith('zh') ? 'zh' : 'en'
}

export function resolveMarkdownLabels(
  locale?: string,
  overrides: Partial<MarkdownRendererLabels> = {},
): MarkdownRendererLabels {
  const resolved = resolveMarkdownLocale(locale)
  return {
    ...markdownRendererMessages[resolved],
    ...overrides,
  }
}

export function resolveTableOfContentsLabels(
  locale?: string,
  overrides: Partial<TableOfContentsLabels> = {},
): TableOfContentsLabels {
  const resolved = resolveMarkdownLocale(locale)
  return {
    ...tableOfContentsMessages[resolved],
    ...overrides,
  }
}
