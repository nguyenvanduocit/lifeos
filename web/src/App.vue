<script setup lang="ts">
import { computed, onMounted, provide, ref, watch } from 'vue'
import { useEventListener, useIntervalFn, useMediaQuery } from '@vueuse/core'
import { House, Inbox, ListChecks, Menu, Search, Sparkles, SquarePlus } from '@lucide/vue'
import { useRoute } from 'vue-router'
import AppNavigation from './components/AppNavigation.vue'
import CommandMenu from './components/CommandMenu.vue'
import Kbd from './components/ds/Kbd.vue'
import AskChip from './components/AskChip.vue'
import LifeOSLogo from './components/LifeOSLogo.vue'
import AssistantPanel from './components/AssistantPanel.vue'
import { Button } from './components/ui/button'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from './components/ui/resizable'
import { Sheet, SheetContent, SheetTitle, SheetTrigger } from './components/ui/sheet'
import { onOpen, panelOpen } from './assistant'
import { linkState, revision } from './core/link'
import { useAssistant } from './nav'
import { useRead } from './data'
import { loadPlugins } from './plugins'
import WidgetSlot from './components/WidgetSlot.vue'
import { pageWidgets } from './widgets'

onMounted(loadPlugins)
watch(revision, loadPlugins)
useEventListener(window, 'focus', loadPlugins)

const route = useRoute()
const pagePath = computed(() => route.path.replace(/\/+$/, '') || '/')
const { data: widgets } = useRead('list_widgets', () => ({ page: pagePath.value }))
provide(pageWidgets, widgets)

// One field for everything (CommandMenu): ⌘K anywhere, or `/` outside a box being typed in.
const commands = ref(false)
useEventListener(window, 'keydown', (e: KeyboardEvent) => {
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
    e.preventDefault()
    commands.value = !commands.value
  }
})

// How fresh the page is: the link's state, and when the server last said something changed.
const changedAt = ref(Date.now())
watch(revision, () => (changedAt.value = Date.now()))
const now = ref(Date.now())
useIntervalFn(() => (now.value = Date.now()), 15_000)
const freshness = computed(() => {
  if (linkState.status === 'connecting') return 'Connecting…'
  if (linkState.status === 'down') return 'Offline · retrying'
  const min = Math.floor((now.value - changedAt.value) / 60_000)
  return min < 1 ? 'Live · up to date' : `Live · updated ${min < 60 ? min + 'm' : Math.floor(min / 60) + 'h'} ago`
})
const mobileNav = ref(false)
const scrollArea = ref<HTMLElement>()
// The Assistant is open while the address says so (?assistant=1, nav.ts), as in remit: a column at the right of every page,
// 420px wide at first, which the owner drags between 320 and 720px; on a narrow screen, a sheet from the bottom.
const assistant = useAssistant()
watch(assistant.open, (open) => (panelOpen.value = open), { immediate: true })
onOpen((open) => (open ? assistant.show() : assistant.close()))
const assistantInline = useMediaQuery('(min-width: 64rem)')
const assistantColumn = { sizeUnit: 'px', defaultSize: 420, minSize: 320, maxSize: 720 } as const
function restoreAssistantFocus(event: Event) {
  event.preventDefault()
  document.querySelector<HTMLElement>('[data-assistant-toggle]')?.focus({ preventScroll: true })
}

// `g a` opens the Assistant from anywhere outside a box being typed in, as in remit.
let afterG = false
let chord: ReturnType<typeof setTimeout> | undefined
useEventListener(window, 'keydown', (e: KeyboardEvent) => {
  if (e.defaultPrevented || e.isComposing || e.metaKey || e.ctrlKey || e.altKey) return
  const t = e.target as HTMLElement | null
  if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))) return
  clearTimeout(chord)
  if (afterG && e.key === 'a') {
    afterG = false
    e.preventDefault()
    assistant.show()
    return
  }
  if (e.key === '/') {
    e.preventDefault()
    commands.value = true
    return
  }
  afterG = e.key === 'g'
  if (afterG) chord = setTimeout(() => (afterG = false), 1000)
})

const shortcuts = [
  { to: '/', label: 'Today', icon: House },
  { to: '/inbox', label: 'Inbox', icon: Inbox },
  { to: '/tasks', label: 'Tasks', icon: ListChecks },
]
const desktopNav = useMediaQuery('(min-width: 48rem)')
watch(() => route.path, () => {
  mobileNav.value = false
  if (scrollArea.value) scrollArea.value.scrollTop = 0
})
watch(desktopNav, (desktop) => {
  if (desktop) mobileNav.value = false
})
</script>

<template>
  <div class="flex h-full overflow-hidden bg-bg">
    <aside class="hidden w-[248px] shrink-0 flex-col md:flex">
      <RouterLink to="/" class="flex h-[72px] shrink-0 items-center px-6">
        <LifeOSLogo />
      </RouterLink>
      <div class="min-h-0 flex-1 overflow-y-auto pt-3 pb-4"><AppNavigation /></div>
      <div class="shrink-0 px-3 pb-4">
        <AppNavigation settings />
      </div>
    </aside>

    <div class="flex min-w-0 flex-1 flex-col md:py-2 md:pr-2">
      <div class="flex min-h-0 flex-1 flex-col overflow-hidden bg-panel md:rounded-2xl md:border md:border-line">
        <header class="flex min-h-12 shrink-0 items-center gap-1.5 border-b border-line px-3 max-sm:flex-wrap max-sm:gap-y-0 max-sm:pt-1 sm:px-4">
          <span class="max-w-24 truncate text-sm font-medium md:hidden">{{ route.meta.title }}</span>
          <Button type="button" variant="ghost" size="sm" class="rounded-lg text-[13px] text-ink2 max-xl:w-8 max-xl:has-[>svg]:px-0" aria-label="Ask LifeOS or jump to (⌘K)" title="Ask LifeOS or jump to (⌘K)" @click="commands = true">
            <Search class="size-3.5" aria-hidden="true" /><span class="hidden xl:inline">Search</span><Kbd class="hidden xl:inline-flex">⌘K</Kbd>
          </Button>
          <div role="status" class="flex shrink-0 items-center gap-2 text-xs text-ink2" :title="linkState.error || freshness">
            <span class="size-2 rounded-full" :class="linkState.status === 'open' ? 'bg-good' : 'bg-warning'" />
            <span class="max-xl:sr-only">{{ freshness }}</span>
          </div>
          <div id="page-actions" class="ml-auto flex min-w-0 items-center gap-1.5 overflow-x-auto whitespace-nowrap py-1 max-sm:order-last max-sm:basis-full max-sm:empty:hidden [&>*]:shrink-0" />
          <AskChip :icon="SquarePlus" label="Add widget" size="sm" icon-only class="max-sm:ml-auto"
            :words="`Create a custom widget on LifeOS page ${JSON.stringify(pagePath)}. Write its content about: `" />
          <Button type="button" variant="outline" size="icon-sm" class="rounded-lg border-line2 aria-expanded:bg-panel2" :aria-expanded="assistant.open.value" aria-label="Assistant" title="Assistant (g a)" data-assistant-toggle @click="assistant.toggle()">
            <Sparkles class="size-3.5" aria-hidden="true" />
          </Button>
        </header>

        <ResizablePanelGroup direction="horizontal" auto-save-id="lifeos-shell" class="min-h-0 flex-1">
          <ResizablePanel id="shell-view" :order="1" class="flex" :min-size="35">
            <main ref="scrollArea" class="min-h-0 min-w-0 flex-1 overflow-y-auto">
              <div class="@container" :class="route.meta.fill ? 'flex h-full min-h-0 flex-col' : 'mx-auto max-w-[1240px] px-4 py-6 sm:px-6 lg:px-9 lg:py-8'">
                <WidgetSlot v-if="pagePath !== '/'" :class="route.meta.fill ? 'shrink-0 border-b border-line px-4 py-4 sm:px-6 lg:px-9' : 'mb-5'" />
                <RouterView />
              </div>
            </main>
          </ResizablePanel>
          <template v-if="assistantInline && assistant.open.value">
            <ResizableHandle aria-label="Resize the Assistant" />
            <ResizablePanel id="shell-assistant" :order="2" class="flex" v-bind="assistantColumn"><AssistantPanel /></ResizablePanel>
          </template>
        </ResizablePanelGroup>
      </div>
      <nav aria-label="Shortcuts" class="grid shrink-0 grid-cols-4 gap-1 border-t border-line bg-panel px-2 pt-1 pb-[max(0.25rem,env(safe-area-inset-bottom))] md:hidden">
        <RouterLink v-for="item in shortcuts" :key="item.to" :to="item.to"
          class="flex min-h-11 flex-col items-center justify-center gap-1 rounded-xl py-1.5 text-[11px] font-medium text-ink3"
          exact-active-class="!text-ink">
          <component :is="item.icon" class="size-5" aria-hidden="true" />
          {{ item.label }}
        </RouterLink>
        <Sheet v-model:open="mobileNav">
          <SheetTrigger as-child>
            <Button variant="ghost" class="h-auto flex-col gap-1 rounded-xl py-1.5 text-[11px] font-medium text-ink3" aria-label="Open navigation">
              <Menu class="size-5" /> More
            </Button>
          </SheetTrigger>
          <SheetContent side="left" class="w-72 max-w-[85vw] gap-0 border-line bg-bg md:hidden" :aria-describedby="undefined">
            <SheetTitle class="flex h-16 shrink-0 items-center px-6">
              <LifeOSLogo />
            </SheetTitle>
            <div class="space-y-6 overflow-y-auto pt-2 pb-6">
              <AppNavigation @navigate="mobileNav = false" />
              <AppNavigation settings @navigate="mobileNav = false" />
            </div>
          </SheetContent>
        </Sheet>
      </nav>
    </div>

    <CommandMenu v-model:open="commands" />

    <Sheet v-if="!assistantInline" :open="assistant.open.value" @update:open="(o: boolean) => (o ? assistant.show() : assistant.close())">
      <SheetContent side="bottom" hide-close overlay-class="bg-black/35" :aria-describedby="undefined"
        class="h-[min(80dvh,44rem)] gap-0 overflow-hidden rounded-t-2xl border-line"
        @close-auto-focus="restoreAssistantFocus">
        <SheetTitle class="sr-only">Assistant</SheetTitle>
        <AssistantPanel />
      </SheetContent>
    </Sheet>
  </div>
</template>
