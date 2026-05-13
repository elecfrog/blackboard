/**
 * Data layer for the project-level wiki (`projects/<project>/wiki/`).
 *
 * These endpoints are served read-only by `bb_server` (plus a single
 * multipart upload endpoint). The contract intentionally mirrors how
 * inbox/tickets talk to the server: small typed wrappers around `fetch`,
 * project-scoped URLs, and JSON in / JSON out — except `assetUrl`, which
 * returns a direct URL so the browser can fetch images via `<img src>`.
 */

export type WikiNodeKind = 'file' | 'dir'

export interface WikiTreeNode {
  name: string
  /** POSIX-style path relative to `projects/<project>/wiki/`. */
  path: string
  kind: WikiNodeKind
  children?: WikiTreeNode[]
}

export interface WikiTreeResponse {
  generated_at: string
  tree: WikiTreeNode[]
}

export interface WikiContentResponse {
  content: string
  /** Lowercased extension hint from the server (e.g. `"md"`, `"svg"`,
   * `"json"`). The WikiPanel uses it to choose Markdown / SVG dual-view /
   * syntax-highlighted code rendering. */
  content_type: string
}

export interface WikiUploadResponse {
  uploaded: string[]
  errors: string[]
}

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url, { cache: 'no-cache' })
  if (!response.ok) {
    // Pull the structured error body the server emits (`{ error: { message } }`)
    // so callers can surface a useful message. Fall back to the HTTP status
    // if parsing fails.
    let message = `HTTP ${response.status} ${response.statusText} for ${url}`
    try {
      const payload = (await response.json()) as { error?: { message?: string } }
      if (payload.error?.message) {
        message = `${message}: ${payload.error.message}`
      }
    } catch {
      /* ignore */
    }
    throw new Error(message)
  }
  return (await response.json()) as T
}

/** List the full wiki tree. Empty tree when the directory doesn't exist. */
export async function loadWikiTree(project: string): Promise<WikiTreeResponse> {
  const encoded = encodeURIComponent(project)
  return fetchJson<WikiTreeResponse>(`/api/projects/${encoded}/wiki/tree`)
}

/**
 * Read one wiki document by its wiki-relative path. Returns both the raw
 * text content and a `contentType` hint (the lowercased file extension)
 * so callers can branch on renderer.
 */
export async function loadWikiContent(
  project: string,
  path: string,
): Promise<{ content: string; contentType: string }> {
  const encoded = encodeURIComponent(project)
  const qs = encodeURIComponent(path)
  const payload = await fetchJson<WikiContentResponse>(
    `/api/projects/${encoded}/wiki/file?path=${qs}`,
  )
  return { content: payload.content, contentType: payload.content_type }
}

/**
 * Build a direct asset URL for an `<img src>` / `<a href>` reference to a
 * wiki-relative file. The same path validation rules as the read endpoint
 * apply server-side; non-whitelisted extensions return 400.
 */
export function wikiAssetUrl(project: string, path: string): string {
  const encoded = encodeURIComponent(project)
  const qs = encodeURIComponent(path)
  return `/api/projects/${encoded}/wiki/asset?path=${qs}`
}

/**
 * Multipart upload into `projects/<project>/wiki/`. Files keep their
 * `webkitRelativePath` when available so uploading a whole directory
 * rebuilds its layout under the wiki root.
 */
export async function uploadWikiFiles(
  project: string,
  files: File[],
): Promise<WikiUploadResponse> {
  const encoded = encodeURIComponent(project)
  const formData = new FormData()
  for (const file of files) {
    // Preserve directory structure when the browser provides it; fall back
    // to the bare filename for single-file pickers.
    const relPath = (file as File & { webkitRelativePath?: string }).webkitRelativePath
    formData.append('file', file, relPath && relPath.length > 0 ? relPath : file.name)
  }
  const response = await fetch(`/api/projects/${encoded}/wiki/upload`, {
    method: 'POST',
    body: formData,
  })
  if (!response.ok) {
    const text = await response.text()
    throw new Error(`Upload failed: ${response.status} ${text}`)
  }
  return response.json() as Promise<WikiUploadResponse>
}
