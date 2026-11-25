// Where each kind of thing lives, once: lists, the command menu and detail pages link through here.
import type { Kind } from './core/generated/Kind'

export function docLink(kind: Kind | string, id: string): string {
  switch (kind) {
    case 'task': return `/tasks/${encodeURIComponent(id)}`
    case 'person': return `/people/${encodeURIComponent(id)}`
    case 'journal': return /^\d{4}-\d{2}-\d{2}$/.test(id) ? `/journal/${id}` : `/doc/${id}`
    case 'goal': return `/goals/${encodeURIComponent(id)}`
    case 'crew': return `/crew/${encodeURIComponent(id)}`
    default: return `/doc/${encodeURIComponent(`${kind}_${id}`)}`
  }
}
