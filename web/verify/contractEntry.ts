// What contract.mjs loads on the server (Vite's ssrLoadModule): the app's own router and pages, rendered to HTML with
// the made-up data of the fake link. Each component is rendered once the reads it asked for are answered.
import { createSSRApp, h, type ComponentPublicInstance } from 'vue'
import { renderToString } from 'vue/server-renderer'
import { RouterView } from 'vue-router'
import { router } from '../src/router'
import { reads } from './fakeLink'
import { useScenario } from './notesData'

/** Resolves once no read is waiting for its answer, and whoever asked has had a turn to take it. It looks every 10ms:
 * the fake link answers a read after 150ms (fakeLink.ts, `wait`), so a look adds at most 10ms to that (chosen; no
 * other source). */
const LOOK_MS = 10
async function settled(): Promise<void> {
  while (reads.waiting > 0) await new Promise((r) => setTimeout(r, LOOK_MS))
}

export interface Rendered {
  /** The page's HTML, and what it put in the header's actions (a Teleport to #page-actions). */
  html: string
  actions: string
  /** Every Ask chip the page made: the words it drafts are a prop, not markup. */
  chips: { label: string; words: string }[]
  /** The pages whose route asks for the whole canvas. */
  fills: string[]
}

/** The page at `at` (a path and its query), on the notes of `scenario` (notesData.ts). */
export async function render(at: string, scenario: string): Promise<Rendered> {
  useScenario(scenario)
  const chips: Rendered['chips'] = []
  const app = createSSRApp({ render: () => h(RouterView) })
  app.mixin({
    created(this: ComponentPublicInstance) {
      const p = this.$props as { words?: unknown; label?: unknown }
      if (typeof p.words === 'string') chips.push({ label: typeof p.label === 'string' ? p.label : p.words, words: p.words })
    },
    serverPrefetch: settled,
  })
  app.use(router)
  // The router's afterEach sets document.title, and there is no document here; the move itself is done by then.
  await router.push(at).catch((err: unknown) => {
    if (!/document is not defined/.test(String(err))) throw err
  })
  await router.isReady()
  const ctx: { teleports?: Record<string, string> } = {}
  const html = await renderToString(app, ctx)
  return { html, actions: ctx.teleports?.['#page-actions'] ?? '', chips, fills: router.getRoutes().filter((r) => r.meta.fill).map((r) => r.path) }
}
