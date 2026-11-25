// What each kind of document looks like: its icon, its area's tone, its name, and the page it lives on.
import type { Component } from 'vue'
import { Bot, CalendarClock, ListChecks, NotebookPen, ScrollText, StickyNote, Target, Users } from '@lucide/vue'
import type { Kind } from '../../core/generated/Kind'
import type { DomainKey } from '../../domains'

export const KINDS: Record<Kind, { label: string; icon: Component; domain: DomainKey; path: string; page: string }> = {
  task: { label: 'Task', icon: ListChecks, domain: 'tasks', path: '/tasks', page: 'Tasks' },
  goal: { label: 'Goal', icon: Target, domain: 'goals', path: '/goals', page: 'Goals' },
  person: { label: 'Person', icon: Users, domain: 'people', path: '/people', page: 'People' },
  journal: { label: 'Journal', icon: NotebookPen, domain: 'journal', path: '/journal', page: 'Journal' },
  note: { label: 'Note', icon: StickyNote, domain: 'notes', path: '/notes', page: 'Notes' },
  review: { label: 'Review', icon: ScrollText, domain: 'notes', path: '/notes', page: 'Notes' },
  crew: { label: 'Crew member', icon: Bot, domain: 'crew', path: '/crew', page: 'Crew' },
  routine: { label: 'Routine', icon: CalendarClock, domain: 'automation', path: '/automation', page: 'Automation' },
}
