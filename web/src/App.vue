<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useMediaQuery } from '@vueuse/core'
import { Compass, Menu, Sparkles } from '@lucide/vue'
import { useRoute } from 'vue-router'
import AppNavigation from './components/AppNavigation.vue'
import AssistantPanel from './components/AssistantPanel.vue'
import { Button } from './components/ui/button'
import { Sheet, SheetContent, SheetTitle, SheetTrigger } from './components/ui/sheet'
import { panel, setOpen } from './assistant'
import { linkState } from './core/link'

const route = useRoute()
const mobileNav = ref(false)
const desktopNav = useMediaQuery('(min-width: 48rem)')
watch(() => route.fullPath, () => { mobileNav.value = false })
watch(desktopNav, (desktop) => {
  if (desktop) mobileNav.value = false
})

const status = computed(() => ({
  open: 'Đã kết nối',
  connecting: 'Đang kết nối…',
  down: 'Mất kết nối, đang thử lại…',
}[linkState.status]))
</script>

<template>
  <div class="flex h-full overflow-hidden">
    <aside class="hidden w-48 shrink-0 flex-col border-r border-line bg-sidebar md:flex lg:w-[13.5rem]">
      <RouterLink to="/" class="flex h-[4.75rem] shrink-0 items-center gap-3 px-5 text-lg font-extrabold tracking-[-.045em]">
        <span class="grid size-9 place-items-center rounded-xl bg-primary text-primary-foreground">
          <Compass class="size-5" />
        </span>
        LifeOS
      </RouterLink>
      <div class="overflow-y-auto pt-1"><AppNavigation /></div>
    </aside>

    <div class="flex min-w-0 flex-1 flex-col">
      <header class="flex h-14 shrink-0 items-center gap-2 border-b border-line bg-panel px-4 sm:px-6 lg:gap-4 lg:px-8">
        <Sheet v-model:open="mobileNav">
          <SheetTrigger as-child>
            <Button variant="ghost" size="icon-sm" class="md:hidden" aria-label="Mở điều hướng">
              <Menu />
            </Button>
          </SheetTrigger>
          <SheetContent side="left" class="w-72 max-w-[85vw] gap-0 border-line bg-sidebar md:hidden" :aria-describedby="undefined">
            <SheetTitle class="flex h-16 shrink-0 items-center px-6 font-semibold">LifeOS</SheetTitle>
            <div class="overflow-y-auto pt-2"><AppNavigation @navigate="mobileNav = false" /></div>
          </SheetContent>
        </Sheet>
        <span class="truncate text-[13px] font-bold">{{ route.meta.title }}</span>
        <div
          role="status"
          class="ml-auto flex shrink-0 items-center gap-1.5 rounded-full border border-line px-2.5 py-1 text-xs text-ink2"
          :title="linkState.error || status"
        >
          <span class="size-2 shrink-0 rounded-full" :class="linkState.status === 'open' ? 'bg-good' : 'bg-warning'" />
          <span class="sr-only sm:not-sr-only">{{ status }}</span>
        </div>
        <Button size="sm" class="rounded-full" :variant="panel.open ? 'secondary' : 'outline'" :aria-expanded="panel.open" @click="setOpen(!panel.open)">
          <Sparkles class="text-primary" /> Assistant
        </Button>
      </header>

      <main class="min-h-0 flex-1 overflow-y-auto">
        <div class="mx-auto max-w-7xl px-4 py-5 sm:px-6 lg:px-8 lg:py-7">
          <RouterView />
        </div>
      </main>
    </div>

    <div v-if="panel.open" class="fixed inset-0 z-20 xl:static xl:z-auto xl:w-[360px] xl:shrink-0 2xl:w-[420px]">
      <AssistantPanel />
    </div>
  </div>
</template>
