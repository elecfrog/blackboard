<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  loadRuntimeConnectors,
  patchRuntimeConnectors,
  type PiShellPathSource,
  type RuntimeConnector,
  type RuntimeConnectorList,
  type RuntimeConnectorStatus,
} from '@/data/runtimeConnectors'
import { BbActionGroup, BbButton, BbInfoGrid, BbInfoItem } from '@/components/common'
import { t } from '@/i18n'

const list = ref<RuntimeConnectorList | null>(null)
const loading = ref(false)
const saving = ref(false)
const error = ref('')
const savedMessage = ref('')

const commandDrafts = ref<Record<string, string>>({})
const concurrencyDrafts = ref<Record<string, number>>({})
const nodeTimeoutDraft = ref(900)
const runTimeoutDraft = ref(1800)
const piShellPathDraft = ref('')

const runtimes = computed(() => list.value?.runtimes ?? [])
const piRuntime = computed(() => list.value?.pi ?? null)
const connectedCount = computed(
  () => runtimes.value.filter((runtime) => runtime.status === 'connected').length,
)

onMounted(() => {
  void refresh()
})

async function refresh() {
  loading.value = true
  error.value = ''
  savedMessage.value = ''
  try {
    const next = await loadRuntimeConnectors()
    applyRuntimeList(next)
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

function applyRuntimeList(next: RuntimeConnectorList) {
  list.value = next
  commandDrafts.value = Object.fromEntries(
    next.runtimes.map((runtime) => [runtime.id, runtime.command]),
  )
  concurrencyDrafts.value = Object.fromEntries(
    next.runtimes.map((runtime) => [runtime.id, runtime.max_concurrency ?? 0]),
  )
  nodeTimeoutDraft.value = next.node_timeout_secs
  runTimeoutDraft.value = next.run_timeout_secs
  piShellPathDraft.value = next.pi.configured_shell_path ?? next.pi.recommended_shell_path ?? ''
}

function collectConcurrency() {
  const values: Record<string, number> = {}
  for (const runtime of runtimes.value) {
    const raw = Number(concurrencyDrafts.value[runtime.id] ?? 0)
    values[runtime.id] = Number.isFinite(raw) ? Math.max(0, Math.floor(raw)) : 0
  }
  return values
}

async function saveSettings(message?: string) {
  const defaultMessage = t('runtimeConnectorSaved')
  if (!list.value || saving.value) return
  saving.value = true
  error.value = ''
  savedMessage.value = ''
  try {
    const next = await patchRuntimeConnectors({
      commands: { ...commandDrafts.value },
      runtime_max_concurrency: collectConcurrency(),
      node_timeout_secs: Math.max(1, Math.floor(Number(nodeTimeoutDraft.value) || 1)),
      run_timeout_secs: Math.max(1, Math.floor(Number(runTimeoutDraft.value) || 1)),
      pi: {
        shell_path: piShellPathDraft.value.trim(),
      },
    })
    applyRuntimeList(next)
    savedMessage.value = message ?? defaultMessage
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    saving.value = false
  }
}

async function applyGitBashWorkaround() {
  const recommended = piRuntime.value?.recommended_shell_path
  if (!recommended) return
  piShellPathDraft.value = recommended
  await saveSettings(t('runtimeConnectorShellSolidifyGitBash'))
}

function statusLabel(status: RuntimeConnectorStatus) {
  if (status === 'connected') return t('runtimeStatusConnected')
  if (status === 'missing') return t('runtimeStatusMissing')
  return t('runtimeStatusError')
}

function statusTone(status: RuntimeConnectorStatus) {
  if (status === 'connected') return 'ok'
  if (status === 'missing') return 'missing'
  return 'warn'
}

function runtimeDescription(runtime: RuntimeConnector) {
  if (runtime.id === 'codex') return t('runtimeCodexDescription')
  if (runtime.id === 'codebuddy') return t('runtimeCodebuddyDescription')
  if (runtime.id === 'opencode') return t('runtimeOpencodeDescription')
  if (runtime.id === 'pi') return t('runtimePiDescription')
  return t('runtimeGenericDescription')
}

function shellSourceLabel(source: PiShellPathSource) {
  if (source === 'settings') return 'settings.json'
  if (source === 'git_bash_default') return t('runtimeShellGitBashDefault')
  if (source === 'path') return t('runtimeShellPath')
  return t('runtimeShellNotFound')
}

function isPi(runtime: RuntimeConnector) {
  return runtime.id === 'pi'
}
</script>

<template>
  <section class="runtime-panel">
    <header class="runtime-panel-header">
      <div class="runtime-panel-title">
        <span class="runtime-eyebrow">{{ t('runtimeConnectorTitle') }}</span>
        <h2>{{ t('runtimeConnectors') }}</h2>
        <p>{{ t('runtimeConnectorDesc') }}</p>
      </div>
      <BbActionGroup class="runtime-panel-actions">
        <BbButton variant="secondary" :disabled="loading || saving" @click="refresh">
          {{ t('refresh') }}
        </BbButton>
        <BbButton
          variant="primary"
          :disabled="loading || saving || !list"
          @click="saveSettings()"
        >
          {{ saving ? t('runtimeSaving') : t('runtimeSaveSettings') }}
        </BbButton>
      </BbActionGroup>
    </header>

    <p v-if="loading" class="runtime-banner runtime-banner-info">{{ t('runtimeCheckingLocal') }}</p>
    <p v-if="error" class="runtime-banner runtime-banner-error">
      {{ error }}
    </p>
    <p v-if="savedMessage" class="runtime-banner runtime-banner-ok">{{ savedMessage }}</p>

    <template v-if="list">
      <section class="runtime-summary">
        <article>
          <span>{{ t('runtimeConnectionStatus') }}</span>
          <strong>{{ connectedCount }} / {{ runtimes.length }}</strong>
        </article>
        <article>
          <span>{{ t('runtimeNodeTimeout') }}</span>
          <strong>{{ list.node_timeout_secs }}s</strong>
        </article>
        <article>
          <span>{{ t('runtimeRunTimeout') }}</span>
          <strong>{{ list.run_timeout_secs }}s</strong>
        </article>
        <article>
          <span>{{ t('runtimeConfigFile') }}</span>
          <strong class="runtime-path">{{ list.config_path }}</strong>
        </article>
      </section>

      <section class="runtime-grid" :aria-label="t('runtimeConnectors')">
        <article
          v-for="runtime in runtimes"
          :key="runtime.id"
          class="runtime-card"
          :data-status="runtime.status"
        >
          <header class="runtime-card-header">
            <div>
              <p class="runtime-card-kicker">{{ runtime.id }}</p>
              <h3>{{ runtime.display_name }}</h3>
              <p>{{ runtimeDescription(runtime) }}</p>
            </div>
            <span class="runtime-status" :data-tone="statusTone(runtime.status)">
              {{ statusLabel(runtime.status) }}
            </span>
          </header>

          <BbInfoGrid class="runtime-card-facts" columns="repeat(2, minmax(0, 1fr))">
            <BbInfoItem
              :label="t('runtimeVersion')"
              :value="runtime.version || '-'"
              variant="mono"
              overflow="truncate"
            />
            <BbInfoItem
              :label="t('runtimeConcurrency')"
              :value="concurrencyDrafts[runtime.id] || 0"
              variant="mono"
              overflow="truncate"
            />
          </BbInfoGrid>

          <div class="runtime-card-form">
            <label class="runtime-field">
              <span>{{ t('runtimeCommand') }}</span>
              <input v-model="commandDrafts[runtime.id]" spellcheck="false" />
            </label>
            <label class="runtime-field runtime-field-small">
              <span>{{ t('runtimeMaxConcurrency') }}</span>
              <input
                v-model.number="concurrencyDrafts[runtime.id]"
                type="number"
                min="0"
                step="1"
              />
            </label>
          </div>

          <section v-if="isPi(runtime) && piRuntime" class="runtime-platform-block">
            <div class="runtime-platform-head">
              <div>
                <h4>{{ t('runtimeWindowsShell') }}</h4>
                <p>{{ t('runtimeWindowsShellDesc') }}</p>
              </div>
              <span
                class="runtime-status"
                :data-tone="piRuntime.shell_path_exists && !piRuntime.workaround_required ? 'ok' : 'warn'"
              >
                {{ piRuntime.shell_path_exists ? t('runtimeShellAvailable') : t('runtimeShellMissing') }}
              </span>
            </div>

            <ol class="shell-chain" :aria-label="t('runtimeShellResolutionOrder')">
              <li :class="{ active: piRuntime.shell_path_source === 'settings' }">{{ t('runtimeShellSettingsJson') }}</li>
              <li :class="{ active: piRuntime.shell_path_source === 'git_bash_default' }">
                {{ t('runtimeShellGitBashDefault') }}
              </li>
              <li :class="{ active: piRuntime.shell_path_source === 'path' }">{{ t('runtimeShellPathBashExe') }}</li>
            </ol>

            <div class="runtime-card-form">
              <label class="runtime-field">
                <span>{{ t('runtimeShellPathLabel') }}</span>
                <input v-model="piShellPathDraft" spellcheck="false" />
              </label>
              <BbButton
                class="runtime-card-action"
                variant="primary"
                :disabled="saving || !piRuntime.recommended_shell_path"
                @click="applyGitBashWorkaround"
              >
                {{ t('runtimeSolidifyGitBash') }}
              </BbButton>
            </div>

            <BbInfoGrid class="runtime-mini-meta" columns="repeat(2, minmax(0, 1fr))">
              <BbInfoItem
                :label="t('runtimeSource')"
                :value="shellSourceLabel(piRuntime.shell_path_source)"
                variant="mono"
                overflow="truncate"
              />
              <BbInfoItem :label="t('runtimeModel')" variant="mono" overflow="truncate">
                {{ piRuntime.default_provider || '-' }} /
                {{ piRuntime.default_model || '-' }}
              </BbInfoItem>
            </BbInfoGrid>
          </section>

          <details class="runtime-diagnostics">
            <summary>{{ t('runtimeDiagnostics') }}</summary>
            <BbInfoGrid class="runtime-meta" variant="rows">
              <BbInfoItem :label="t('runtimeActualLaunch')" :value="runtime.resolved_program" variant="mono" />
              <BbInfoItem
                v-if="runtime.resolved_args.length"
                :label="t('runtimeLaunchArgs')"
                :value="runtime.resolved_args.join(' ')"
                variant="mono"
              />
              <BbInfoItem v-if="runtime.error" :label="t('runtimeError')" :value="runtime.error" variant="mono" />
              <template v-if="isPi(runtime) && piRuntime">
                <BbInfoItem :label="t('runtimePiSettings')" :value="piRuntime.settings_path || '-'" variant="mono" />
                <BbInfoItem
                  :label="t('runtimeEffectiveShell')"
                  :value="piRuntime.effective_shell_path || '-'"
                  variant="mono"
                />
                <BbInfoItem
                  :label="t('runtimeRecommendedGitBash')"
                  :value="piRuntime.recommended_shell_path || t('runtimeRecommendedGitBashNotFound')"
                  variant="mono"
                />
                <BbInfoItem
                  v-if="piRuntime.settings_error"
                  :label="t('runtimeConfigError')"
                  :value="piRuntime.settings_error"
                  variant="mono"
                />
              </template>
            </BbInfoGrid>
          </details>
        </article>
      </section>

      <details class="runtime-advanced">
        <summary>{{ t('runtimeAdvancedSettings') }}</summary>
        <section class="runtime-advanced-body">
          <label class="runtime-field">
            <span>{{ t('runtimeNodeTimeoutSeconds') }}</span>
            <input v-model.number="nodeTimeoutDraft" type="number" min="1" step="1" />
          </label>
          <label class="runtime-field">
            <span>{{ t('runtimeRunTimeoutSeconds') }}</span>
            <input v-model.number="runTimeoutDraft" type="number" min="1" step="1" />
          </label>
          <div class="runtime-config-path">
            <span>{{ t('runtimeRunnerConfig') }}</span>
            <code>{{ list.config_path }}</code>
          </div>
        </section>
      </details>
    </template>
  </section>
</template>

<style scoped>
.runtime-panel {
  --runtime-accent: var(--bb-theme-primary);
  --runtime-accent-soft: var(--bb-theme-primary-soft);
  --runtime-accent-border: var(--bb-theme-primary-border);
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 24px;
  border: 1px solid var(--runtime-accent-border);
  border-radius: 16px;
  background:
    linear-gradient(135deg, color-mix(in srgb, var(--runtime-accent) 8%, transparent), transparent 34%),
    var(--bb-surface-soft);
  box-shadow: 0 16px 40px color-mix(in srgb, var(--runtime-accent) 7%, transparent);
}

.runtime-panel-header {
  display: flex;
  justify-content: space-between;
  gap: 16px;
}

.runtime-panel-title {
  max-width: 760px;
}

.runtime-eyebrow,
.runtime-card-kicker {
  display: inline-flex;
  margin: 0 0 6px;
  color: var(--runtime-accent);
  font-size: 11px;
  font-weight: 800;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.runtime-panel h2,
.runtime-panel h3,
.runtime-panel h4 {
  margin: 0;
  color: var(--bb-text-strong);
}

.runtime-panel h2 {
  font-size: 20px;
}

.runtime-panel h3 {
  font-size: 17px;
}

.runtime-panel h4 {
  font-size: 13px;
}

.runtime-panel p {
  margin: 4px 0 0;
  color: var(--bb-text-muted);
  font-size: 13px;
  line-height: 1.5;
}

.runtime-banner {
  margin: 0;
  padding: 10px 14px;
  border-radius: 10px;
  font-size: 13px;
}

.runtime-banner-info {
  color: var(--runtime-accent);
  background: var(--runtime-accent-soft);
}

.runtime-banner-ok {
  color: var(--bb-success);
  background: color-mix(in srgb, var(--bb-success) 12%, var(--bb-surface));
}

.runtime-banner-error {
  color: var(--bb-error);
  background: var(--bb-md-error-bg);
  border: 1px solid var(--bb-md-error-border);
}

.runtime-summary {
  display: grid;
  grid-template-columns: minmax(120px, 0.8fr) minmax(120px, 0.7fr) minmax(120px, 0.7fr) minmax(240px, 2fr);
  gap: 10px;
}

.runtime-summary article {
  min-width: 0;
  padding: 12px 14px;
  border: 1px solid var(--bb-hairline);
  border-radius: 12px;
  background: color-mix(in srgb, var(--bb-surface) 92%, var(--runtime-accent-soft));
}

.runtime-summary span {
  display: block;
  margin-bottom: 4px;
  color: var(--bb-text-muted);
  font-size: 12px;
}

.runtime-summary strong {
  display: block;
  min-width: 0;
  color: var(--bb-text-strong);
  font-size: 18px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.runtime-summary .runtime-path {
  font: 12px var(--bb-font-mono);
}

.runtime-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(320px, 1fr));
  gap: 14px;
}

.runtime-card {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-width: 0;
  padding: 16px;
  border: 1px solid var(--bb-hairline);
  border-radius: 14px;
  background: var(--bb-surface);
}

.runtime-card[data-status='connected'] {
  border-color: color-mix(in srgb, var(--bb-success) 22%, var(--bb-hairline));
}

.runtime-card-header,
.runtime-platform-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.runtime-status {
  flex: 0 0 auto;
  padding: 4px 9px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 800;
}

.runtime-status[data-tone='ok'] {
  color: var(--bb-success);
  background: color-mix(in srgb, var(--bb-success) 12%, var(--bb-surface));
}

.runtime-status[data-tone='warn'] {
  color: var(--bb-warning);
  background: color-mix(in srgb, var(--bb-warning) 12%, var(--bb-surface));
}

.runtime-status[data-tone='missing'] {
  color: var(--bb-text-muted);
  background: var(--bb-surface-muted);
}

.runtime-card-facts,
.runtime-mini-meta {
  --bb-info-grid-gap: 8px;
  --bb-info-item-padding: 10px;
  --bb-info-item-border: 1px solid var(--bb-hairline);
  --bb-info-item-radius: 10px;
}

.runtime-card-form,
.runtime-advanced-body {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 128px;
  gap: 10px;
  align-items: end;
}

.runtime-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.runtime-field span,
.runtime-config-path span {
  color: var(--bb-text-muted);
  font-size: 12px;
}

.runtime-field input {
  width: 100%;
  min-width: 0;
  padding: 8px 10px;
  border: 1px solid var(--bb-hairline);
  border-radius: 10px;
  color: var(--bb-text-strong);
  background: var(--bb-surface-soft);
  font: 13px var(--bb-font-mono);
}

.runtime-field input:focus {
  outline: 2px solid color-mix(in srgb, var(--runtime-accent) 24%, transparent);
  border-color: var(--runtime-accent-border);
}

.runtime-field-small input {
  text-align: center;
}

.runtime-platform-block {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px;
  border: 1px solid color-mix(in srgb, var(--runtime-accent) 24%, var(--bb-hairline));
  border-radius: 12px;
  background:
    linear-gradient(135deg, color-mix(in srgb, var(--runtime-accent) 7%, transparent), transparent 70%),
    var(--bb-surface-soft);
}

.shell-chain {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.shell-chain li {
  min-width: 0;
  padding: 7px 8px;
  border: 1px solid var(--bb-hairline);
  border-radius: 999px;
  color: var(--bb-text-muted);
  background: var(--bb-surface);
  font-size: 11px;
  font-weight: 700;
  text-align: center;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shell-chain li.active {
  color: var(--bb-theme-on-primary);
  background: var(--runtime-accent);
  border-color: var(--runtime-accent);
}

.runtime-card-action {
  border-radius: 10px;
}

.runtime-diagnostics,
.runtime-advanced {
  border-top: 1px solid var(--bb-hairline);
  padding-top: 10px;
}

.runtime-diagnostics summary,
.runtime-advanced summary {
  color: var(--bb-text-muted);
  font-size: 12px;
  font-weight: 800;
  cursor: pointer;
}

.runtime-meta {
  margin: 10px 0 0;
  --bb-info-row-gap: 8px;
  --bb-info-row-label-width: 80px;
  --bb-info-row-gap-x: 10px;
  --bb-info-label-font-size: 12px;
  --bb-info-value-font-weight: 650;
}

.runtime-advanced {
  padding: 14px 16px;
  border: 1px solid var(--bb-hairline);
  border-radius: 12px;
  background: var(--bb-surface);
}

.runtime-advanced-body {
  grid-template-columns: repeat(2, minmax(140px, 180px)) minmax(0, 1fr);
  margin-top: 12px;
}

.runtime-config-path {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.runtime-config-path code {
  min-width: 0;
  padding: 8px 10px;
  border: 1px solid var(--bb-hairline);
  border-radius: 10px;
  color: var(--bb-text-strong);
  background: var(--bb-surface-soft);
  font: 12px var(--bb-font-mono);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 1100px) {
  .runtime-summary,
  .runtime-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 760px) {
  .runtime-panel-header,
  .runtime-card-header,
  .runtime-platform-head {
    flex-direction: column;
    align-items: stretch;
  }

  .runtime-panel-actions {
    align-self: stretch;
    justify-content: flex-start;
  }

  .runtime-card-form,
  .runtime-advanced-body,
  .runtime-card-facts,
  .runtime-mini-meta,
  .shell-chain {
    grid-template-columns: 1fr;
  }
}
</style>
