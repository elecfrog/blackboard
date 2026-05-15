<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { AlertCircle, AtSign, Bot, BrainCircuit, ChevronDown, Cpu, MessageSquare, RotateCcw, Wrench, X } from 'lucide-vue-next'
import { Button as TButton, Dropdown as TDropdown, Input as TInput, Popup as TPopup, Space as TSpace } from 'tdesign-vue-next'
import {
  ChatMessage as TChatMessage,
  ChatSender,
  type AIMessageContent,
  type ChatMessageRole,
  type ChatMessageStatus,
  type UserMessageContent,
} from '@tdesign-vue-next/chat'
import type { TdChatContentProps } from 'tdesign-web-components'
import {
  createAgentSession,
  readAgentSession,
  watchAgentSessionEvents,
  type AgentEvent,
  type AgentSession,
} from '@/data/agentSessions'
import { loadProjectAgents, type ProjectAgentProfile } from '@/data/agents'
import { t } from '@/i18n'

type ChatContent = AIMessageContent | UserMessageContent
type MarkdownContent = Extract<AIMessageContent, { type: 'markdown' }>
type TextContent = Extract<AIMessageContent, { type: 'text' }>
type ThinkingContent = Extract<AIMessageContent, { type: 'thinking' }>

interface ChatMessage {
  id: string
  role: ChatMessageRole
  content: ChatContent[]
  status: ChatMessageStatus
  sessionId?: string
  meta?: string
  tools: string[]
}

const route = useRoute()
const open = ref(false)
const draft = ref('')
const sending = ref(false)
const error = ref('')
const messages = ref<ChatMessage[]>([])
const projectAgents = ref<ProjectAgentProfile[]>([])
const agentsLoading = ref(false)
const agentsError = ref('')
const selectedAgent = ref('opencode')
const selectedModel = ref('')
const selectedVariant = ref('')
const modelPopupOpen = ref(false)
const currentProviderSessionId = ref('')
const scrollBody = ref<HTMLElement | null>(null)
const seenEvents = new Set<string>()
const stopStreams = new Map<string, () => void>()
let agentLoadToken = 0

const project = computed(() => {
  const value = route.params.project
  return typeof value === 'string' ? value : ''
})

const canSend = computed(() => Boolean(project.value && draft.value.trim() && !sending.value))
const chatSubtitle = computed(() =>
  currentProviderSessionId.value
    ? t('agentChatSubtitleThread', {
        project: project.value,
        session: shortSessionId(currentProviderSessionId.value),
      })
    : t('agentChatSubtitle', { project: project.value }),
)
const opencodeAgents = computed(() =>
  projectAgents.value.filter(
    (agent) => agent.status === 'active' && agent.assignable && agent.runtime === 'opencode',
  ),
)
const selectedProfile = computed(
  () => opencodeAgents.value.find((agent) => agent.id === selectedAgent.value) ?? null,
)
const modelOptions = computed(() =>
  uniqueStrings(opencodeAgents.value.map((agent) => agent.model).filter(Boolean) as string[]),
)
const variantOptions = computed(() =>
  uniqueStrings([
    ...(opencodeAgents.value.map((agent) => agent.variant).filter(Boolean) as string[]),
    'minimal',
    'low',
    'medium',
    'high',
    'xhigh',
    'max',
  ]),
)
const agentDropdownOptions = computed(() => {
  if (agentsLoading.value) {
    return [{ content: t('agentChatProfileLoading'), value: selectedAgent.value, disabled: true }]
  }
  if (!opencodeAgents.value.length) {
    return [{ content: t('agentChatProfileFallback'), value: 'opencode' }]
  }
  return opencodeAgents.value.map((agent) => ({
    content: agentLabel(agent),
    value: agent.id,
  }))
})
const variantDropdownOptions = computed(() => [
  { content: t('agentChatVariantPlaceholder'), value: '' },
  ...variantOptions.value.map((variant) => ({
    content: variant,
    value: variant,
  })),
])
const modelQuickOptions = computed(() =>
  uniqueStrings([
    selectedProfile.value?.model ?? '',
    selectedModel.value,
    ...modelOptions.value,
  ]),
)
const selectedAgentLabel = computed(() => {
  if (agentsLoading.value) return t('agentChatProfileLoading')
  const profile = selectedProfile.value
  if (profile) return profile.display_name || profile.id
  return selectedAgent.value || t('agentChatProfileFallback')
})
const selectedModelLabel = computed(() => selectedModel.value.trim() || t('agentChatModelPlaceholder'))
const selectedVariantLabel = computed(() => selectedVariant.value.trim() || t('agentChatVariantPlaceholder'))

watch(
  project,
  (value) => {
    restoreProviderSession(value)
    resetConversation({ clearProviderSession: false })
    void loadChatAgents(value)
  },
  { immediate: true },
)

watch(selectedAgent, () => {
  applySelectedAgentDefaults()
})

onBeforeUnmount(() => {
  stopAllStreams()
})

function toggleOpen() {
  open.value = !open.value
  if (open.value) void scrollToBottom()
}

function resetConversation(options: { clearProviderSession?: boolean } = {}) {
  stopAllStreams()
  seenEvents.clear()
  error.value = ''
  sending.value = false
  draft.value = ''
  if (options.clearProviderSession ?? true) {
    setProviderSessionId('')
  }
  messages.value = project.value
    ? [
        {
          id: 'welcome',
          role: 'assistant',
          content: [createMarkdownContent(t('agentChatEmpty', { project: project.value }))],
          status: 'complete',
          tools: [],
        },
      ]
    : []
}

function newConversation() {
  resetConversation({ clearProviderSession: true })
  void scrollToBottom()
}

function stopCurrentResponse() {
  stopAllStreams()
  finishActiveAssistantMessage('stop')
  sending.value = false
}

async function sendMessage(value?: string) {
  const prompt = (value ?? draft.value).trim()
  if (!prompt || sending.value || !project.value) return

  draft.value = ''
  error.value = ''
  sending.value = true

  messages.value.push({
    id: `user-${Date.now()}`,
    role: 'user',
    content: [createUserTextContent(prompt)],
    status: 'complete',
    tools: [],
  })

  const assistantMessage: ChatMessage = {
    id: `assistant-${Date.now()}`,
    role: 'assistant',
    content: [],
    status: 'pending',
    meta: t('agentChatStarting'),
    tools: [],
  }
  messages.value.push(assistantMessage)
  await scrollToBottom()

  try {
    const model = selectedModel.value.trim()
    const variant = selectedVariant.value.trim()
    const session = await createAgentSession(project.value, {
      prompt,
      runtime: 'opencode',
      agent: selectedAgent.value || 'opencode',
      ...(model ? { model } : {}),
      ...(variant ? { variant } : {}),
      ...(currentProviderSessionId.value
        ? { provider_session_id: currentProviderSessionId.value }
        : {}),
    })
    assistantMessage.sessionId = session.id
    assistantMessage.meta = chatSessionMeta(session)
    startSessionStream(session.id, assistantMessage)
  } catch (err) {
    const message = friendlyErrorMessage(err)
    setMessageError(assistantMessage, message)
    error.value = message
    sending.value = false
  } finally {
    await scrollToBottom()
  }
}

function startSessionStream(sessionId: string, message: ChatMessage) {
  stopStreams.get(sessionId)?.()
  const stop = watchAgentSessionEvents(
    project.value,
    sessionId,
    (events) => {
      applyEvents(sessionId, message, events)
      void scrollToBottom()
    },
    (streamError) => {
      const messageText = friendlyErrorText(streamError)
      if (!hasReadableContent(message)) setMessageError(message, messageText)
      else finishMessage(message, 'error')
      error.value = messageText
      sending.value = false
    },
  )
  stopStreams.set(sessionId, stop)
}

function applyEvents(sessionId: string, message: ChatMessage, events: AgentEvent[]) {
  for (const event of events) {
    const key = `${sessionId}:${event.seq}`
    if (seenEvents.has(key)) continue
    seenEvents.add(key)

    if (event.session_id) setProviderSessionId(event.session_id)

    if (event.type === 'text' && event.content) {
      appendAssistantText(message, event.content)
      message.status = 'streaming'
    } else if (event.type === 'thinking' && event.content) {
      appendAssistantThinking(message, event.content)
      message.status = 'streaming'
    } else if (event.type === 'tool_use') {
      const label = event.tool
        ? event.status
          ? `${event.tool} · ${event.status}`
          : event.tool
        : t('agentSessionToolUse')
      if (!message.tools.includes(label)) message.tools.push(label)
    } else if (event.type === 'tool_result') {
      const label = event.tool ? `${event.tool} · done` : t('agentSessionToolResult')
      if (!message.tools.includes(label)) message.tools.push(label)
    } else if (event.type === 'status' && event.status) {
      message.meta = chatEventMeta(sessionId, event.status)
    } else if (event.type === 'error') {
      const messageText = friendlyErrorText(event.content || t('agentSessionError'))
      setMessageError(message, messageText)
      error.value = messageText
      sending.value = false
    }
  }

  const hasTerminalError = events.some((event) => event.type === 'error')
  const hasUsage = events.some((event) => event.type === 'usage_update')
  if (hasTerminalError || (hasReadableContent(message) && hasUsage)) {
    if (!hasTerminalError) finishMessage(message, 'complete')
    sending.value = false
    if (!currentProviderSessionId.value) void refreshProviderSessionId(sessionId)
  }
}

function createMarkdownContent(data: string, status: ChatMessageStatus = 'complete'): MarkdownContent {
  return { type: 'markdown', data, status } as MarkdownContent
}

function createUserTextContent(data: string): UserMessageContent {
  return { type: 'text', data, status: 'complete' }
}

function createErrorContent(data: string): TextContent {
  return { type: 'text', data, status: 'error' } as TextContent
}

function createThinkingContent(data: string): ThinkingContent {
  return {
    type: 'thinking',
    data: {
      title: t('agentChatThinkingTitle'),
      text: data,
    },
    status: 'streaming',
  } as ThinkingContent
}

function appendAssistantText(message: ChatMessage, chunk: string) {
  const existing = message.content.find((item): item is MarkdownContent => item.type === 'markdown')
  if (existing) {
    existing.data += chunk
    existing.status = 'streaming'
  } else {
    message.content.push(createMarkdownContent(chunk, 'streaming'))
  }
  refreshMessageContent(message)
}

function appendAssistantThinking(message: ChatMessage, chunk: string) {
  const existing = message.content.find((item): item is ThinkingContent => item.type === 'thinking')
  if (existing) {
    existing.data = {
      ...existing.data,
      title: existing.data?.title || t('agentChatThinkingTitle'),
      text: `${existing.data?.text ?? ''}${chunk}`,
    }
    existing.status = 'streaming'
  } else {
    message.content.push(createThinkingContent(chunk))
  }
  refreshMessageContent(message)
}

function setMessageError(message: ChatMessage, messageText: string) {
  message.status = 'error'
  message.content = [createErrorContent(messageText)]
}

function finishMessage(message: ChatMessage, status: ChatMessageStatus) {
  message.status = status
  for (const content of message.content) {
    content.status = status
  }
  refreshMessageContent(message)
}

function finishActiveAssistantMessage(status: ChatMessageStatus) {
  const message = [...messages.value]
    .reverse()
    .find((item) => item.role === 'assistant' && (item.status === 'pending' || item.status === 'streaming'))
  if (message) finishMessage(message, status)
}

function hasReadableContent(message: ChatMessage) {
  return message.content.some((content) => {
    if (typeof content.data === 'string') return content.data.trim().length > 0
    if (content.type === 'thinking') return Boolean(content.data?.text?.trim())
    return Boolean(content.data)
  })
}

function chatMessageContent(message: ChatMessage) {
  return message.content as AIMessageContent[] | UserMessageContent[]
}

function chatContentProps(message: ChatMessage): TdChatContentProps {
  const collapseThinking = message.role === 'assistant' && message.status === 'complete'
  return {
    thinking: {
      layout: 'border' as const,
      animation: 'gradient' as const,
      collapsed: collapseThinking,
      maxHeight: 180,
    },
    reasoning: {
      layout: 'border' as const,
      animation: 'gradient' as const,
      collapsed: collapseThinking,
      maxHeight: 180,
    },
  }
}

function refreshMessageContent(message: ChatMessage) {
  message.content = [...message.content]
}

function stopAllStreams() {
  for (const stop of stopStreams.values()) stop()
  stopStreams.clear()
}

async function refreshProviderSessionId(sessionId: string) {
  if (!project.value) return
  try {
    const session = await readAgentSession(project.value, sessionId)
    if (session.provider_session_id) setProviderSessionId(session.provider_session_id)
  } catch {
    // The stream already produced the user-visible result; keep this best-effort.
  }
}

async function loadChatAgents(projectName: string) {
  const token = ++agentLoadToken
  projectAgents.value = []
  agentsError.value = ''
  if (!projectName) return

  agentsLoading.value = true
  try {
    const result = await loadProjectAgents(projectName)
    if (token !== agentLoadToken) return
    projectAgents.value = result.agents
    const preferred =
      opencodeAgents.value.find((agent) => agent.id === selectedAgent.value) ??
      opencodeAgents.value.find((agent) => agent.id === 'opencode') ??
      opencodeAgents.value[0]
    selectedAgent.value = preferred?.id ?? 'opencode'
    applySelectedAgentDefaults()
  } catch (err) {
    if (token !== agentLoadToken) return
    agentsError.value = t('agentChatProfileLoadFailed', { error: friendlyErrorMessage(err) })
    selectedAgent.value = 'opencode'
    selectedModel.value = ''
    selectedVariant.value = ''
  } finally {
    if (token === agentLoadToken) agentsLoading.value = false
  }
}

function applySelectedAgentDefaults() {
  const profile = selectedProfile.value
  selectedModel.value = profile?.model ?? ''
  selectedVariant.value = profile?.variant ?? ''
}

function agentLabel(agent: ProjectAgentProfile) {
  return agent.display_name ? `${agent.display_name} (${agent.id})` : agent.id
}

function chooseAgent(option: { value?: unknown }) {
  if (sending.value) return
  const value = typeof option.value === 'string' ? option.value : ''
  if (value) selectedAgent.value = value
}

function chooseVariant(option: { value?: unknown }) {
  if (sending.value) return
  selectedVariant.value = typeof option.value === 'string' ? option.value : ''
}

function chooseModel(model: string) {
  if (sending.value) return
  selectedModel.value = model
  modelPopupOpen.value = false
}

function chatSessionMeta(session: AgentSession) {
  if (session.provider_session_id) setProviderSessionId(session.provider_session_id)
  return [
    session.runtime,
    session.agent,
    session.model,
    session.variant,
    session.id,
  ].filter(Boolean).join(' · ')
}

function chatEventMeta(sessionId: string, status: string) {
  return [
    currentProviderSessionId.value
      ? `oc:${shortSessionId(currentProviderSessionId.value)}`
      : '',
    `as:${shortSessionId(sessionId)}`,
    status,
  ].filter(Boolean).join(' · ')
}

function restoreProviderSession(projectName: string) {
  currentProviderSessionId.value = ''
  if (typeof window === 'undefined' || !projectName) return
  currentProviderSessionId.value =
    window.localStorage.getItem(providerSessionStorageKey(projectName)) ?? ''
}

function setProviderSessionId(sessionId: string) {
  currentProviderSessionId.value = sessionId
  if (typeof window === 'undefined' || !project.value) return
  const key = providerSessionStorageKey(project.value)
  if (sessionId) {
    window.localStorage.setItem(key, sessionId)
  } else {
    window.localStorage.removeItem(key)
  }
}

function providerSessionStorageKey(projectName: string) {
  return `blackboard.agentChat.providerSession.${projectName}`
}

function shortSessionId(sessionId: string) {
  if (sessionId.length <= 14) return sessionId
  return sessionId.slice(0, 8)
}

function uniqueStrings(values: string[]) {
  return Array.from(new Set(values.map((value) => value.trim()).filter(Boolean)))
}

function friendlyErrorMessage(err: unknown) {
  return friendlyErrorText(err instanceof Error ? err.message : String(err))
}

function friendlyErrorText(message: string) {
  return message === 'Failed to fetch' || message.includes('NetworkError')
    ? t('agentChatNetworkError')
    : message
}

async function scrollToBottom() {
  await nextTick()
  const body = scrollBody.value
  if (body) body.scrollTop = body.scrollHeight
}
</script>

<template>
  <button
    v-if="project"
    type="button"
    class="bb-agent-chat-fab"
    :aria-label="t('agentChatOpen')"
    :aria-expanded="open"
    @click="toggleOpen"
  >
    <MessageSquare aria-hidden="true" />
  </button>

  <section v-if="project && open" class="bb-agent-chat-panel" :aria-label="t('agentChatTitle')">
    <header class="bb-agent-chat-head">
      <div class="bb-agent-chat-title">
        <span class="bb-agent-chat-mark" aria-hidden="true">
          <Bot />
        </span>
        <div>
          <h2>{{ t('agentChatTitle') }}</h2>
          <p>{{ chatSubtitle }}</p>
        </div>
      </div>
      <div class="bb-agent-chat-actions">
        <button type="button" :aria-label="t('agentChatNew')" @click="newConversation">
          <RotateCcw aria-hidden="true" />
        </button>
        <button type="button" :aria-label="t('close')" @click="open = false">
          <X aria-hidden="true" />
        </button>
      </div>
    </header>

    <div ref="scrollBody" class="bb-agent-chat-body">
      <TChatMessage
        v-for="message in messages"
        :key="message.id"
        class="bb-agent-chat-item"
        :role="message.role"
        :status="message.status"
        :content="chatMessageContent(message)"
        :name="message.role === 'user' ? t('agentChatYou') : t('agentChatAssistant')"
        :datetime="message.meta"
        :placement="message.role === 'user' ? 'right' : 'left'"
        :variant="message.role === 'user' ? 'base' : 'text'"
        :chat-content-props="chatContentProps(message)"
        animation="gradient"
      />
      <div v-if="messages.some((message) => message.tools.length > 0)" class="bb-agent-chat-tool-tray">
        <span v-for="tool in messages.flatMap((message) => message.tools)" :key="tool">
          <Wrench aria-hidden="true" />
          {{ tool }}
        </span>
      </div>
    </div>

    <p v-if="error" class="bb-agent-chat-error">
      <AlertCircle aria-hidden="true" />
      <span>{{ error }}</span>
    </p>

    <footer class="bb-agent-chat-footer">
      <ChatSender
        v-model="draft"
        :loading="sending"
        :send-btn-disabled="!canSend"
        :textarea-props="{ autofocus: open, autosize: { minRows: 2, maxRows: 6 } }"
        :placeholder="t('agentChatPlaceholder')"
        @send="sendMessage"
        @stop="stopCurrentResponse"
      >
        <template #inner-header>
          <div class="bb-agent-chat-reference-prefix">
            <AtSign aria-hidden="true" />
            <span>{{ t('agentChatReferenceEmpty') }}</span>
          </div>
        </template>

        <template #footer-prefix>
          <div class="bb-agent-chat-sender-prefix">
            <TSpace class="bb-agent-chat-toolbar" align="center" size="small" :break-line="true">
              <TDropdown
                :options="agentDropdownOptions"
                trigger="click"
                placement="top-left"
                :disabled="sending"
                :popup-props="{ overlayInnerClassName: 'bb-agent-chat-dropdown-popup' }"
                @click="chooseAgent"
              >
                <TButton
                  class="bb-agent-chat-control bb-agent-chat-control--agent"
                  shape="round"
                  variant="text"
                  :disabled="sending"
                  :title="t('agentChatProfileLabel')"
                >
                  <template #icon>
                    <Bot aria-hidden="true" />
                  </template>
                  <span>{{ selectedAgentLabel }}</span>
                  <ChevronDown aria-hidden="true" class="bb-agent-chat-control-chevron" />
                </TButton>
              </TDropdown>

              <TPopup
                v-model:visible="modelPopupOpen"
                trigger="click"
                placement="top-left"
                :disabled="sending"
                overlay-inner-class-name="bb-agent-chat-picker-popup"
              >
                <TButton
                  class="bb-agent-chat-control bb-agent-chat-control--model"
                  shape="round"
                  variant="text"
                  :disabled="sending"
                  :title="t('agentChatModelLabel')"
                >
                  <template #icon>
                    <Cpu aria-hidden="true" />
                  </template>
                  <span>{{ selectedModelLabel }}</span>
                  <ChevronDown aria-hidden="true" class="bb-agent-chat-control-chevron" />
                </TButton>

                <template #content>
                  <div class="bb-agent-chat-picker">
                    <label>
                      <span>{{ t('agentChatModelLabel') }}</span>
                      <TInput
                        v-model="selectedModel"
                        size="small"
                        clearable
                        :disabled="sending"
                        :placeholder="selectedProfile?.model || t('agentChatModelPlaceholder')"
                        @enter="modelPopupOpen = false"
                      />
                    </label>
                    <div v-if="modelQuickOptions.length" class="bb-agent-chat-picker-options">
                      <button
                        v-for="model in modelQuickOptions"
                        :key="model"
                        type="button"
                        :class="{ active: selectedModel === model }"
                        @click="chooseModel(model)"
                      >
                        {{ model }}
                      </button>
                    </div>
                  </div>
                </template>
              </TPopup>

              <TDropdown
                :options="variantDropdownOptions"
                trigger="click"
                placement="top-left"
                :disabled="sending"
                :popup-props="{ overlayInnerClassName: 'bb-agent-chat-dropdown-popup' }"
                @click="chooseVariant"
              >
                <TButton
                  class="bb-agent-chat-control bb-agent-chat-control--variant"
                  shape="round"
                  variant="text"
                  :disabled="sending"
                  :title="t('agentChatVariantLabel')"
                >
                  <template #icon>
                    <BrainCircuit aria-hidden="true" />
                  </template>
                  <span>{{ selectedVariantLabel }}</span>
                  <ChevronDown aria-hidden="true" class="bb-agent-chat-control-chevron" />
                </TButton>
              </TDropdown>
            </TSpace>
            <span v-if="agentsError" class="bb-agent-chat-toolbar-error">{{ agentsError }}</span>
          </div>
        </template>
      </ChatSender>
    </footer>
  </section>
</template>
