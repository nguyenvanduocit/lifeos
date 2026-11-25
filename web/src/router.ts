import { createRouter, createWebHistory } from 'vue-router'

export const PAGES = [
  { path: '/', group: 'Kế hoạch', name: 'Tổng quan', icon: 'LayoutDashboard', component: () => import('./pages/Overview.vue') },
  { path: '/inbox', group: 'Kế hoạch', name: 'Inbox', icon: 'Inbox', component: () => import('./pages/Inbox.vue') },
  { path: '/tasks', group: 'Kế hoạch', name: 'Việc', icon: 'ListChecks', component: () => import('./pages/Tasks.vue') },
  { path: '/goals', group: 'Kế hoạch', name: 'Mục tiêu', icon: 'Target', component: () => import('./pages/Goals.vue') },
  { path: '/habits', group: 'Kế hoạch', name: 'Thói quen', icon: 'Repeat', component: () => import('./pages/Habits.vue') },
  { path: '/people', group: 'Cuộc sống', name: 'Mối quan hệ', icon: 'Users', component: () => import('./pages/People.vue') },
  { path: '/money', group: 'Cuộc sống', name: 'Tài chính', icon: 'Wallet', component: () => import('./pages/Money.vue') },
  { path: '/health', group: 'Cuộc sống', name: 'Sức khoẻ', icon: 'HeartPulse', component: () => import('./pages/Health.vue') },
  { path: '/journal', group: 'Cuộc sống', name: 'Nhật ký', icon: 'NotebookPen', component: () => import('./pages/Journal.vue') },
  { path: '/notes', group: 'Cuộc sống', name: 'Ghi chú', icon: 'StickyNote', component: () => import('./pages/Notes.vue') },
  { path: '/crew', group: 'Agent', name: 'Crew', icon: 'Bot', component: () => import('./pages/Crew.vue') },
  { path: '/memory', group: 'Agent', name: 'Memory', icon: 'Brain', component: () => import('./pages/Memory.vue') },
] as const

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    ...PAGES.map((p) => ({ path: p.path, component: p.component, meta: { title: p.name } })),
    { path: '/people/:id', component: () => import('./pages/Person.vue'), props: true, meta: { title: 'Mối quan hệ' } },
    { path: '/doc/:id(.*)', component: () => import('./pages/Doc.vue'), props: true, meta: { title: 'Tài liệu' } },
    { path: '/:rest(.*)', redirect: '/' },
  ],
})

router.afterEach((to) => {
  document.title = `${String(to.meta.title ?? '')} · lifeos`
})
