// The panel's place in the address, as remit keeps it (nav.ts there): ?assistant=1 while the Assistant panel is open, and
// ?assistantPanel= the tab it shows. A move to another page keeps them, so the panel stays open across pages.
import { computed, type WritableComputedRef } from 'vue'
import { useRoute, useRouter, type LocationQuery, type RouteLocationNormalizedGeneric, type RouteLocationRaw } from 'vue-router'

export function useQueryRef<T extends string>(key: string, fallback: T, valid?: readonly T[]): WritableComputedRef<T> {
  const route = useRoute()
  const router = useRouter()
  return computed({
    get: () => {
      const v = route.query[key]
      return (typeof v === 'string' && (!valid || (valid as readonly string[]).includes(v)) ? v : fallback) as T
    },
    set: (v) => {
      void router.replace({ query: { ...route.query, [key]: v === fallback ? undefined : v } })
    },
  })
}

export function useAssistant() {
  const route = useRoute()
  const router = useRouter()
  const open = computed(() => route.query.assistant !== undefined)
  const show = () => void router.push({ query: { ...route.query, assistant: '1' } })
  const close = () => void router.replace({ query: { ...route.query, assistant: undefined, assistantPanel: undefined } })
  return { open, show, close, toggle: () => (open.value ? close() : show()) }
}

// The layers that stay open over every page (the query keys that open them, and that say what they show).
const LAYERS = ['assistant', 'assistantPanel'] as const
type At = Pick<RouteLocationNormalizedGeneric, 'path' | 'query' | 'hash'>
export function carryLayers(to: At, from: At): RouteLocationRaw | undefined {
  if (to.path === from.path) return undefined
  const carried: LocationQuery = {}
  for (const key of LAYERS) if (from.query[key] !== undefined && to.query[key] === undefined) carried[key] = from.query[key]
  if (!Object.keys(carried).length) return undefined
  return { path: to.path, query: { ...to.query, ...carried }, hash: to.hash }
}
