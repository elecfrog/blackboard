export type TicketStatus = 'todo' | 'in_progress' | 'blocked' | 'review' | 'done' | 'archived'

/// Ticket shape after the lane refactor. Only fields the platform actively
/// owns (id/lane/title/status/created_at/updated_at) are first-class; every
/// other frontmatter key is exposed verbatim through `extra`. `assignee`,
/// historical `current` / `family`, bespoke tags — all live there.
export interface BlackboardTicket {
  id: string
  lane: string
  title: string
  status: string
  created_at: string
  updated_at: string
  file_name: string
  file_path: string
  dependencies: string[]
  attachments?: TicketAttachment[]
  /// Markdown body content. Only populated when the ticket detail is loaded
  /// on demand via `loadTicketContent`; the list endpoint no longer returns
  /// this field (index/content split).
  content?: string
  /// Arbitrary frontmatter KV pairs carried through from bb-core verbatim.
  /// Known conventions today: `assignee`. Legacy data may still carry
  /// `current` / `family` until those files are cleaned up via
  /// `update_ticket` + `remove`.
  extra: Record<string, string>
}

export interface TicketAttachment {
  kind: string
  target: string
  label?: string
  description?: string
}

/// Lane definition declared in `__project__.json` under `lanes`. Replaces the
/// former hard-coded `familyMeta` map; the front-end loads this catalog per
/// project so users can add or archive lanes without code changes.
export interface LaneDef {
  id: string
  label: string
  color: string
  description: string
  status: 'active' | 'archived' | string
}

export interface LanesPayload {
  lanes: LaneDef[]
  generated_at?: string
  project?: string
}

export interface ProjectBoardViewSettings {
  hidden_statuses: string[]
}

export interface ProjectMeta {
  name: string
  type: string
  repos: string[]
  description?: string
  data_root?: string
  local_host?: string
  /// Lane catalog from `__project__.json`, exposed by bb-server as runtime data.
  lanes?: LaneDef[]
  /// Project-scoped BoardView preferences persisted in `__project__.json`.
  board_view?: ProjectBoardViewSettings
}

export interface ProjectEntry {
  name: string
  uuid: string
  meta: ProjectMeta
}

export interface ProjectsPayload {
  generated_at?: string
  projects: ProjectEntry[]
}

export interface ProjectDirectorySelection {
  data_root: string
}

export interface ProjectDirectoryInspection extends ProjectDirectorySelection {
  meta: ProjectMeta
  indexed_as?: string | null
}

export interface ProjectDirectoryCreateInput {
  name: string
  data_root: string
  uuid?: string
  display_name?: string
  type?: string
  repos?: string[]
  description?: string | null
}

export interface ProjectDirectoryOpenInput {
  name: string
  data_root: string
}

export type WorkspaceFolderStatus = 'valid' | 'uninitialized' | 'invalid'

export interface WorkspaceFolderInspection {
  root: string
  status: WorkspaceFolderStatus
  message?: string | null
  suggested_project_name: string
  suggested_display_name: string
  active_project?: string | null
  projects: ProjectEntry[]
}

export interface WorkspaceFolderOpenInput {
  root: string
  initialize: boolean
}

export interface WorkspaceFolderOpenResult {
  root: string
  active_project?: string | null
  projects: ProjectEntry[]
}

export interface BlackboardPayload {
  generated_at: string
  project: string
  meta: ProjectMeta
  current_counter: string
  tickets: BlackboardTicket[]
}

export interface TicketWriteTicket {
  id: string
  lane: string
  title: string
  status: string
  created_at: string
  updated_at: string
  file_name: string
  path: string
  extra: Record<string, string>
}

export interface TicketWriteResult {
  ticket: TicketWriteTicket
}

export interface DeprecateTicketResult {
  ticket: TicketWriteTicket
  removed_dependency_refs: string[]
  removed_attachment_refs: string[]
}

export interface PatchTicketInput {
  status?: string
  lane?: string
  assignee?: string
  depends_on?: string[]
  attachments?: TicketAttachment[]
}

// Matches the shape returned by bb-server at
// GET /api/projects/{project}/inbox/notes.
export interface InboxNoteEntry {
  name: string
  size?: number
  modified_at?: string
  excerpt?: string
}

export interface InboxNotesPayload {
  notes: InboxNoteEntry[]
  generated_at?: string
  project?: string
}

// Matches GET /api/projects/{project}/inbox/notes/{name}.
export interface InboxNote {
  name: string
  content: string
}

// Mirrors bb_core::BoardSummary returned by the stdio board_summary tool and
// the new HTTP endpoint GET /api/projects/{project}/board/summary. Fields use
// snake_case to match the backend JSON directly, no camelCase conversion.
export interface TicketMetadataDiagnostic {
  path: string
  message: string
}

export interface BoardSummary {
  total: number
  by_status: Record<string, number>
  /// Renamed from `by_family` after the lane refactor. Tickets without a
  /// resolvable lane land under the placeholder key `__unknown`.
  by_lane: Record<string, number>
  metadata_warning_count: number
  metadata_error_count: number
  metadata_warnings: TicketMetadataDiagnostic[]
  metadata_errors: TicketMetadataDiagnostic[]
}

// Source tag kept only for runtime/error state. The web app requires the
// bb-server runtime API for Blackboard data.
export type DataSource = 'rest' | 'none'

export interface Loaded<T> {
  data: T
  source: DataSource
  error?: string
}

/// Built-in lane metadata kept as a final visual fallback for historical
/// tickets that cite a lane missing from the runtime project catalog.
export const BUILTIN_LANE_META: Record<string, { label: string; color: string; description: string }> = {
  bbp: { label: '产品', color: '#7c3aed', description: '商业化、合规、产品准入、发布门槛' },
  bbt: { label: '后端', color: '#0f766e', description: 'VM、数据模型、模块功能、服务端、工程基建' },
  bbd: { label: '前端', color: '#2563eb', description: 'UI、人机交互、设计实现、页面入口、视觉基线' },
  bbq: { label: '质量', color: '#ea580c', description: '报错警告、测试工程、多模态与人工冒烟' },
}

export const projectTypeLabels: Record<string, string> = {
  workflow: '工作流',
  product: '产品',
  tool: '工具',
  research: '研究',
  business: '业务',
  game: '游戏',
}

export const ticketStatusOrder: ReadonlyArray<TicketStatus> = [
  'todo',
  'in_progress',
  'review',
  'done',
  'blocked',
  'archived',
]

export const openTicketStatuses: ReadonlyArray<TicketStatus> = [
  'todo',
  'in_progress',
  'blocked',
  'review',
]

export function isTicketStatus(value: string): value is TicketStatus {
  return ticketStatusOrder.includes(value as TicketStatus)
}

export function isOpenTicketStatus(value: string) {
  return openTicketStatuses.includes(value as TicketStatus)
}

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url, { cache: 'no-cache' })
  if (!response.ok) {
    throw new Error(`HTTP ${response.status} ${response.statusText} for ${url}`)
  }
  return (await response.json()) as T
}

async function responseError(response: Response, fallback: string): Promise<string> {
  try {
    const payload = (await response.json()) as { error?: { message?: string } }
    const message = payload.error?.message
    if (message) return `${fallback}: ${message}`
  } catch {
    // Ignore malformed error bodies and use the HTTP status below.
  }
  return `${fallback}: HTTP ${response.status} ${response.statusText}`
}

export async function loadProjects(): Promise<ProjectsPayload> {
  return fetchJson<ProjectsPayload>('/api/projects')
}

export async function selectCreateProjectDirectoryFromDialog(): Promise<ProjectDirectorySelection> {
  const response = await fetch('/api/projects/select-create-directory', { method: 'POST' })
  if (!response.ok) {
    throw new Error(await responseError(response, 'select project directory failed'))
  }
  return (await response.json()) as ProjectDirectorySelection
}

export async function selectOpenProjectDirectoryFromDialog(): Promise<ProjectDirectoryInspection> {
  const response = await fetch('/api/projects/select-open-directory', { method: 'POST' })
  if (!response.ok) {
    throw new Error(await responseError(response, 'select project directory failed'))
  }
  return (await response.json()) as ProjectDirectoryInspection
}

export async function createProjectDirectory(input: ProjectDirectoryCreateInput): Promise<ProjectEntry> {
  const response = await fetch('/api/projects/create', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
  if (!response.ok) {
    throw new Error(await responseError(response, 'create project directory failed'))
  }
  return (await response.json()) as ProjectEntry
}

export async function openProjectDirectory(input: ProjectDirectoryOpenInput): Promise<ProjectEntry> {
  const response = await fetch('/api/projects/open', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
  if (!response.ok) {
    throw new Error(await responseError(response, 'open project directory failed'))
  }
  return (await response.json()) as ProjectEntry
}

export async function selectWorkspaceFolderFromDialog(): Promise<WorkspaceFolderInspection> {
  const response = await fetch('/api/workspace/folder/select', { method: 'POST' })
  if (!response.ok) {
    throw new Error(await responseError(response, 'select workspace folder failed'))
  }
  return (await response.json()) as WorkspaceFolderInspection
}

export async function openWorkspaceFolder(
  input: WorkspaceFolderOpenInput,
): Promise<WorkspaceFolderOpenResult> {
  const response = await fetch('/api/workspace/folder/open', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
  if (!response.ok) {
    throw new Error(await responseError(response, 'open workspace folder failed'))
  }
  return (await response.json()) as WorkspaceFolderOpenResult
}

export async function loadBlackboardData(project: string): Promise<BlackboardPayload> {
  const encoded = encodeURIComponent(project)
  return fetchJson<BlackboardPayload>(`/api/projects/${encoded}/tickets`)
}

/// Load the Markdown body content for a single ticket by ID. This is the
/// "content" half of the index/content split — the list endpoint returns
/// structural index fields only; call this when the user opens a detail view.
export async function loadTicketContent(project: string, id: string): Promise<string> {
  const encoded = encodeURIComponent(project)
  const safeId = encodeURIComponent(id)
  const payload = await fetchJson<{ content: string }>(
    `/api/projects/${encoded}/tickets/${safeId}/content`,
  )
  return payload.content
}

export async function patchTicket(
  project: string,
  id: string,
  patch: PatchTicketInput,
): Promise<TicketWriteResult> {
  const encoded = encodeURIComponent(project)
  const safeId = encodeURIComponent(id)
  const response = await fetch(`/api/projects/${encoded}/tickets/${safeId}`, {
    method: 'PATCH',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(patch),
  })
  if (!response.ok) {
    throw new Error(await responseError(response, 'patch_ticket failed'))
  }
  return (await response.json()) as TicketWriteResult
}

export async function deprecateTicket(
  project: string,
  id: string,
): Promise<DeprecateTicketResult> {
  const encoded = encodeURIComponent(project)
  const safeId = encodeURIComponent(id)
  const response = await fetch(`/api/projects/${encoded}/tickets/${safeId}/deprecate`, {
    method: 'POST',
  })
  if (!response.ok) {
    throw new Error(await responseError(response, 'deprecate_ticket failed'))
  }
  return (await response.json()) as DeprecateTicketResult
}

export async function loadInboxNotes(project: string): Promise<Loaded<InboxNoteEntry[]>> {
  const encoded = encodeURIComponent(project)
  const payload = await fetchJson<{ notes: InboxNoteEntry[] }>(
    `/api/projects/${encoded}/inbox/notes`,
  )
  return { data: payload.notes ?? [], source: 'rest' }
}

export async function loadInboxNote(
  project: string,
  name: string,
): Promise<Loaded<InboxNote | null>> {
  const encoded = encodeURIComponent(project)
  const safeName = encodeURIComponent(name)
  try {
    const note = await fetchJson<InboxNote>(
      `/api/projects/${encoded}/inbox/notes/${safeName}`,
    )
    return { data: note, source: 'rest' }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err)
    return { data: null, source: 'none', error: message }
  }
}

export async function loadBoardSummary(
  project: string,
): Promise<Loaded<BoardSummary | null>> {
  const encoded = encodeURIComponent(project)
  const summary = await fetchJson<BoardSummary>(
    `/api/projects/${encoded}/board/summary`,
  )
  return { data: summary, source: 'rest' }
}

export async function patchProjectBoardView(
  project: string,
  settings: ProjectBoardViewSettings,
): Promise<ProjectBoardViewSettings> {
  const encoded = encodeURIComponent(project)
  const response = await fetch(`/api/projects/${encoded}/board/view`, {
    method: 'PATCH',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(settings),
  })
  if (!response.ok) {
    throw new Error(await responseError(response, 'patch board view settings failed'))
  }
  return (await response.json()) as ProjectBoardViewSettings
}

export async function loadLanes(project: string): Promise<Loaded<LaneDef[]>> {
  const encoded = encodeURIComponent(project)
  const payload = await fetchJson<{ lanes: LaneDef[] }>(
    `/api/projects/${encoded}/lanes`,
  )
  return { data: payload.lanes ?? [], source: 'rest' }
}

/// Create-or-update a lane via the HTTP write API used by the web runtime.
export async function upsertLane(project: string, def: LaneDef): Promise<LaneDef> {
  const encoded = encodeURIComponent(project)
  const response = await fetch(`/api/projects/${encoded}/lanes`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(def),
  })
  if (!response.ok) {
    throw new Error(`upsert_lane failed: HTTP ${response.status}`)
  }
  return (await response.json()) as LaneDef
}

/// Patch an existing lane (partial update). All fields except the URL `id`
/// are optional.
export async function patchLane(
  project: string,
  id: string,
  patch: Partial<Pick<LaneDef, 'label' | 'color' | 'description' | 'status'>>,
): Promise<LaneDef> {
  const encoded = encodeURIComponent(project)
  const safeId = encodeURIComponent(id)
  const response = await fetch(`/api/projects/${encoded}/lanes/${safeId}`, {
    method: 'PATCH',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(patch),
  })
  if (!response.ok) {
    throw new Error(`patch_lane failed: HTTP ${response.status}`)
  }
  return (await response.json()) as LaneDef
}

/// Archive a lane. Existing tickets in this lane keep working (they appear in
/// `by_lane` but new tickets cannot be created). Returns the affected ticket
/// count so the UI can warn the user.
export async function archiveLane(
  project: string,
  id: string,
): Promise<{ lane: LaneDef; affected_ticket_count: number }> {
  const encoded = encodeURIComponent(project)
  const safeId = encodeURIComponent(id)
  const response = await fetch(
    `/api/projects/${encoded}/lanes/${safeId}/archive`,
    { method: 'POST' },
  )
  if (!response.ok) {
    throw new Error(`archive_lane failed: HTTP ${response.status}`)
  }
  return (await response.json()) as {
    lane: LaneDef
    affected_ticket_count: number
  }
}

/// Resolve display metadata for a lane id, preferring the live catalog and
/// using built-in visual metadata only for historical/missing lane ids.
export function resolveLaneMeta(
  laneId: string,
  catalog: LaneDef[],
): { label: string; color: string; description: string } {
  const live = catalog.find((lane) => lane.id === laneId)
  if (live) return { label: live.label, color: live.color, description: live.description }
  const builtin = BUILTIN_LANE_META[laneId]
  if (builtin) return builtin
  return { label: laneId, color: '#64748b', description: '' }
}

export function boardRoute(project: string): string {
  return `/projects/${project}`
}

export function ticketRoute(project: string, id: string): string {
  return `/projects/${project}/tickets/${id}`
}

export function attachmentsFromExtra(extra: Record<string, string>): TicketAttachment[] {
  const raw = extra.attachments
  if (!raw) return []
  try {
    const parsed = JSON.parse(raw) as unknown
    if (!Array.isArray(parsed)) return []
    return parsed
      .map((item) => {
        if (!item || typeof item !== 'object') return null
        const record = item as Record<string, unknown>
        const kind = typeof record.kind === 'string' ? record.kind.trim() : ''
        const target = typeof record.target === 'string' ? record.target.trim() : ''
        if (!kind || !target) return null
        const label = typeof record.label === 'string' ? record.label.trim() : ''
        const description =
          typeof record.description === 'string' ? record.description.trim() : ''
        return {
          kind,
          target,
          ...(label ? { label } : {}),
          ...(description ? { description } : {}),
        }
      })
      .filter((item): item is TicketAttachment => Boolean(item))
  } catch {
    return []
  }
}

/// Pull a short plain-text preview out of a ticket's markdown body for the
/// `# 当前进展` / (progress) section. This is the single source of truth for
/// "current" information after frontmatter.current was removed — both the
/// BoardView search bar and the TicketCard preview read this value so there
/// is only one concept of "current" to reason about.
export function extractProgressText(content: string): string {
  return extractSectionPreview(content, '当前进展')
}

function extractSectionPreview(content: string, heading: string): string {
  const lines = content.split(/\r?\n/)
  const headingIndex = lines.findIndex((line) => {
    const match = line.match(/^#{1,6}\s+(.+?)\s*$/)
    return match?.[1]?.trim() === heading
  })

  if (headingIndex === -1) return ''

  const sectionLines: string[] = []
  for (const line of lines.slice(headingIndex + 1)) {
    if (/^#{1,6}\s+/.test(line)) break
    const cleanedLine = line
      .trim()
      .replace(/^- \[[ xX]\]\s+/, '')
      .replace(/^[-*]\s+/, '')
    if (cleanedLine) sectionLines.push(cleanedLine)
  }

  const text = sectionLines
    .join(' ')
    .replace(/`([^`]+)`/g, '$1')
    .replace(/\*\*([^*]+)\*\*/g, '$1')
    .replace(/\[([^\]]+)]\([^)]+\)/g, '$1')
    .replace(/\s+/g, ' ')
    .trim()

  return text.length > 150 ? `${text.slice(0, 150)}...` : text
}
