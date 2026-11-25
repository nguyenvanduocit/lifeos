import { createRouter, createWebHistory } from 'vue-router'

export const PAGES = [
  { path: '/', name: 'Tổng quan', icon: 'LayoutDashboard', component: () => import('./pages/Overview.vue') },
  { path: '/tasks', name: 'Việc', icon: 'ListChecks', component: () => import('./pages/Tasks.vue') },
  { path: '/goals', name: 'Mục tiêu', icon: 'Target', component: () => import('./pages/Goals.vue') },
  { path: '/habits', name: 'Thói quen', icon: 'Repeat', component: () => import('./pages/Habits.vue') },
  { path: '/people', name: 'Mối quan hệ', icon: 'Users', component: () => import('./pages/People.vue') },
  { path: '/money', name: 'Tài chính', icon: 'Wallet', component: () => import('./pages/Money.vue') },
  { path: '/health', name: 'Sức khoẻ', icon: 'HeartPulse', component: () => import('./pages/Health.vue') },
  { path: '/journal', name: 'Nhật ký', icon: 'NotebookPen', component: () => import('./pages/Journal.vue') },
  { path: '/notes', name: 'Ghi chú', icon: 'StickyNote', component: () => import('./pages/Notes.vue') },
  { path: '/inbox', name: 'Inbox', icon: 'Inbox', component: () => import('./pages/Inbox.vue') },
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
