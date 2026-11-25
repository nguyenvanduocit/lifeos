// Each area of the owner's life once: its name, its icon and its hue (style.css, --t-<key>).
// The navigation, every page's head and the cards that speak for an area read them here.
import type { Component } from 'vue'
import {
  Bot, Brain, CalendarClock, MessagesSquare, HeartPulse, House, Inbox, ListChecks, NotebookPen, Plug, Repeat, StickyNote, Target, Users, Wallet,
} from '@lucide/vue'

export interface Domain {
  name: string
  icon: Component
  /** One line under the page title. */
  tagline: string
}

export const DOMAINS = {
  overview: { name: 'Today', icon: House, tagline: 'Your day at a glance.' },
  inbox: { name: 'Inbox', icon: Inbox, tagline: 'Everything captured, waiting to become a task, a note or nothing.' },
  tasks: { name: 'Tasks', icon: ListChecks, tagline: 'What is late, what is today, and what comes next.' },
  goals: { name: 'Goals', icon: Target, tagline: 'Where you are heading, and the tasks that move you there.' },
  habits: { name: 'Habits', icon: Repeat, tagline: 'Small things, done often. Twelve weeks at a glance.' },
  people: { name: 'People', icon: Users, tagline: 'The people who matter, birthdays, and when to reach out.' },
  money: { name: 'Money', icon: Wallet, tagline: 'Where the month’s money went, and how budgets hold.' },
  health: { name: 'Health', icon: HeartPulse, tagline: 'Your body’s numbers over time.' },
  journal: { name: 'Journal', icon: NotebookPen, tagline: 'A page a day, in your own words.' },
  notes: { name: 'Notes', icon: StickyNote, tagline: 'Ideas, reading notes, briefs and reviews. Search across everything.' },
  crew: { name: 'Crew', icon: Bot, tagline: 'Your agents: their roles, instructions and tools.' },
  talk: { name: 'Team talk', icon: MessagesSquare, tagline: 'What your agents ask and hand each other, and what came of it.' },
  automation: { name: 'Automation', icon: CalendarClock, tagline: 'Agents at work on a schedule, a webhook or a LifeOS event.' },
  integrations: { name: 'Integrations', icon: Plug, tagline: 'Connections, Vault, Claude accounts and plugins.' },
  memory: { name: 'Memory', icon: Brain, tagline: 'What your agents remember across conversations.' },
} as const satisfies Record<string, Domain>

export type DomainKey = keyof typeof DOMAINS
