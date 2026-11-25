// A goal's state as a tone and a word, once: the list and the goal's page both show it.
export function goalTone(status: string | null): { tone: string; label: string } {
  switch (status) {
    case 'done': return { tone: 'good', label: 'Achieved' }
    case 'paused': return { tone: 'warning', label: 'Paused' }
    case 'dropped': return { tone: 'neutral', label: 'Dropped' }
    default: return { tone: 'info', label: 'Active' }
  }
}
