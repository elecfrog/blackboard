import {
  isTicketStatus,
  ticketStatusOrder,
  type TicketStatus,
} from '@/data/tickets'

const statusOrder = [...ticketStatusOrder]

export function visibleStatusesFromHidden(hiddenStatuses: readonly string[] = []) {
  const hidden = new Set(hiddenStatuses.filter(isTicketStatus))
  const visible = statusOrder.filter((status) => !hidden.has(status))
  return visible.length > 0 ? visible : [...statusOrder]
}

export function hiddenStatusesFromVisible(visibleStatuses: readonly TicketStatus[]) {
  const visible = new Set(visibleStatuses)
  return statusOrder.filter((status) => !visible.has(status))
}

export function dependenciesFromExtra(extra: Record<string, string>, selfId: string) {
  const raw = extra.depends_on || extra.dependencies || ''
  return raw
    .split(/[,\s]+/)
    .map((item) => item.trim())
    .filter((item, index, all) => item && item !== selfId && all.indexOf(item) === index)
}
