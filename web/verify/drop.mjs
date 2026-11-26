#!/usr/bin/env node
// `npm run verify:drop`: a file or a link dropped anywhere on LifeOS opens the agent with it attached, in a real headless
// Chromium on made-up data through the fake link (fakeLink.ts, no server, no network), as verify:notes runs (notes.mjs
// says how, and the one-time setup). One line per assertion; exit 1 when any fails, 2 when the check could not run.
//
// What it asserts, on two pages (Notes and Crew), from the owner's words (chain ch_8ac5880933a4): "when a link or any file
// is dragged and dropped onto LifeOS, the agent opens, in attach mode":
// - while files or a link are dragged over a page, an overlay says a drop attaches them;
// - a drop of files opens the Assistant with each file in its box; a drop of a link opens it with the link in its box;
// and that the drops already there keep what they do:
// - a drop in the Assistant's box attaches the file once, not twice;
// - text or a link dragged into a field being typed in is the field's (no overlay, the Assistant not opened, the drop
//   not taken);
// - a link or image dragged from the page itself opens nothing, and a drag the page cancels leaves nothing behind;
// - the Vault's .env drop still reads the file, and the Assistant stays shut.
// The drags are DOM events with a DataTransfer the page builds (a test cannot drag from the desktop): what the app sees
// of a real drag is these events.
import fs from 'node:fs'
import path from 'node:path'
import { begin, check, fatal, finish, info, scratch, started, step, where, why } from './run.mjs'

process.env.PLAYWRIGHT_BROWSERS_PATH = '0'
const SETUP = 'cd web && npm ci && PLAYWRIGHT_BROWSERS_PATH=0 npx playwright-core install chromium-headless-shell'
const WIDE = { width: 1440, height: 900 }
// Time limits as notes.mjs has them (measured there under load).
const ACT_MS = 30_000
const LOAD_MS = 90_000
const RUN_MS = 30 * 60_000
const ORIGIN = 'http://127.0.0.1:59408'

const OVERLAY = '[data-drop-overlay]'
const BOX = 'textarea[aria-label^="Message"]'
const ATTACHED = '[data-composer] [data-images] li'
const FILES = ['trip-plan.pdf', 'receipts.csv']
const LINK = 'https://example.com/articles/sleep-and-memory'
// The pages the drops are made on, with something each one shows once drawn.
const PAGES = [
  { path: '/notes', ready: 'main [aria-label="Notes list"]' },
  { path: '/crew', ready: 'main h1' },
]

begin('verify:drop', RUN_MS)

let chromium, build, verifyConfig
try {
  ;({ chromium } = await import('playwright-core'))
  ;({ build } = await import('vite'))
  ;({ verifyConfig } = await import('./config.mjs'))
} catch (err) {
  fatal(`npm packages are missing or could not be loaded (${why(err)}). Set up once with: ${SETUP}`)
}

step('building the page')
const out = scratch('lifeos-verify-drop-')
const site = path.join(out, 'site')
try {
  await build({
    ...verifyConfig(),
    configFile: false,
    cacheDir: path.join(out, 'cache'),
    logLevel: 'error',
    clearScreen: false,
    build: { outDir: site, emptyOutDir: true, target: 'es2022', minify: false, reportCompressedSize: false },
  })
} catch (err) {
  fatal(`Vite could not build the page into ${site}: ${why(err)}`)
}

const TYPES = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.json': 'application/json',
  '.svg': 'image/svg+xml', '.png': 'image/png', '.webp': 'image/webp', '.woff2': 'font/woff2', '.woff': 'font/woff', '.ttf': 'font/ttf',
}
const uploads = []
/** Answers the page from the built files. An attachment's upload is taken (and counted); the Vault answers empty; any
 * other address is refused. */
async function serve(route) {
  const req = route.request()
  const url = new URL(req.url())
  try {
    if (url.origin !== ORIGIN) return await route.abort('blockedbyclient')
    if (url.pathname.startsWith('/api/attachments/') && req.method() === 'PUT') {
      uploads.push(decodeURIComponent(url.pathname))
      return await route.fulfill({ status: 200, body: '' })
    }
    if (url.pathname === '/api/vault') return await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ items: [], uses: [] }) })
    let file = path.join(site, decodeURIComponent(url.pathname))
    if (!file.startsWith(site + path.sep) || !fs.statSync(file, { throwIfNoEntry: false })?.isFile()) file = path.join(site, 'index.html')
    await route.fulfill({ status: 200, contentType: TYPES[path.extname(file)] ?? 'application/octet-stream', body: fs.readFileSync(file) })
  } catch (err) {
    if (!/closed|Route is already handled/i.test(String(err?.message))) throw err
  }
}

step('starting the browser')
let browser
let closing = false
try {
  browser = await chromium.launch({ headless: true })
  browser.on('disconnected', () => { if (!closing) fatal(`the browser ended on its own, while at: ${where()}`) })
} catch (err) {
  fatal(`headless Chromium did not start (${why(err)}). Set up once with: ${SETUP}`)
}

const pageErrors = []
async function openAt(at, ready) {
  const context = await browser.newContext({ viewport: WIDE, locale: 'en-US', timezoneId: 'Asia/Ho_Chi_Minh', serviceWorkers: 'block' })
  await context.route('**/*', serve)
  context.setDefaultTimeout(ACT_MS)
  context.setDefaultNavigationTimeout(LOAD_MS)
  const page = await context.newPage()
  page.on('pageerror', (e) => pageErrors.push(String(e.message)))
  await page.goto(`${ORIGIN}${at}${at.includes('?') ? '&' : '?'}scenario=many&as=idle`)
  await page.locator(ready).first().waitFor({ state: 'attached', timeout: LOAD_MS })
  return page
}

// ---- what runs inside the page --------------------------------------------------------------------------------------

/** Builds a DataTransfer as a drag from outside would carry it, and keeps it on window for the drag's events. */
function carryInPage({ files, link, text }) {
  const dt = new DataTransfer()
  for (const name of files ?? []) dt.items.add(new File([`made-up ${name}`], name, { type: name.endsWith('.pdf') ? 'application/pdf' : 'text/plain' }))
  if (link) {
    dt.setData('text/uri-list', link)
    dt.setData('text/plain', link)
  }
  if (text) dt.setData('text/plain', text)
  window.__drag = dt
}
/** Fires `kinds` of drag events at the element `sel` (the page's middle when none) with the carried DataTransfer, and
 * says, per event, whether something took it (preventDefault). */
function fireInPage([sel, kinds]) {
  const target = sel ? document.querySelector(sel) : document.elementFromPoint(innerWidth / 3, innerHeight / 2)
  if (!target) throw new Error(`nothing at ${sel}`)
  const taken = {}
  for (const kind of kinds) {
    const e = new DragEvent(kind, { bubbles: true, cancelable: true, composed: true, dataTransfer: window.__drag, clientX: innerWidth / 3, clientY: innerHeight / 2 })
    target.dispatchEvent(e)
    taken[kind] = e.defaultPrevented
  }
  return taken
}
/** What shows: the overlay, whether the Assistant is open (in the address), the files in its box, and its words. */
function seenInPage([overlay, box, attached]) {
  const o = document.querySelector(overlay)
  const r = o?.getBoundingClientRect()
  const boxEl = document.querySelector(box)
  return {
    overlay: !!o && r.width > 0 && r.height > 0 && getComputedStyle(o).visibility !== 'hidden' && getComputedStyle(o).display !== 'none',
    open: new URL(location.href).searchParams.has('assistant'),
    attached: [...document.querySelectorAll(attached)].map((li) => li.textContent.trim()).filter(Boolean),
    words: boxEl?.value ?? null,
  }
}
const seen = (page) => page.evaluate(seenInPage, [OVERLAY, BOX, ATTACHED])
const describe = (s) => `overlay ${s.overlay ? 'shown' : 'not shown'}, Assistant ${s.open ? 'open' : 'shut'}, box has ${s.attached.length ? JSON.stringify(s.attached) : 'no files'}${s.words ? ` and words ${JSON.stringify(s.words)}` : ''}`
/** Waits until `ok(seen)` holds, or the limit; returns the last seen. */
async function until(page, ok, ms = ACT_MS) {
  const end = Date.now() + ms
  let s = await seen(page)
  while (!ok(s) && Date.now() < end) {
    await page.waitForTimeout(100)
    s = await seen(page)
  }
  return s
}

// ---- the run --------------------------------------------------------------------------------------------------------

try {
  for (const p of PAGES) {
    step(`dropping files on ${p.path}`)
    const page = await openAt(p.path, p.ready)
    await page.evaluate(carryInPage, { files: FILES })
    await check(`${p.path}: files dragged over the page show the drop overlay`, async () => {
      await page.evaluate(fireInPage, [null, ['dragenter', 'dragover']])
      const s = await until(page, (s) => s.overlay, 3000)
      return { ok: s.overlay && !s.open, measured: describe(s), limit: 'overlay shown, Assistant not yet open' }
    })
    await check(`${p.path}: a drop of ${FILES.length} files opens the Assistant with each file in its box`, async () => {
      const taken = await page.evaluate(fireInPage, [null, ['drop']])
      const s = await until(page, (s) => s.open && FILES.every((f) => s.attached.includes(f)) && !s.overlay)
      const ok = taken.drop && s.open && !s.overlay && FILES.every((f) => s.attached.includes(f)) && s.attached.length === FILES.length
      return { ok, measured: `drop ${taken.drop ? 'taken' : 'not taken'}; ${describe(s)}`, limit: `drop taken, Assistant open, overlay gone, box has ${JSON.stringify(FILES)}` }
    })
    await page.context().close()

    step(`dropping a link on ${p.path}`)
    const linked = await openAt(p.path, p.ready)
    await linked.evaluate(carryInPage, { link: LINK })
    await check(`${p.path}: a link dragged over the page shows the drop overlay`, async () => {
      await linked.evaluate(fireInPage, [null, ['dragenter', 'dragover']])
      const s = await until(linked, (s) => s.overlay, 3000)
      return { ok: s.overlay && !s.open, measured: describe(s), limit: 'overlay shown, Assistant not yet open' }
    })
    await check(`${p.path}: a drop of a link opens the Assistant with the link in its box`, async () => {
      const taken = await linked.evaluate(fireInPage, [null, ['drop']])
      const s = await until(linked, (s) => s.open && (s.words ?? '').includes(LINK) && !s.overlay)
      const ok = taken.drop && s.open && !s.overlay && (s.words ?? '').includes(LINK)
      return { ok, measured: `drop ${taken.drop ? 'taken' : 'not taken'}; ${describe(s)}`, limit: `drop taken, Assistant open, overlay gone, ${LINK} in the box` }
    })
    await linked.context().close()
  }

  step('dropping a file in the Assistant box')
  const boxed = await openAt('/notes?assistant=1', BOX)
  await boxed.evaluate(carryInPage, { files: [FILES[0]] })
  await check('the Assistant box: a file dropped in it is attached once', async () => {
    const before = uploads.length
    await boxed.evaluate(fireInPage, ['[data-composer] > div', ['dragenter', 'dragover', 'drop']])
    let s = await until(boxed, (s) => s.attached.includes(FILES[0]))
    // Room for a second, wrong copy to arrive.
    await boxed.waitForTimeout(500)
    s = await seen(boxed)
    const copies = s.attached.filter((f) => f === FILES[0]).length
    const put = uploads.length - before
    return { ok: copies === 1 && put === 1 && !s.overlay, measured: `${copies} in the box, ${put} uploaded, overlay ${s.overlay ? 'shown' : 'not shown'}`, limit: '1 in the box, 1 upload, no overlay' }
  })
  await boxed.context().close()

  // A box outside the Assistant's: Notes' search field. (The Assistant's own box takes every drop on it, text too, as it did
  // before: its handler, not this one.)
  step('dragging text and a link into a field being typed in')
  const field = await openAt('/notes', PAGES[0].ready)
  const SEARCH = 'input[aria-label="Search the vault"]'
  for (const [what, carried] of [['text', { text: 'a few words' }], ['a link', { link: LINK }]]) {
    await field.evaluate(carryInPage, carried)
    await check(`a field being typed in: ${what} dragged into it is left to the field`, async () => {
      const taken = await field.evaluate(fireInPage, [SEARCH, ['dragenter', 'dragover']])
      const during = await seen(field)
      const dropped = await field.evaluate(fireInPage, [SEARCH, ['drop']])
      await field.waitForTimeout(300)
      const s = await seen(field)
      const ok = !taken.dragover && !dropped.drop && !during.overlay && !s.overlay && !s.open
      return { ok, measured: `dragover ${taken.dragover ? 'taken' : 'left'}, drop ${dropped.drop ? 'taken' : 'left to the browser'}; overlay ${during.overlay ? 'shown' : 'not shown'} during; after: ${describe(s)}`, limit: 'not taken by the app, no overlay, Assistant shut' }
    })
  }
  await field.context().close()

  // A drag that starts in the page (a link of the sidebar, the logo's image) is the page's own, not something brought to
  // it: dragged with the mouse, as the owner would, it shows no overlay and opens nothing.
  for (const [what, sel] of [['a sidebar link', 'aside a[href="/crew"]'], ["the logo's image", 'aside a[href="/"] img']]) {
    step(`dragging ${what} within the page`)
    const own = await openAt('/notes', PAGES[0].ready)
    await check(`a drag within the page: ${what} dropped on the page opens nothing`, async () => {
      const from = await own.locator(sel).first().boundingBox()
      if (!from) throw new Error(`no ${sel}`)
      const to = { x: WIDE.width / 2, y: WIDE.height / 2 }
      await own.mouse.move(from.x + from.width / 2, from.y + from.height / 2)
      await own.mouse.down()
      let during = await seen(own)
      for (let n = 1; n <= 10; n++) {
        await own.mouse.move(from.x + (to.x - from.x) * n / 10, from.y + (to.y - from.y) * n / 10)
        const s = await seen(own)
        if (s.overlay) during = s
      }
      await own.mouse.up()
      await own.waitForTimeout(500)
      const s = await seen(own)
      const ok = !during.overlay && !s.overlay && !s.open && !s.words
      return { ok, measured: `overlay ${during.overlay ? 'shown' : 'not shown'} during; after: ${describe(s)}`, limit: 'no overlay, Assistant shut, box empty' }
    })
    await own.context().close()
  }

  // A drag the page cancels at its start (the widget grid does while it is organized: WidgetSlot.vue, dragstart
  // prevented) never happens, so it leaves nothing behind: a file brought from outside afterwards is still taken.
  step('a cancelled drag, then a file from outside')
  const cancelled = await openAt('/notes', PAGES[0].ready)
  await check('after a drag the page cancelled, a file brought from outside is still taken', async () => {
    await cancelled.evaluate(() => {
      const a = document.querySelector('aside a[href="/crew"]')
      a.addEventListener('dragstart', (e) => e.preventDefault())
    })
    const from = await cancelled.locator('aside a[href="/crew"]').first().boundingBox()
    await cancelled.mouse.move(from.x + from.width / 2, from.y + from.height / 2)
    await cancelled.mouse.down()
    for (let n = 1; n <= 10; n++) await cancelled.mouse.move(from.x + (WIDE.width / 2 - from.x) * n / 10, from.y + (WIDE.height / 2 - from.y) * n / 10)
    await cancelled.mouse.up()
    await cancelled.waitForTimeout(300)
    await cancelled.evaluate(carryInPage, { files: [FILES[0]] })
    await cancelled.evaluate(fireInPage, [null, ['dragenter', 'dragover']])
    const during = await until(cancelled, (s) => s.overlay, 3000)
    const taken = await cancelled.evaluate(fireInPage, [null, ['drop']])
    const s = await until(cancelled, (s) => s.open && s.attached.includes(FILES[0]))
    const ok = during.overlay && taken.drop && s.open && s.attached.includes(FILES[0])
    return { ok, measured: `overlay ${during.overlay ? 'shown' : 'not shown'} during; drop ${taken.drop ? 'taken' : 'not taken'}; after: ${describe(s)}`, limit: `overlay shown, drop taken, Assistant open with ${FILES[0]}` }
  })
  await cancelled.context().close()

  step('the Vault .env drop')
  const vault = await openAt('/connections/vault', 'main h1')
  await check('the Vault: a .env dropped on Import .env is read there, the Assistant stays shut', async () => {
    await vault.getByRole('button', { name: /Import \.env/ }).click()
    await vault.getByText('Drop a .env file here').waitFor()
    await vault.evaluate(() => {
      const dt = new DataTransfer()
      dt.items.add(new File(['EXAMPLE_NAME=made-up-value\n'], '.env', { type: 'text/plain' }))
      window.__drag = dt
    })
    const sel = 'label:has(input[type=file][accept*=".env"])'
    await vault.evaluate(fireInPage, [sel, ['dragenter', 'dragover']])
    const during = await seen(vault)
    await vault.evaluate(fireInPage, [sel, ['drop']])
    const read = await vault.locator('code', { hasText: 'EXAMPLE_NAME' }).first().waitFor({ timeout: ACT_MS }).then(() => true, () => false)
    await vault.waitForTimeout(300)
    const s = await seen(vault)
    return { ok: read && !during.overlay && !s.open && !s.overlay, measured: `key ${read ? 'read' : 'not read'}; overlay ${during.overlay ? 'shown' : 'not shown'} during; after: ${describe(s)}`, limit: 'EXAMPLE_NAME listed, no overlay, Assistant shut' }
  })
  await vault.context().close()
} catch (err) {
  closing = true
  await browser.close().catch(() => {})
  fatal(`${why(err)}, while at: ${where()}`)
}

closing = true
await browser.close()
await check('the pages threw no errors', () => ({ ok: pageErrors.length === 0, measured: pageErrors.length ? pageErrors.join(' | ') : 'none', limit: '0 uncaught errors' }))
info(`took ${Math.round((Date.now() - started) / 1000)}s`)
finish()
