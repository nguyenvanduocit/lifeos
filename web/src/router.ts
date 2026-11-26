import { createRouter, createWebHistory } from 'vue-router'
import { carryLayers } from './nav'
import type { DomainKey } from './domains'

export const PAGES = [
  { path: '/', group: 'Today', name: 'Today', domain: 'overview', component: () => import('./pages/Overview.vue') },
  { path: '/inbox', group: 'Today', name: 'Inbox', domain: 'inbox', component: () => import('./pages/Inbox.vue') },
  { path: '/tasks', group: 'Life', name: 'Tasks', domain: 'tasks', component: () => import('./pages/Tasks.vue') },
  { path: '/goals', group: 'Life', name: 'Goals', domain: 'goals', component: () => import('./pages/Goals.vue') },
  { path: '/habits', group: 'Life', name: 'Habits', domain: 'habits', component: () => import('./pages/Habits.vue') },
  { path: '/people', group: 'Life', name: 'People', domain: 'people', component: () => import('./pages/People.vue') },
  { path: '/money', group: 'Life', name: 'Money', domain: 'money', component: () => import('./pages/Money.vue') },
  { path: '/health', group: 'Life', name: 'Health', domain: 'health', component: () => import('./pages/Health.vue') },
  { path: '/journal', group: 'Life', name: 'Journal', domain: 'journal', component: () => import('./pages/Journal.vue') },
  { path: '/notes', group: 'Life', name: 'Notes', domain: 'notes', fill: true, component: () => import('./pages/Notes.vue') },
  { path: '/crew', group: 'Agents', name: 'Crew', domain: 'crew', component: () => import('./pages/Crew.vue') },
  { path: '/talk', group: 'Agents', name: 'Team talk', domain: 'talk', component: () => import('./pages/Talk.vue') },
  { path: '/automation', group: 'Agents', name: 'Automation', domain: 'automation', component: () => import('./pages/Automation.vue') },
  { path: '/connections', group: 'Settings', name: 'Integrations', domain: 'integrations', component: () => import('./pages/Connections.vue') },
  { path: '/memory', group: 'Agents', name: 'Memory', domain: 'memory', component: () => import('./pages/Memory.vue') },
] as const satisfies readonly { path: string; group: string; name: string; domain: DomainKey; component: unknown; fill?: boolean }[]

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    // `fill`: the page takes the whole canvas and lays out its own panes (Notes), instead of the 1240px column (App.vue).
    ...PAGES.map((p) => ({ path: p.path, component: p.component, meta: { title: p.name, domain: p.domain, fill: 'fill' in p && p.fill } })),
    { path: '/plugins/:plugin/:slot', component: () => import('./pages/PluginPage.vue'), meta: { title: 'Plugin', domain: 'integrations' } },
    { path: '/plugins', redirect: '/connections/plugins' },
    { path: '/connections/plugins', component: () => import('./pages/Plugins.vue'), meta: { title: 'Plugins', domain: 'integrations' } },
    { path: '/connections/paypal', redirect: '/connections/plugins/paypal/settings' },
    { path: '/connections/plugins/:plugin/settings', component: () => import('./pages/PluginSettings.vue'), meta: { title: 'Plugin settings', domain: 'integrations' } },
    { path: '/connections/google', component: () => import('./pages/Google.vue'), meta: { title: 'Google', domain: 'integrations' } },
    { path: '/connections/desktop', component: () => import('./pages/Desktop.vue'), meta: { title: 'Desktop', domain: 'integrations' } },
    { path: '/connections/vault', component: () => import('./pages/Vault.vue'), meta: { title: 'Vault', domain: 'integrations' } },
    { path: '/connections/vault/:id', component: () => import('./pages/VaultItem.vue'), props: true, meta: { title: 'Vault', domain: 'integrations' } },
    { path: '/connections/accounts', component: () => import('./pages/Accounts.vue'), meta: { title: 'Accounts', domain: 'integrations' } },
    { path: '/tasks/:id', component: () => import('./pages/Task.vue'), props: true, meta: { title: 'Task', domain: 'tasks' } },
    { path: '/people/:id', component: () => import('./pages/Person.vue'), props: true, meta: { title: 'People', domain: 'people' } },
    { path: '/goals/:id', component: () => import('./pages/Goal.vue'), props: true, meta: { title: 'Goal', domain: 'goals' } },
    { path: '/habits/:name', component: () => import('./pages/Habit.vue'), props: true, meta: { title: 'Habit', domain: 'habits' } },
    { path: '/health/:metric', component: () => import('./pages/Metric.vue'), props: true, meta: { title: 'Health', domain: 'health' } },
    { path: '/journal/:day', component: () => import('./pages/JournalDay.vue'), props: true, meta: { title: 'Journal', domain: 'journal' } },
    { path: '/crew/:id', component: () => import('./pages/Member.vue'), props: true, meta: { title: 'Crew', domain: 'crew' } },
    { path: '/doc/:id(.*)', component: () => import('./pages/Doc.vue'), props: true, meta: { title: 'Document' } },
    { path: '/:rest(.*)', redirect: '/' },
  ],
})

router.afterEach((to) => {
  document.title = `${String(to.meta.title ?? '')} · LifeOS`
})

// The Assistant panel stays open across pages (nav.ts).
router.beforeEach((to, from) => carryLayers(to, from))
