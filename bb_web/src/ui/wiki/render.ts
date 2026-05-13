export type WikiRenderKind = 'markdown' | 'svg' | 'code' | 'image' | 'pdf'

const MARKDOWN_EXTENSIONS = new Set(['md', 'markdown'])
const BINARY_IMAGE_EXTENSIONS = new Set([
  'png',
  'jpg',
  'jpeg',
  'gif',
  'webp',
  'avif',
  'ico',
  'bmp',
])
const IFRAME_EXTENSIONS = new Set(['pdf'])

export function fileExtension(path: string): string {
  const dot = path.lastIndexOf('.')
  if (dot < 0) return ''
  return path.slice(dot + 1).toLowerCase()
}

export function pickWikiRenderKind(ext: string): WikiRenderKind {
  if (MARKDOWN_EXTENSIONS.has(ext)) return 'markdown'
  if (ext === 'svg') return 'svg'
  if (BINARY_IMAGE_EXTENSIONS.has(ext)) return 'image'
  if (IFRAME_EXTENSIONS.has(ext)) return 'pdf'
  return 'code'
}

export function codeLanguageForExtension(ext: string): string {
  switch (ext) {
    case 'ts':
    case 'tsx':
      return 'typescript'
    case 'js':
    case 'jsx':
    case 'mjs':
    case 'cjs':
      return 'javascript'
    case 'yml':
      return 'yaml'
    case 'sh':
    case 'bash':
    case 'zsh':
      return 'bash'
    case 'rs':
      return 'rust'
    case 'py':
      return 'python'
    case 'rb':
      return 'ruby'
    case 'kt':
    case 'kts':
      return 'kotlin'
    case 'cc':
    case 'cpp':
    case 'hpp':
      return 'cpp'
    case 'htm':
      return 'html'
    case 'scss':
    case 'less':
      return 'css'
    default:
      return ext
  }
}

export function buildCodeMarkdown(content: string, ext: string): string {
  const lang = codeLanguageForExtension(ext) || 'text'
  let fence = '```'
  while (content.includes(fence)) fence += '`'
  return `${fence}${lang}\n${content}\n${fence}\n`
}
