export interface IdeaCanvasEntry {
  id: string
  title: string
  note_count: number
  created_at: string
  updated_at: string
}

export interface IdeaCanvasIndex {
  generated_at: string
  canvases: IdeaCanvasEntry[]
}

export interface StickyNote {
  id: string
  text: string
  x: number
  y: number
  color: string
  created_at: string
  updated_at: string
}

export interface IdeaCanvasDetail {
  id: string
  title: string
  created_at: string
  updated_at: string
  notes: StickyNote[]
}

export interface IdeaCanvasWriteResult {
  canvas: IdeaCanvasDetail
}

export interface StickyNoteWriteResult {
  canvas: IdeaCanvasDetail
  note: StickyNote
}

export interface StickyNoteDeleteResult {
  canvas_id: string
  note_id: string
}

export interface IdeaCanvasDeleteResult {
  id: string
}

export interface CreateIdeaCanvasInput {
  title: string
  id?: string
}

export interface PatchIdeaCanvasInput {
  title?: string
}

export interface CreateStickyNoteInput {
  text?: string
  x: number
  y: number
  color?: string
}

export interface PatchStickyNoteInput {
  text?: string
  x?: number
  y?: number
  color?: string
}

async function responseError(response: Response, fallback: string): Promise<string> {
  try {
    const payload = (await response.json()) as { error?: { message?: string } }
    const message = payload.error?.message
    if (message) return `${fallback}: ${message}`
  } catch {
    /* ignore malformed error body */
  }
  return `${fallback}: HTTP ${response.status} ${response.statusText}`
}

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url, { cache: 'no-cache' })
  if (!response.ok) throw new Error(await responseError(response, `GET ${url} failed`))
  return (await response.json()) as T
}

function projectPath(project: string) {
  return `/api/projects/${encodeURIComponent(project)}/idea-canvases`
}

function canvasPath(project: string, canvasId: string) {
  return `${projectPath(project)}/${encodeURIComponent(canvasId)}`
}

function notePath(project: string, canvasId: string, noteId: string) {
  return `${canvasPath(project, canvasId)}/notes/${encodeURIComponent(noteId)}`
}

export function loadIdeaCanvases(project: string): Promise<IdeaCanvasIndex> {
  return fetchJson<IdeaCanvasIndex>(projectPath(project))
}

export function loadIdeaCanvas(project: string, canvasId: string): Promise<IdeaCanvasDetail> {
  return fetchJson<IdeaCanvasDetail>(canvasPath(project, canvasId))
}

export async function createIdeaCanvas(
  project: string,
  input: CreateIdeaCanvasInput,
): Promise<IdeaCanvasWriteResult> {
  const response = await fetch(projectPath(project), {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
  if (!response.ok) throw new Error(await responseError(response, 'create_idea_canvas failed'))
  return (await response.json()) as IdeaCanvasWriteResult
}

export async function patchIdeaCanvas(
  project: string,
  canvasId: string,
  input: PatchIdeaCanvasInput,
): Promise<IdeaCanvasWriteResult> {
  const response = await fetch(canvasPath(project, canvasId), {
    method: 'PATCH',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
  if (!response.ok) throw new Error(await responseError(response, 'update_idea_canvas failed'))
  return (await response.json()) as IdeaCanvasWriteResult
}

export async function deleteIdeaCanvas(project: string, canvasId: string): Promise<IdeaCanvasDeleteResult> {
  const response = await fetch(canvasPath(project, canvasId), { method: 'DELETE' })
  if (!response.ok) throw new Error(await responseError(response, 'delete_idea_canvas failed'))
  return (await response.json()) as IdeaCanvasDeleteResult
}

export async function createStickyNote(
  project: string,
  canvasId: string,
  input: CreateStickyNoteInput,
): Promise<StickyNoteWriteResult> {
  const response = await fetch(`${canvasPath(project, canvasId)}/notes`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
  if (!response.ok) throw new Error(await responseError(response, 'create_sticky_note failed'))
  return (await response.json()) as StickyNoteWriteResult
}

export async function patchStickyNote(
  project: string,
  canvasId: string,
  noteId: string,
  input: PatchStickyNoteInput,
): Promise<StickyNoteWriteResult> {
  const response = await fetch(notePath(project, canvasId, noteId), {
    method: 'PATCH',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
  if (!response.ok) throw new Error(await responseError(response, 'update_sticky_note failed'))
  return (await response.json()) as StickyNoteWriteResult
}

export async function deleteStickyNote(
  project: string,
  canvasId: string,
  noteId: string,
): Promise<StickyNoteDeleteResult> {
  const response = await fetch(notePath(project, canvasId, noteId), { method: 'DELETE' })
  if (!response.ok) throw new Error(await responseError(response, 'delete_sticky_note failed'))
  return (await response.json()) as StickyNoteDeleteResult
}
