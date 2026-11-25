import { defineAsyncComponent, type Component } from 'vue'
import { parseDate } from '@internationalized/date'
import catalog from './components/ui-catalog.json'
import NetworkGraph from './components/NetworkGraph.vue'
import BlockChart from './components/assistant/BlockChart.vue'
import type { WidgetUi } from './core/generated/WidgetUi'

const modules = import.meta.glob<Record<string, Component>>('./components/ui/*/index.ts')
const components = new Map<string, Component>([['Graph', BlockChart], ['NetworkGraph', NetworkGraph]])
export function resolveUi(name: string) {
  if (!Object.hasOwn(catalog, name)) return
  if (!components.has(name)) {
    const family = catalog[name as keyof typeof catalog]
    components.set(name, defineAsyncComponent(async () => (await modules[`./components/ui/${family}/index.ts`]())[name]))
  }
  return components.get(name)
}

// Only declarative data and presentation options can cross the document boundary.
const allowed = new Set(`
accept accessibleLabel activationMode activeStep align alignOffset allowNonContiguousRanges alt
asChild autocomplete avoidCollisions calendarLabel checked checkedValue code collapsedSize
collapsible collisionPadding cols count cursor defaultActiveStep defaultChecked defaultExpanded
defaultModelValue defaultOpen defaultPage defaultPlaceholder defaultPressed defaultSearchTerm
defaultSize defaultTabindex defaultValue delayDuration delayMount description dir direction
disableDaysOutsideCurrentView disabled errors fixedWeeks for forceMount glyph granularity height
hideCloseButton hideIcon hideIndicator hideLabel highlightOnHover highlightToday hourCycle href id
ignoreFilter index indicator indicatorPosition inputmode inset isInitialFocus items itemsPerPage
label labelKey language layout length linear loading locale loop matchBy max maxLength maxSize
maxValue maxVisible maxWidth maximumDays min minSize minStepsBetweenThumbs minValue modal mode
modelValue monthControls multiple name nameKey numberOfMonths open openOnClick openOnFocus options
orientation page pagedNavigation part pattern payload placeholder position pressed preventDeselect
progress progressValue readonly required resetSearchTermOnBlur resetSearchTermOnSelect role rows
searchTerm separator shortcut showCloseButton showEdges showTrigger siblingCount side sideOffset
sideWidth size skipDelayDuration src state status step sticky text textValue theme thumbAlignment
timeZone title total type uncheckedValue unmountOnClose unmountOnHide value variant verticalAlign
weekStartsOn width withHandle yearControls
`.trim().split(/\s+/))
type SlotScope = Record<string, unknown>

function scopedValue(value: unknown, scope: SlotScope): unknown {
  if (!value || typeof value !== 'object' || !('$slot' in value) || typeof value.$slot !== 'string') return value
  let current: unknown = scope
  for (const key of value.$slot.split('.')) {
    if (!current || typeof current !== 'object' || !Object.hasOwn(current, key)) return undefined
    current = (current as SlotScope)[key]
  }
  return current
}

export function uiProps(name: string, input: SlotScope = {}, scope: SlotScope = {}) {
  const values = Object.fromEntries(Object.entries(input).map(([key, value]) => [key, scopedValue(value, scope)]))
  const result = Object.fromEntries(Object.entries(values).filter(([key, value]) => {
    if (key === 'href' || key === 'src') return typeof value === 'string' && /^(https?:\/\/|\/(?!\/)|#)/i.test(value)
    return allowed.has(key) || /^aria-[a-z-]+$/.test(key)
  }))
  if (name === 'Calendar' || name === 'RangeCalendar') {
    for (const key of ['defaultValue', 'modelValue', 'minValue', 'maxValue', 'placeholder', 'defaultPlaceholder']) {
      const date = result[key]
      if (typeof date === 'string') result[key] = parseDate(date)
      else if (date && typeof date === 'object' && 'start' in date && 'end' in date) {
        result[key] = { start: parseDate(String(date.start)), end: parseDate(String(date.end)) }
      }
    }
  }
  if (name === 'NetworkGraph') {
    result.nodes = Array.isArray(values.nodes) ? values.nodes : []
    result.edges = Array.isArray(values.edges) ? values.edges : []
  }
  if (name.startsWith('Chart')) {
    const config = values.config
    result.config = Object.fromEntries(Object.entries(config && typeof config === 'object' ? config : {}).flatMap(([key, item]) => {
      if (!/^[a-zA-Z][a-zA-Z0-9_-]*$/.test(key) || !item || typeof item !== 'object') return []
      return [[key, { label: String(item.label ?? key), color: /^#[0-9a-f]{3,8}$/i.test(item.color) ? item.color : undefined }]]
    }))
    delete result.id
  }
  return result
}

export function parseUiDocument(value: WidgetUi): WidgetUi {
  if (!value.elements || typeof value.root !== 'string') throw Error('Invalid UI document')
  const seen = new Set<string>()
  function visit(id: string, depth: number) {
    const node = value.elements[id]
    if (!Object.hasOwn(value.elements, id) || !node || seen.has(id) || depth > 24 || seen.size >= 256)
      throw Error('Invalid UI component tree')
    seen.add(id)
    let groups: string[][] = []
    switch (node.component) {
      case 'shadcn':
        if (!Object.hasOwn(catalog, node.name)) throw Error('Unknown UI component ' + node.name)
        groups = Object.values(node.slots ?? {})
        break
      case 'section': groups = [node.header ?? [], node.body, node.actions ?? [], node.footer ?? []]; break
      case 'grid': case 'stack': case 'row': groups = [node.children]; break
      case 'badge': case 'text': case 'heading': case 'button': case 'data': break
      default: throw Error('Unknown component')
    }
    for (const group of groups) {
      if (!Array.isArray(group)) throw Error('Invalid UI slot')
      for (const child of group) visit(child, depth + 1)
    }
  }
  visit(value.root, 1)
  if (Object.keys(value.elements).length !== seen.size) throw Error('Unreachable UI component')
  return value
}
