<script setup lang="ts">
import { t } from '@/i18n'
// Metrics are placeholder — backend API not yet available
interface MetricCard {
  labelKey: 'metricRuns' | 'metricSuccessRate' | 'metricAvgDuration' | 'metricTokensTotal'
  value: string
  trend: string
  trendUp: boolean
}

const metrics: MetricCard[] = [
  { labelKey: 'metricRuns', value: '—', trend: '', trendUp: false },
  { labelKey: 'metricSuccessRate', value: '—', trend: '', trendUp: false },
  { labelKey: 'metricAvgDuration', value: '—', trend: '', trendUp: false },
  { labelKey: 'metricTokensTotal', value: '—', trend: '', trendUp: false },
]
</script>

<template>
  <div class="aw-section">
    <div class="aw-section-header">
      <h4>{{ t('metricsSectionTitle') }}</h4>
    </div>
    <div class="aw-metrics-grid">
      <div v-for="m in metrics" :key="m.labelKey" class="aw-metric-card">
        <span class="aw-metric-label">{{ t(m.labelKey) }}</span>
        <span class="aw-metric-value">{{ m.value }}</span>
        <span v-if="m.trend" :class="['aw-metric-trend', m.trendUp ? 'up' : 'down']">
          {{ m.trendUp ? '↑' : '↓' }} {{ m.trend }} {{ t('metricVsPrev7Days') }}
        </span>
      </div>
    </div>
    <div class="aw-metrics-footer">
      <a href="#" class="aw-link">{{ t('metricsViewAnalytics') }}</a>
    </div>
  </div>
</template>
