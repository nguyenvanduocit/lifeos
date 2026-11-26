#!/usr/bin/env node
// `npm run verify:drop:contract`: what a drop anywhere on LifeOS does, without a browser. The page's own drop target
// (src/dropTarget.ts, which App.vue puts on the window) and the Assistant's store (src/assistant.ts, on the fake link)
// are loaded through Vite with no server listening, and driven with drag events this check makes: a stand-in window,
// events with a DataTransfer of files or a link, and a `fetch` that takes uploads. One line per assertion; exit 1 when
// any fails, 2 when the check could not run.
//
// It is the check for a place that cannot start a browser (Remit's check jail refuses Chromium); `npm run verify:drop`
// (drop.mjs) drives the same in a real Chromium. It needs node and web/node_modules, opens no socket, starts no other
// program, watches no file, and writes only under os.tmpdir().
//
// What it CANNOT see, and drop.mjs does: the overlay drawn (only the state that shows it), the Assistant panel and its
// box on screen, events bubbling through real elements (an element's own drop zone is stood in for by an event already
// prevented, as such a zone leaves it), and a browser's own drags (a drag started in the page is a `dragstart` here).
import path from 'node:path'
import { begin, check, fatal, finish, info, scratch, started, step, where, why } from './run.mjs'

const SETUP = 'cd web && npm ci'
// Remit's jail ends a check after 5 minutes; this one says where it was before that.
const RUN_MS = 4 * 60_000

begin('verify:drop:contract', RUN_MS)

// ---- the names only a browser has, which the fake link and the app's modules read when they load --------------------
const store = () => {
  const kept = new Map()
  return { getItem: (k) => kept.get(k) ?? null, setItem: (k, v) => void kept.set(k, String(v)), removeItem: (k) => void kept.delete(k), clear: () => kept.clear() }
}
for (const [name, value] of Object.entries({ window: globalThis, location: { search: '?as=idle&still=1', href: 'http://127.0.0.1/?as=idle&still=1' }, sessionStorage: store(), localStorage: store() })) {
  Object.defineProperty(globalThis, name, { value, configurable: true, writable: true })
}
// Uploads (images.ts puts a file with fetch): taken, and named here.
const uploads = []
globalThis.fetch = async (url, init) => {
  uploads.push({ url: String(url), method: init?.method ?? 'GET' })
  return new Response('', { status: 200 })
}

let createServer, verifyConfig
try {
  ;({ createServer } = await import('vite'))
  ;({ verifyConfig } = await import('./config.mjs'))
} catch (err) {
  fatal(`npm packages are missing or could not be loaded (${why(err)}). Set up once with: ${SETUP}`)
}

step('making a directory for Vite’s cache')
const out = scratch('lifeos-verify-drop-contract-')
step('starting Vite without a server')
let vite
try {
  vite = await createServer({
    ...verifyConfig(),
    configFile: false,
    cacheDir: path.join(out, 'cache'),
    logLevel: 'silent',
    appType: 'custom',
    server: { middlewareMode: true, hmr: false, ws: false, watch: null },
    optimizeDeps: { noDiscovery: true, include: [] },
  })
} catch (err) {
  fatal(`${why(err)}, while at: ${where()}`)
}

step('loading the drop target and the Assistant’s store')
let dropTarget = null
let assistant = null
let missing = ''
try {
  ;({ dropTarget } = await vite.ssrLoadModule('/src/dropTarget.ts'))
  assistant = await vite.ssrLoadModule('/src/assistant.ts')
  if (typeof dropTarget !== 'function') throw new Error('src/dropTarget.ts exports no dropTarget')
  if (typeof assistant.attachDropped !== 'function') throw new Error('src/assistant.ts exports no attachDropped')
} catch (err) {
  // A page with no drop target fails every assertion below; it is not a check that could not run.
  missing = String(err?.message ?? err).split('\n')[0]
}

// ---- the stand-ins for a browser's drag -------------------------------------------------------------------------------

/** A DataTransfer as a drag from outside carries it: files, or a link (text/uri-list and text/plain), or text. */
function carried({ files = [], link = '', text = '' } = {}) {
  const data = new Map()
  if (link) data.set('text/uri-list', link).set('text/plain', link)
  if (text) data.set('text/plain', text)
  const list = files.map((name) => new File([`made-up ${name}`], name, { type: name.endsWith('.pdf') ? 'application/pdf' : 'text/plain' }))
  return {
    types: [...(list.length ? ['Files'] : []), ...data.keys()],
    files: list,
    getData: (type) => data.get(type) ?? '',
    dropEffect: 'none',
  }
}
const FIELD = { tagName: 'INPUT', isContentEditable: false }
const PAGE = { tagName: 'DIV', isContentEditable: false }
/** Fires a drag event of `kind` at the stand-in window and says whether the page took it (preventDefault). `taken`
 * marks it taken before, as a drop zone of its own leaves it; `on` is the element it is over. */
function fire(win, kind, data, { on = PAGE, taken = false } = {}) {
  const e = new Event(kind, { cancelable: true })
  Object.defineProperty(e, 'dataTransfer', { value: data })
  Object.defineProperty(e, 'target', { value: on })
  if (taken) e.preventDefault()
  win.dispatchEvent(e)
  return e
}
/** A drop target on a fresh stand-in window, with each drop it hands on kept. */
function fresh() {
  const win = new EventTarget()
  const dropped = []
  const target = dropTarget(win, (data) => dropped.push(data))
  return { win, dropped, target }
}
const tick = () => new Promise((r) => setTimeout(r, 0))
/** Waits until `ok()` holds, at most `ms`. */
async function until(ok, ms = 3000) {
  const end = Date.now() + ms
  while (!ok() && Date.now() < end) await new Promise((r) => setTimeout(r, 10))
  return ok()
}
const FILES = ['trip-plan.pdf', 'receipts.csv']
const LINK = 'https://example.com/articles/sleep-and-memory'
const absent = () => ({ ok: false, measured: `no drop target: ${missing}`, limit: 'src/dropTarget.ts and attachDropped' })

// ---- the run --------------------------------------------------------------------------------------------------------

try {
  await check('files dragged over the page: the overlay is on and the drag is taken', () => {
    if (missing) return absent()
    const { win, target } = fresh()
    const data = carried({ files: FILES })
    fire(win, 'dragenter', data)
    const over = fire(win, 'dragover', data)
    return { ok: target.dropping.value && over.defaultPrevented && data.dropEffect === 'copy', measured: `overlay ${target.dropping.value ? 'on' : 'off'}, dragover ${over.defaultPrevented ? 'taken' : 'left'}, dropEffect ${data.dropEffect}`, limit: 'on, taken, copy' }
  })
  await check('files dropped: the drop is taken, the overlay goes off and the files are handed on', () => {
    if (missing) return absent()
    const { win, dropped, target } = fresh()
    const data = carried({ files: FILES })
    fire(win, 'dragenter', data)
    fire(win, 'dragover', data)
    const drop = fire(win, 'drop', data)
    return { ok: drop.defaultPrevented && !target.dropping.value && dropped.length === 1 && dropped[0] === data, measured: `drop ${drop.defaultPrevented ? 'taken' : 'left'}, overlay ${target.dropping.value ? 'on' : 'off'}, ${dropped.length} handed on`, limit: 'taken, off, 1 handed on' }
  })
  await check('a link dragged over the page and dropped: overlay on, then taken and handed on', () => {
    if (missing) return absent()
    const { win, dropped, target } = fresh()
    const data = carried({ link: LINK })
    fire(win, 'dragenter', data)
    fire(win, 'dragover', data)
    const on = target.dropping.value
    const drop = fire(win, 'drop', data)
    return { ok: on && drop.defaultPrevented && dropped.length === 1, measured: `overlay ${on ? 'on' : 'off'} during, drop ${drop.defaultPrevented ? 'taken' : 'left'}, ${dropped.length} handed on`, limit: 'on, taken, 1 handed on' }
  })
  await check('a drop a zone of its own took (the Assistant box, the Vault import) is left to it', () => {
    if (missing) return absent()
    const { win, dropped, target } = fresh()
    const data = carried({ files: FILES })
    fire(win, 'dragenter', data)
    fire(win, 'dragover', data, { taken: true })
    const on = target.dropping.value
    fire(win, 'drop', data, { taken: true })
    return { ok: !on && dropped.length === 0, measured: `overlay ${on ? 'on' : 'off'}, ${dropped.length} handed on`, limit: 'off, 0 handed on' }
  })
  for (const [what, carry] of [['text', { text: 'a few words' }], ['a link', { link: LINK }]]) {
    await check(`${what} dragged into a field being typed in is left to the field`, () => {
      if (missing) return absent()
      const { win, dropped, target } = fresh()
      const data = carried(carry)
      fire(win, 'dragenter', data, { on: FIELD })
      const over = fire(win, 'dragover', data, { on: FIELD })
      const on = target.dropping.value
      const drop = fire(win, 'drop', data, { on: FIELD })
      return { ok: !on && !over.defaultPrevented && !drop.defaultPrevented && dropped.length === 0, measured: `overlay ${on ? 'on' : 'off'}, dragover ${over.defaultPrevented ? 'taken' : 'left'}, drop ${drop.defaultPrevented ? 'taken' : 'left'}, ${dropped.length} handed on`, limit: 'off, left, left, 0' }
    })
  }
  await check('text dragged over the page (no file, no link) is not taken', () => {
    if (missing) return absent()
    const { win, dropped, target } = fresh()
    const data = carried({ text: 'a few words' })
    fire(win, 'dragenter', data)
    const over = fire(win, 'dragover', data)
    fire(win, 'drop', data)
    return { ok: !target.dropping.value && !over.defaultPrevented && dropped.length === 0, measured: `overlay ${target.dropping.value ? 'on' : 'off'}, dragover ${over.defaultPrevented ? 'taken' : 'left'}, ${dropped.length} handed on`, limit: 'off, left, 0' }
  })
  await check('a drag started in the page (a link, an image) is not taken', () => {
    if (missing) return absent()
    const { win, dropped, target } = fresh()
    const data = carried({ link: 'http://127.0.0.1:7420/crew' })
    fire(win, 'dragstart', data)
    fire(win, 'dragenter', data)
    const over = fire(win, 'dragover', data)
    const on = target.dropping.value
    const drop = fire(win, 'drop', data)
    return { ok: !on && !over.defaultPrevented && !drop.defaultPrevented && dropped.length === 0, measured: `overlay ${on ? 'on' : 'off'}, dragover ${over.defaultPrevented ? 'taken' : 'left'}, drop ${drop.defaultPrevented ? 'taken' : 'left'}, ${dropped.length} handed on`, limit: 'off, left, left, 0' }
  })
  await check('after a drag started in the page ends, a file from outside is taken again', () => {
    if (missing) return absent()
    const { win, dropped } = fresh()
    fire(win, 'dragstart', carried({ link: 'http://127.0.0.1:7420/crew' }))
    fire(win, 'dragend', carried())
    const data = carried({ files: [FILES[0]] })
    fire(win, 'dragenter', data)
    fire(win, 'dragover', data)
    const drop = fire(win, 'drop', data)
    return { ok: drop.defaultPrevented && dropped.length === 1, measured: `drop ${drop.defaultPrevented ? 'taken' : 'left'}, ${dropped.length} handed on`, limit: 'taken, 1' }
  })
  await check('a drag the page cancels at its start leaves nothing behind: a file from outside is taken', async () => {
    if (missing) return absent()
    const { win, dropped, target } = fresh()
    fire(win, 'dragstart', carried({ link: 'http://127.0.0.1:7420/crew' }), { taken: true })
    await tick()
    const data = carried({ files: [FILES[0]] })
    fire(win, 'dragenter', data)
    fire(win, 'dragover', data)
    const on = target.dropping.value
    const drop = fire(win, 'drop', data)
    return { ok: on && drop.defaultPrevented && dropped.length === 1, measured: `overlay ${on ? 'on' : 'off'}, drop ${drop.defaultPrevented ? 'taken' : 'left'}, ${dropped.length} handed on`, limit: 'on, taken, 1' }
  })
  await check('dropped files open the Assistant with each file uploaded and in its box', async () => {
    if (missing) return absent()
    const { ASSISTANT, agentState, panelOpen, attachDropped, setImages, setDraft } = assistant
    setImages(ASSISTANT, [])
    setDraft(ASSISTANT, '')
    panelOpen.value = false
    const before = uploads.length
    attachDropped(carried({ files: FILES }))
    const a = agentState(ASSISTANT)
    const names = () => a.images.map((i) => i.id.slice(i.id.indexOf('/') + 1))
    await until(() => FILES.every((f) => names().includes(f)) && a.putting === 0)
    const put = uploads.slice(before).filter((u) => u.method === 'PUT').map((u) => decodeURIComponent(u.url))
    const ok = panelOpen.value && FILES.every((f) => names().includes(f)) && put.length === FILES.length && !a.notAdded
    return { ok, measured: `Assistant ${panelOpen.value ? 'open' : 'shut'}, box has ${JSON.stringify(names())}, ${put.length} uploads${a.notAdded ? `, not added: ${a.notAdded}` : ''}`, limit: `open, ${JSON.stringify(FILES)}, ${FILES.length} uploads` }
  })
  await check('a dropped link opens the Assistant with the link in its words', () => {
    if (missing) return absent()
    const { ASSISTANT, agentState, panelOpen, attachDropped, setImages, setDraft } = assistant
    setImages(ASSISTANT, [])
    setDraft(ASSISTANT, 'Read this:')
    panelOpen.value = false
    attachDropped(carried({ link: LINK }))
    const a = agentState(ASSISTANT)
    const ok = panelOpen.value && a.draft === `Read this:\n${LINK}\n` && a.images.length === 0
    return { ok, measured: `Assistant ${panelOpen.value ? 'open' : 'shut'}, words ${JSON.stringify(a.draft)}, ${a.images.length} files`, limit: `open, "Read this:\\n${LINK}\\n", 0 files` }
  })
} catch (err) {
  await vite?.close().catch(() => {})
  fatal(`${why(err)}, while at: ${where()}`)
}

await vite.close().catch(() => {})
info(`took ${Math.round((Date.now() - started) / 1000)}s; ${uploads.length} uploads taken by the stand-in fetch, nothing sent`)
finish()
