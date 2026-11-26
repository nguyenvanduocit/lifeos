// The page's drop target (App.vue puts it on the window): files or a link dropped anywhere are handed on (to
// attachDropped, which opens the agent with them). A place that takes a drop itself (the Assistant's box, the Vault's
// import) prevents the event before it reaches the window, and keeps it; a drag that started in the page is the page's;
// text, or a link, dragged into a field being typed in is the field's. `dropping` is on while a drop here would be taken
// (the overlay shows then, and lets every event through to what is under it). verify/drop-contract.mjs drives it.
import { ref, type Ref } from 'vue'
import { carriesAttachable } from './assistant'

interface Typed {
  tagName?: string
  isContentEditable?: boolean
}
const editable = (t: EventTarget | null): boolean => {
  const el = t as Typed | null
  return !!el && (!!el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName ?? ''))
}

export function dropTarget(win: EventTarget, onDrop: (data: DataTransfer) => void): { dropping: Ref<boolean>; stop: () => void } {
  const dropping = ref(false)
  let depth = 0
  // A drag that started in this page (a link, an image, selected text) is the page's own, not something brought to it.
  let fromPage = false
  /** Whether this window takes the drop the event is part of. */
  function takes(e: DragEvent): boolean {
    const types = Array.from(e.dataTransfer?.types ?? [])
    if (fromPage || e.defaultPrevented || !carriesAttachable(types)) return false
    return types.includes('Files') || !editable(e.target)
  }
  const reset = () => {
    fromPage = false
    depth = 0
    dropping.value = false
  }
  const on: [string, (e: DragEvent) => void, boolean?][] = [
    // Taken at the capture phase, so a target that stops the event still marks it; a drag its target cancels never
    // starts (no dragend comes), so the mark is cleared once the event is through, and on any mouseup (none comes
    // during a drag).
    ['dragstart', (e) => {
      fromPage = true
      setTimeout(() => {
        if (e.defaultPrevented) fromPage = false
      })
    }, true],
    ['mouseup', () => (fromPage = false)],
    ['dragenter', () => depth++],
    ['dragleave', () => {
      depth = Math.max(0, depth - 1)
      if (!depth) dropping.value = false
    }],
    ['dragover', (e) => {
      dropping.value = takes(e)
      if (!dropping.value) return
      e.preventDefault()
      if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy'
    }],
    ['drop', (e) => {
      const mine = takes(e)
      reset()
      if (!mine || !e.dataTransfer) return
      e.preventDefault()
      onDrop(e.dataTransfer)
    }],
    ['dragend', reset],
  ]
  for (const [kind, fn, capture] of on) win.addEventListener(kind, fn as EventListener, { capture: !!capture })
  return {
    dropping,
    stop: () => {
      for (const [kind, fn, capture] of on) win.removeEventListener(kind, fn as EventListener, { capture: !!capture })
    },
  }
}
