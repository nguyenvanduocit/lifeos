// The kinds of memory, once: their word, mark and tone (the Memory page, an agent's page).
import type { Component } from 'vue'
import { Brain, Compass, Gavel, GraduationCap, Heart, Lightbulb, Target } from '@lucide/vue'

export const KINDS: Record<string, { label: string; icon: Component; tone: string }> = {
  fact: { label: 'Fact', icon: Lightbulb, tone: 'info' },
  preference: { label: 'Preference', icon: Heart, tone: 'people' },
  principle: { label: 'Principle', icon: Compass, tone: 'goals' },
  lesson: { label: 'Lesson', icon: GraduationCap, tone: 'habits' },
  decision: { label: 'Decision', icon: Gavel, tone: 'automation' },
  intent: { label: 'Intent', icon: Target, tone: 'tasks' },
}

export const kindOf = (k: string) => KINDS[k] ?? { label: k, icon: Brain, tone: 'neutral' }
