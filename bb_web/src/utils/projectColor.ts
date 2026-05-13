const projectPalette = [
  '#0f766e',
  '#2563eb',
  '#7c3aed',
  '#c2410c',
  '#be123c',
  '#047857',
  '#0369a1',
  '#9333ea',
  '#b45309',
  '#4f46e5',
  '#0e7490',
  '#a21caf',
]

export function projectColor(projectName: string) {
  const seed = projectName.trim().toLowerCase() || 'project'
  if (seed === 'blackboard') return 'var(--bb-project-blackboard-bg)'

  let hash = 2166136261

  for (let index = 0; index < seed.length; index += 1) {
    hash ^= seed.charCodeAt(index)
    hash = Math.imul(hash, 16777619)
  }

  return projectPalette[Math.abs(hash) % projectPalette.length]
}

export function projectBadgeTextColor(projectName: string) {
  const seed = projectName.trim().toLowerCase()
  return seed === 'blackboard' ? 'var(--bb-project-blackboard-fg)' : '#ffffff'
}
