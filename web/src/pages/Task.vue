<script setup lang="ts">
// A task on its own page: the trail back to Tasks, then the same detail the peek shows, in a reading column.
import { ChevronRight } from '@lucide/vue'
import { useRead } from '../data'
import TaskDetail from '../components/tasks/TaskDetail.vue'

const props = defineProps<{ id: string }>()
const { data } = useRead('read_doc', () => ({ id: `task_${props.id}` }))
</script>

<template>
  <nav aria-label="Breadcrumb" class="mb-6 flex items-center gap-1.5 text-[14px] text-ink2">
    <RouterLink to="/tasks" class="hover:text-ink">Tasks</RouterLink>
    <ChevronRight class="size-3.5 text-ink3" aria-hidden="true" />
    <span class="truncate text-ink" aria-current="page">{{ data?.summary.title ?? '…' }}</span>
  </nav>
  <div class="max-w-[1080px]">
    <TaskDetail :id="id" :key="id" page />
  </div>
</template>
