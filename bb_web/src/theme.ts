import { computed, ref, watch } from 'vue'

export type ThemeMode = 'light' | 'dark'
export type ThemeSkinId = (typeof themeSkinPresets)[number]['id']

interface ThemeSkinTokens {
  primary: string
  onPrimary: string
}

export const themeSkinPresets = [
  {
    id: 'blackboard',
    labelKey: 'themeSkinBlackboard',
    badge: 'B',
    swatch: '#18181b',
    badgeTextColor: '#ffffff',
    tokens: {
      light: {
        primary: '#18181b',
        onPrimary: '#ffffff',
      },
      dark: {
        primary: '#f5f5f4',
        onPrimary: '#18181b',
      },
    } satisfies Record<ThemeMode, ThemeSkinTokens>,
  },
] as const

const THEME_STORAGE_KEY = 'blackboard.theme'
const SKIN_STORAGE_KEY = 'blackboard.themeSkin'

function initialTheme(): ThemeMode {
  if (typeof window === 'undefined') return 'light'
  const stored = window.localStorage.getItem(THEME_STORAGE_KEY)
  if (stored === 'light' || stored === 'dark') return stored
  return window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

function isThemeSkinId(value: string | null): value is ThemeSkinId {
  return themeSkinPresets.some((skin) => skin.id === value)
}

function initialSkin(): ThemeSkinId {
  if (typeof window === 'undefined') return 'blackboard'
  const stored = window.localStorage.getItem(SKIN_STORAGE_KEY)
  return isThemeSkinId(stored) ? stored : 'blackboard'
}

function skinPreset(id: ThemeSkinId) {
  return themeSkinPresets.find((skin) => skin.id === id) ?? themeSkinPresets[0]
}

function applyTheme(mode: ThemeMode, skinId: ThemeSkinId) {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  const skin = skinPreset(skinId)
  const tokens = skin.tokens[mode]
  root.dataset.theme = mode
  root.dataset.skin = skin.id
  root.setAttribute('theme-mode', mode)
  root.style.colorScheme = mode
  root.style.setProperty('--bb-theme-primary', tokens.primary)
  root.style.setProperty('--bb-theme-on-primary', tokens.onPrimary)
}

export const themeMode = ref<ThemeMode>(initialTheme())
export const themeSkin = ref<ThemeSkinId>(initialSkin())

watch(
  [themeMode, themeSkin],
  ([mode, skin]) => {
    applyTheme(mode, skin)
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(THEME_STORAGE_KEY, mode)
      window.localStorage.setItem(SKIN_STORAGE_KEY, skin)
    }
  },
  { immediate: true },
)

export const themeLabel = computed(() => (themeMode.value === 'dark' ? 'Dark' : 'Light'))
export const themeSkinPreset = computed(() => skinPreset(themeSkin.value))

export function toggleTheme() {
  themeMode.value = themeMode.value === 'dark' ? 'light' : 'dark'
}

export function setThemeSkin(id: ThemeSkinId) {
  themeSkin.value = id
}
