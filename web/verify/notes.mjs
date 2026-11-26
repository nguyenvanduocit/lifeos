#!/usr/bin/env node
// `npm run verify:notes`: the Notes page in a real headless Chromium, on made-up notes through the fake link
// (notesData.ts, no server, no network). One line per assertion (✓/✗ name, what was measured, the limit); exit 1 when
// any assertion fails, exit 2 when the check itself could not run (no browser, the page did not load, or the place it
// runs in refused something it needs: one line says what).
//
// One-time setup (needs the network; the check itself runs offline afterwards):
//   cd web && npm ci && PLAYWRIGHT_BROWSERS_PATH=0 npx playwright-core install chromium-headless-shell
// The browser is kept inside web/node_modules/playwright-core/.local-browsers. A run writes nothing in the checkout:
// the built page and the browser's profile go to the system temp directory. NOTES_SHOTS=<dir> also saves screenshots
// there.
//
// It runs in Remit's check jail, as far as this script can see to it (remit dd1b3c11: crates/jail/src/sandbox_darwin.rs:32
// `(deny default)`, crates/shipping/src/repochecks.rs:176-200 a command check gets no network rule, 22-29 its limits):
// - no socket: the page is built once (Vite) and its files are handed to the browser by this process (`route`), and
//   the browser is driven over a pipe. Nothing listens and nothing connects, loopback included.
// - files: written only under os.tmpdir() (the jail's TMPDIR and HOME; the worktree's .git is read-only there).
// - time: the jail ends a check after 5 minutes and any one process after 2 minutes of CPU; the last line of a run says
//   what this run took.
// Whether Chromium itself starts there is not known: the jail allows a program per-pid Mach services only
// (/System/Library/Sandbox/Profiles/system.sb:18-19) and Chromium registers a named one for its child processes. If it
// does not, this check says so and exits 2; verify:notes:contract (contract.mjs) is the check that needs no browser.
import fs from 'node:fs'
import path from 'node:path'
import { A_REVIEW, LONG_ID, LONG_STARTS, LONG_TITLE, NO_GARDEN, OTHER_NOTE, begin, check, fatal, finish, info, scratch, started, step, where, why } from './run.mjs'

process.env.PLAYWRIGHT_BROWSERS_PATH = '0'
const SETUP = 'cd web && npm ci && PLAYWRIGHT_BROWSERS_PATH=0 npx playwright-core install chromium-headless-shell'
// ---- the sizes and limits, each with where it comes from -------------------------------------------------------------
// The two screens of the brief (chain ch_e165c2b26beb): the owner's screenshot was about 1568px wide; a phone.
const WIDE = { width: 1568, height: 900 }
const PHONE = { width: 390, height: 844 }
// The brief's limits, set by the chain's Chair before the page was changed, and what the old page measured:
// - the gap between `main` and the page's content, each side. The old page, held in the shell's 1240px column, had 71px.
const MAX_GAP = 48
// - the median number of characters on a line of the long note. The old 410px peek had 60; docs/design.md's 68ch
//   measured 96, which is why the Reader column became 56ch (79).
const MEASURE = { min: 50, max: 85 }
// - how much of the viewport's height the note is read in. The old peek had 78%.
const MIN_READING = 0.7
// - the long fixture note, as long as the owner's own (a book's notes).
const MIN_WORDS = 1500
// - the note's column beside the open Assistant: 45ch of 16px Geist, whose "0" is 10.608px wide. The old peek had 410px.
const MIN_COLUMN_BESIDE_ASSISTANT = 477
// The Reader type (docs/design.md "Reader", the `.reader` rule in style.css): 16px, line height 1.7, at most 56ch.
const FONT = 16
const LEADING = 16 * 1.7
const COLUMN = 594 // 56 × 10.608px
// The shell (App.vue): `main` is the window less the 248px sidebar, the 8px right padding and the canvas's two 1px
// borders; the Assistant column opens 420px wide beside a 1px handle.
const MAIN = WIDE.width - 248 - 8 - 2
const MAIN_BESIDE_ASSISTANT = MAIN - 420 - 1
// The page's own sizes (Notes.vue), chosen for this page with no outside source: the list pane (340px, 300px below a
// 64rem page), a row's least height (min-h-14, a finger's target), the focus outline (outline-2), and the least the
// panes get when page widgets take the height: the list's two 48px bars and four 57px rows.
const LIST_WIDTH = { wide: 340, narrow: 300 }
const MIN_ROW = 56
const FOCUS_OUTLINE = 2
const FLOOR = 2 * 48 + 4 * 57
// The phone's text column: at most the window less the shell's 16px page padding each side (App.vue, px-4), and at least
// four fifths of the window (chosen: "full width" with room for a card's own padding).
const PHONE_COLUMN = { min: 0.8 * PHONE.width, max: PHONE.width - 2 * 16 }
// Boxes are compared within a pixel or two: sizes are rounded, and a 1px border may fall on either side.

// The page's two panes, by the names it gives them.
const LIST = 'main [aria-label="Notes list"]'
const READER = 'main section[aria-label="Note"]'

// Time limits, from what this machine (10 cores) measured while the page came from a Vite dev server. With little else
// running (load 16–31) a whole run took 33–51s. Under other work (load 58–75) everything ran about 18 times slower (a
// type check: 20s of CPU in 6m04s): a click took more than 8s. A limit that is too short fails an assertion the page
// meets, so each has room; the whole run has a limit too, so it cannot hang.
// - a click, or a wait for what it brings: nearly four times the 8s that was not enough.
const ACT_MS = 30_000
// - a page load: three times a click's limit (chosen; with the dev server no load after the first exceeded 8s but one).
const LOAD_MS = 90_000
// - the whole run: 51s × 18 is about 15 minutes, doubled. Remit's jail ends it after 5 minutes whatever this says.
const RUN_MS = 30 * 60_000
const shots = process.env.NOTES_SHOTS ? path.resolve(process.env.NOTES_SHOTS) : ''

begin('verify:notes', RUN_MS)

// ---- what runs inside the page -------------------------------------------------------------------------------------

/** The left and right gap between `main` and what is painted in it (borders, backgrounds, text), in px. */
function gapsInPage() {
  const main = document.querySelector('main')
  const m = main.getBoundingClientRect()
  const visible = (c) => c && c !== 'transparent' && !/rgba\(.*,\s*0\)$/.test(c)
  let left = Infinity
  let right = -Infinity
  for (const el of main.querySelectorAll('*')) {
    const r = el.getBoundingClientRect()
    if (r.width <= 1 || r.height <= 1 || r.bottom < m.top || r.top > m.bottom) continue
    const cs = getComputedStyle(el)
    if (cs.visibility === 'hidden' || cs.display === 'none') continue
    const text = [...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim())
    const painted = visible(cs.backgroundColor) || (parseFloat(cs.borderLeftWidth) > 0 && visible(cs.borderLeftColor)) || text
    if (!painted) continue
    left = Math.min(left, Math.max(r.left, m.left))
    right = Math.max(right, Math.min(r.right, m.right))
  }
  return { main: Math.round(m.width), surface: Math.round(right - left), left: Math.round(left - m.left), right: Math.round(m.left + main.clientWidth - right) }
}

/** The open long note as it is rendered: characters per line of its paragraphs, font size, column width, how tall the
 * area it scrolls in is, and how wide a row of the list beside it is (0 when no list shows). */
function readerInPage([starts, otherTitle]) {
  const shown = (el) => {
    const r = el.getBoundingClientRect()
    return r.width > 1 && r.height > 1 && getComputedStyle(el).visibility !== 'hidden'
  }
  const first = [...document.querySelectorAll('p')].find((p) => p.textContent.startsWith(starts) && shown(p))
  if (!first) return null
  const body = first.parentElement
  const lines = []
  for (const p of body.querySelectorAll(':scope > p')) {
    const perLine = new Map()
    const walker = document.createTreeWalker(p, NodeFilter.SHOW_TEXT)
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      for (const m of node.textContent.matchAll(/\S+\s*/g)) {
        const range = document.createRange()
        range.setStart(node, m.index)
        range.setEnd(node, m.index + m[0].trimEnd().length)
        const top = Math.round(range.getClientRects()[0]?.top ?? NaN)
        if (Number.isNaN(top)) continue
        perLine.set(top, (perLine.get(top) ?? 0) + m[0].length)
      }
    }
    const counts = [...perLine.entries()].sort((a, b) => a[0] - b[0]).map(([, n]) => n)
    if (counts.length >= 3) lines.push(...counts.slice(0, -1))
  }
  lines.sort((a, b) => a - b)
  let scroller = body.parentElement
  while (scroller && scroller !== document.documentElement) {
    const o = getComputedStyle(scroller).overflowY
    if ((o === 'auto' || o === 'scroll') && scroller.scrollHeight > scroller.clientHeight) break
    scroller = scroller.parentElement
  }
  const s = (scroller ?? document.documentElement).getBoundingClientRect()
  const height = Math.max(0, Math.min(s.bottom, innerHeight) - Math.max(s.top, 0))
  const other = [...document.querySelectorAll('main *')].find((el) => el.children.length === 0 && el.textContent.trim() === otherTitle && shown(el))
  const pane = document.querySelector('main section[aria-label="Note"]')?.getBoundingClientRect()
  const row = other?.closest('[role=row], [role=option], li') ?? other?.closest('a, button') ?? other
  return {
    lines: lines.length,
    median: lines.length ? lines[Math.floor(lines.length / 2)] : 0,
    min: lines[0] ?? 0,
    max: lines[lines.length - 1] ?? 0,
    font: parseFloat(getComputedStyle(first).fontSize),
    leading: parseFloat(getComputedStyle(first).lineHeight),
    offCentre: pane ? Math.round(10 * (first.getBoundingClientRect().left + first.getBoundingClientRect().width / 2 - (pane.left + pane.width / 2))) / 10 : null,
    column: Math.round(first.getBoundingClientRect().width),
    height: Math.round(height),
    viewport: innerHeight,
    list: row ? Math.round(row.getBoundingClientRect().width) : 0,
  }
}

/** The two panes as they are laid out, and what the list marks and the reader shows. */
function panesInPage(listSel) {
  const box = (el) => {
    if (!el) return null
    const r = el.getBoundingClientRect()
    return { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) }
  }
  const main = document.querySelector('main')
  const list = document.querySelector(listSel)
  const reader = document.querySelector('main section[aria-label="Note"]')
  const rows = list ? [...list.querySelectorAll('li')] : []
  const title = (li) => li.querySelector('a span span')?.textContent.trim() ?? li.textContent.trim()
  return {
    main: box(main),
    list: box(list),
    reader: box(reader),
    mainScroll: main.scrollHeight,
    mainClient: main.clientHeight,
    rows: rows.length,
    minRow: rows.length ? Math.min(...rows.map((li) => Math.round(li.getBoundingClientRect().height))) : 0,
    firstRow: rows[0] ? title(rows[0]) : '',
    current: rows.filter((li) => li.querySelector('[aria-current="true"]')).map(title),
    h2: reader?.querySelector("#note-title")?.textContent.trim() ?? '',
    peek: new URL(location.href).searchParams.has('peek'),
    history: history.length,
  }
}

/** Column heads showing in `main`. */
function headsInPage() {
  return [...document.querySelectorAll('main [role=columnheader], main th')]
    .filter((el) => el.getBoundingClientRect().width > 1)
    .map((el) => el.textContent.trim())
}

/** Whether anything sticks out sideways: the document, and `main` (the app's shell clips, so the document alone never
 * scrolls). */
function overflowInPage() {
  const main = document.querySelector('main')
  return { doc: document.documentElement.scrollWidth, inner: innerWidth, main: main.scrollWidth, mainClient: main.clientWidth }
}

// ---- the run --------------------------------------------------------------------------------------------------------

// The address the pages are opened at. Nothing listens there and the browser never connects to it: every request is
// answered by `serve` below. An address on the machine itself, so the page is a secure context as on 127.0.0.1:7420.
const ORIGIN = 'http://127.0.0.1:59407'

let chromium
let build
let verifyConfig
let web
try {
  ;({ chromium } = await import('playwright-core'))
  ;({ build } = await import('vite'))
  ;({ verifyConfig, web } = await import('./config.mjs'))
} catch (err) {
  fatal(`npm packages are missing or could not be loaded (${why(err)}). Set up once with: ${SETUP}`)
}

// The page, built once: the app's index.html with the fake link (config.mjs), as the dev server of `npm run verify`
// serves it at a page's own path.
step('making a directory for the built page')
const out = scratch('lifeos-verify-notes-')
const site = path.join(out, 'site')
step('building the page')
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
const built = Date.now()
step('opening the Notes page')

const TYPES = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.json': 'application/json',
  '.svg': 'image/svg+xml', '.png': 'image/png', '.jpg': 'image/jpeg', '.ico': 'image/x-icon', '.woff2': 'font/woff2', '.woff': 'font/woff', '.ttf': 'font/ttf',
}
const elsewhere = []
/** Answers a request of the page from the built files. A path that is no file gets the app's page, as Vite's dev server
 * answers (the app's own requests to /api/… get it too, there and here). A request to any other address is refused. */
async function serve(route) {
  const url = new URL(route.request().url())
  try {
    if (url.origin !== ORIGIN) {
      elsewhere.push(url.origin)
      return await route.abort('blockedbyclient')
    }
    let file = path.join(site, decodeURIComponent(url.pathname))
    if (!file.startsWith(site + path.sep) || !fs.statSync(file, { throwIfNoEntry: false })?.isFile()) file = path.join(site, 'index.html')
    await route.fulfill({ status: 200, contentType: TYPES[path.extname(file)] ?? 'application/octet-stream', body: fs.readFileSync(file) })
  } catch (err) {
    // The tab closed while its request was being answered: nothing to answer. Anything else is the check's own fault.
    if (!/closed|Route is already handled/i.test(String(err?.message))) throw err
  }
}

step('starting the browser')
let browser
let closing = false
// How much processor time each of the browser's processes has used, the most seen per process: Remit's jail ends a
// process at 2 minutes.
const cpu = new Map()
try {
  browser = await chromium.launch({ headless: true })
  browser.on('disconnected', () => { if (!closing) fatal(`the browser ended on its own, while at: ${where()}: ${why(new Error('browser closed'))}`) })
  const session = await browser.newBrowserCDPSession()
  const sample = async () => {
    const { processInfo } = await session.send('SystemInfo.getProcessInfo')
    for (const p of processInfo) cpu.set(`${p.type} ${p.id}`, Math.max(cpu.get(`${p.type} ${p.id}`) ?? 0, p.cpuTime))
  }
  setInterval(() => void sample().catch(() => {}), 1000).unref()
} catch (err) {
  fatal(`headless Chromium did not start (browser launch refused or no browser: ${why(err)}). Set up once with: ${SETUP}`)
}
async function end() {
  closing = true
  await browser.close()
}

const pageErrors = []
const consoleErrors = []
/** A fresh tab (its own storage, so the Assistant column has its first width) at `size`, opened at `at` with the
 * scenario's notes and the Assistant at rest. */
async function openAt(size, at, scenario, { timeout = ACT_MS, dark = false } = {}) {
  const context = await browser.newContext({ viewport: size, deviceScaleFactor: size.width < 600 ? 2 : 1, hasTouch: size.width < 600, locale: 'en-US', timezoneId: 'Asia/Ho_Chi_Minh', colorScheme: dark ? 'dark' : 'light', serviceWorkers: 'block' })
  await context.route('**/*', serve)
  context.setDefaultTimeout(timeout)
  context.setDefaultNavigationTimeout(Math.max(timeout, LOAD_MS))
  const page = await context.newPage()
  page.on('pageerror', (e) => pageErrors.push(String(e.message)))
  page.on('console', (m) => { if (m.type() === 'error') consoleErrors.push(m.text()) })
  await page.goto(`${ORIGIN}${at}${at.includes('?') ? '&' : '?'}scenario=${scenario}&as=idle`)
  await page.locator('main h1').first().waitFor({ state: 'attached', timeout: Math.max(timeout, LOAD_MS) })
  await page.evaluate(() => document.fonts.ready)
  return page
}
/** The Notes page, drawn: its list (or its empty state) is there. */
async function open(size, scenario, query = '', opts = {}) {
  const page = await openAt(size, `/notes${query ? `?${query.replace(/^&/, '')}` : ''}`, scenario, opts)
  await ready(page, Math.max(opts.timeout ?? 0, LOAD_MS))
  return page
}
async function ready(page, timeout = LOAD_MS) {
  const main = page.locator('main')
  await main.getByText(LONG_TITLE).or(main.getByText(A_REVIEW)).or(main.getByText('No notes yet.')).first().waitFor({ timeout })
  await page.evaluate(() => document.fonts.ready)
}
/** A step on the way to a screenshot or the next assertion. A page that lacks what the step waits for (the page before
 * this design has no reader) is not a check that cannot run: the assertions around it say what is missing. */
const reach = (step) => step.catch(() => {})
const longBody = (page) => page.locator('p', { hasText: LONG_STARTS }).first()
const listOf = (page) => page.locator(LIST)
const row = (page, title) => listOf(page).locator('li', { hasText: title }).first()
/** The reader has the note titled `title`. */
const shown = (page, title) => page.locator(`${READER} #note-title`, { hasText: title }).waitFor()
async function openLong(page) {
  await page.locator('main').getByText(LONG_TITLE).first().click()
  await longBody(page).waitFor()
  await page.evaluate(() => document.fonts.ready)
}
async function shot(page, name) {
  if (!shots) return
  fs.mkdirSync(shots, { recursive: true })
  await page.screenshot({ path: path.join(shots, `${name}.png`) })
}
const composer = (page) => page.locator('textarea[aria-label^="Message"]').first()
const named = (page, re) => page.getByRole('button', { name: re }).or(page.getByRole('link', { name: re })).or(page.getByRole('radio', { name: re })).or(page.getByRole('tab', { name: re })).first()
const gapsOk = (g) => ({ ok: g.left <= MAX_GAP && g.right <= MAX_GAP, measured: `left ${g.left}px, right ${g.right}px (content ${g.surface}px of main ${g.main}px)`, limit: `each gap ≤ ${MAX_GAP}px` })
const near = (a, b, by) => Math.abs(a - b) <= by
/** The layout has come to rest: main and its sections kept their boxes for five frames in a row (five is a choice: the
 * widget grid sizes itself a frame or two after it has measured its width). */
const settled = (page) => page.evaluate(() => new Promise((resolve) => {
  const main = document.querySelector('main')
  let last = ''
  let same = 0
  const tick = () => {
    const now = `${main.scrollHeight}:${[...main.querySelectorAll('section')].map((s) => { const r = s.getBoundingClientRect(); return `${r.top},${r.height}` }).join('|')}`
    same = now === last ? same + 1 : 0
    last = now
    if (same >= 5) resolve()
    else requestAnimationFrame(tick)
  }
  tick()
}))
/** Page widgets that leave the panes less than their floor: the panes keep the floor and the page scrolls to them. */
async function crowded(size, widgets, name, shotName) {
  const page = await open(size, 'long', `widgets=${widgets}`)
  await check(name, async () => {
    await page.locator('main [data-widget-id]').first().waitFor()
    await settled(page)
    const m = await page.evaluate(([listSel, readerSel, minRow]) => {
      const main = document.querySelector('main')
      const list = document.querySelector(listSel)
      const reader = document.querySelector(readerSel)
      const rows = list.querySelector('ul')?.parentElement
      const widgets = document.querySelector('main section[aria-label="Custom widgets"]')
      const out = { main: main.clientHeight, scroll: main.scrollHeight, list: Math.round(list.getBoundingClientRect().height), reader: reader ? Math.round(reader.getBoundingClientRect().height) : null, rows: rows ? rows.clientHeight : 0, widgets: Math.round(widgets.getBoundingClientRect().height) }
      // Everything can be reached: the page scrolls down to the foot of the panes, the list scrolls to its last row,
      // and back at the top the widgets start in view.
      main.scrollTop = main.scrollHeight
      const foot = main.getBoundingClientRect().bottom
      out.footShort = Math.round(list.getBoundingClientRect().bottom - foot)
      if (rows) rows.scrollTop = rows.scrollHeight
      const last = list.querySelector('li:last-child')
      const l = last?.getBoundingClientRect()
      out.lastRow = !!l && !!rows && l.height >= minRow && l.top >= rows.getBoundingClientRect().top - 1 && l.bottom <= rows.getBoundingClientRect().bottom + 1 && l.bottom <= foot + 1
      main.scrollTop = 0
      if (rows) rows.scrollTop = 0
      out.top = Math.round(widgets.getBoundingClientRect().top - main.getBoundingClientRect().top)
      return out
    }, [LIST, READER, MIN_ROW])
    const ok = m.list >= FLOOR && (m.reader === null || m.reader >= FLOOR) && m.rows >= FLOOR - 2 * 48 && m.scroll > m.main && Math.abs(m.footShort) <= 1 && m.lastRow && m.top >= 0
    return { ok, measured: `widgets ${m.widgets}px in a ${m.main}px page; list pane ${m.list}px (rows area ${m.rows}px)${m.reader === null ? '' : `, reader pane ${m.reader}px`}; page scrolls ${m.scroll}px, to the panes' foot (${m.footShort}px off) and the last row ${m.lastRow ? 'in view' : 'out of reach'}`, limit: `panes ≥ ${FLOOR}px, rows area ≥ ${FLOOR - 2 * 48}px, the page scrolls to everything` }
  })
  await shot(page, shotName)
  await page.evaluate(() => { const main = document.querySelector('main'); main.scrollTop = main.scrollHeight })
  await shot(page, `${shotName}-scrolled`)
  await page.context().close()
}
const measure = (r) => `median ${r.median} characters a line (min ${r.min}, max ${r.max}, over ${r.lines} full lines; column ${r.column}px)`
const noHeads = async (page) => {
  const heads = await page.evaluate(headsInPage)
  return { ok: heads.length === 0, measured: heads.length ? `${heads.length} column head(s): ${heads.join(', ')}` : 'none', limit: '0 column heads' }
}

try {
  const words = fs.readFileSync(path.join(web, 'verify/longNote.md'), 'utf8').trim().split(/\s+/).length
  await check('fixture: the long note is long', () => ({ ok: words >= MIN_WORDS, measured: `${words} words`, limit: `≥ ${MIN_WORDS} words` }))

  // ---- layout, 1568×900 ----
  for (const assistant of [false, true]) {
    const tag = assistant ? 'wide, Assistant open' : 'wide'
    const q = assistant ? 'assistant=1' : ''
    const suffix = assistant ? '-assistant' : ''

    let page = await open(WIDE, 'many', q)
    // The reader fills a moment after the list; what follows reads both. (If it never fills, the assertions say so.)
    await shown(page, OTHER_NOTE).catch(() => {})
    await check(`layout [${tag}]: the list of many notes fills the page`, async () => gapsOk(await page.evaluate(gapsInPage)))
    await check(`layout [${tag}]: two panes, the list then the reader, as tall as the page`, async () => {
      const p = await page.evaluate(panesInPage, LIST)
      if (!p.list || !p.reader) return { ok: false, measured: `list ${p.list ? 'there' : 'missing'}, reader ${p.reader ? 'there' : 'missing'}`, limit: 'both panes' }
      const want = assistant ? { main: MAIN_BESIDE_ASSISTANT, list: LIST_WIDTH.narrow } : { main: MAIN, list: LIST_WIDTH.wide }
      const ok = near(p.main.w, want.main, 2) && near(p.list.w, want.list, 1) && near(p.list.x, p.main.x, 1) && near(p.reader.x, p.list.x + p.list.w, 1) && near(p.reader.x + p.reader.w, p.main.x + p.main.w, 1) && near(p.list.h, p.main.h, 2) && near(p.reader.h, p.main.h, 2)
      return { ok, measured: `main ${p.main.w}×${p.main.h}px; list ${p.list.w}px at its left edge; reader ${p.reader.w}px to its right edge; panes ${p.list.h}px and ${p.reader.h}px tall`, limit: `main ${want.main}±2, list ${want.list}±1, reader the rest, panes as tall as main ±2` }
    })
    await check(`layout [${tag}]: the page does not scroll, the list scrolls on its own`, async () => {
      const p = await page.evaluate(panesInPage, LIST)
      const moved = await page.evaluate((sel) => {
        const list = document.querySelector(sel)
        const scroller = [...list.querySelectorAll('*')].find((el) => /auto|scroll/.test(getComputedStyle(el).overflowY) && el.scrollHeight > el.clientHeight)
        const bar = document.querySelector('main section[aria-label="Note"]').firstElementChild
        const before = bar.getBoundingClientRect().y
        if (scroller) scroller.scrollTop = 600 // any distance: the list of 28 rows is about 1600px in a 737px pane
        return { scrolled: scroller?.scrollTop ?? 0, bar: bar.getBoundingClientRect().y - before, main: document.querySelector('main').scrollTop }
      }, LIST)
      return { ok: p.mainScroll === p.mainClient && moved.scrolled > 0 && moved.bar === 0 && moved.main === 0, measured: `main scroll height ${p.mainScroll}px in ${p.mainClient}px; list scrolled ${moved.scrolled}px, reader bar moved ${moved.bar}px`, limit: 'main does not scroll; the list does; the reader stays' }
    })
    if (!assistant) {
      await check('layout [wide]: with no ?peek= the first note is shown and marked, the address untouched', async () => {
        const p = await page.evaluate(panesInPage, LIST)
        return { ok: p.current.length === 1 && p.current[0] === OTHER_NOTE && p.firstRow === OTHER_NOTE && p.h2 === OTHER_NOTE && !p.peek, measured: `marked row: ${JSON.stringify(p.current)}; reader title: ${JSON.stringify(p.h2)}; ?peek ${p.peek ? 'in' : 'not in'} the address`, limit: 'the first row marked, its note in the reader, no ?peek=' }
      })
      await check('layout [wide]: picking a row replaces the address (no new history entry)', async () => {
        const third = 'Packing list for the Hà Giang loop'
        const before = await page.evaluate(() => history.length)
        await row(page, third).locator('a').click()
        await page.locator(`${READER} #note-title`, { hasText: third }).waitFor()
        const p = await page.evaluate(panesInPage, LIST)
        const peek = new URL(page.url()).searchParams.get('peek')
        return { ok: peek === 'note_packing-list-for-the-ha-giang-loop' && p.history === before && p.current.length === 1 && p.current[0] === third, measured: `?peek=${peek}; history ${before} → ${p.history}; marked row ${JSON.stringify(p.current)}`, limit: '?peek=note_<id>, history length unchanged, that row marked' }
      })
      await page.goto(page.url().replace(/&?peek=[^&]*/, ''))
      await reach(ready(page))
      await reach(shown(page, OTHER_NOTE))
      await shot(page, '1568x900-many')
    }
    await page.context().close()

    page = await open(WIDE, 'one', q)
    await check(`layout [${tag}]: a list of one note has no column heads`, () => noHeads(page))
    await check(`layout [${tag}]: one note is one row, marked, and shown in the reader`, async () => {
      await page.locator(`${READER} #note-title`).waitFor()
      const p = await page.evaluate(panesInPage, LIST)
      return { ok: p.rows === 1 && p.current.length === 1 && p.h2 === LONG_TITLE, measured: `${p.rows} row(s), ${p.current.length} marked, reader title ${JSON.stringify(p.h2)}`, limit: '1 row, marked, its note in the reader' }
    })
    if (!assistant) await shot(page, '1568x900-one')
    if (shots) {
      // The owner's screen: the only note, open.
      await openLong(page).catch(() => {})
      await shot(page, `1568x900-one-open${suffix}`)
    }
    await page.context().close()

    page = await open(WIDE, 'long', q)
    let r = null
    let g = null
    try {
      await openLong(page)
      r = await page.evaluate(readerInPage, [LONG_STARTS, OTHER_NOTE])
      g = await page.evaluate(gapsInPage)
    } catch (err) {
      info(`[${tag}] the long note did not open: ${String(err.message).split('\n')[0]}`)
    }
    const need = (fn) => () => {
      if (!r || !g) throw new Error('the long note did not open')
      return fn()
    }
    await check(`layout [${tag}]: with the long note open, the page is filled`, need(() => gapsOk(g)))
    await check(`layout [${tag}]: long note line measure`, need(() => ({ ok: r.median >= MEASURE.min && r.median <= MEASURE.max, measured: measure(r), limit: `median ${MEASURE.min}–${MEASURE.max} characters a line` })))
    await check(`layout [${tag}]: long note type`, need(() => ({ ok: r.font >= FONT && near(r.leading, LEADING, 0.1), measured: `${r.font}px, line height ${r.leading}px`, limit: `≥ ${FONT}px, line height ${LEADING}px (1.7)` })))
    await check(`layout [${tag}]: long note reading height`, need(() => ({ ok: r.height >= MIN_READING * r.viewport, measured: `${r.height}px, ${Math.round((100 * r.height) / r.viewport)}% of the ${r.viewport}px viewport`, limit: `≥ ${100 * MIN_READING}% of the viewport` })))
    await check(`layout [${tag}]: the open note is wider than the list beside it`, need(() => ({ ok: r.list === 0 || r.column > r.list, measured: r.list ? `note text column ${r.column}px, list row ${r.list}px` : `note text column ${r.column}px, no list beside it`, limit: 'note column > list row' })))
    if (assistant) await check(`layout [${tag}]: the open note keeps a readable column beside the Assistant`, need(() => ({ ok: r.column >= MIN_COLUMN_BESIDE_ASSISTANT, measured: `note text column ${r.column}px (main ${g.main}px)`, limit: `≥ ${MIN_COLUMN_BESIDE_ASSISTANT}px` })))
    else await check(`layout [${tag}]: the note's column is 56ch, centred in the reader`, need(() => ({ ok: near(r.column, COLUMN, 3) && r.offCentre !== null && Math.abs(r.offCentre) <= 2, measured: `text column ${r.column}px, ${r.offCentre}px off the reader's centre`, limit: `${COLUMN}±3px, within 2px of the centre` })))
    await check(`layout [${tag}]: the reader scrolls on its own`, need(async () => {
      const moved = await page.evaluate(([sel, starts]) => {
        const main = document.querySelector('main')
        const p = [...document.querySelectorAll('p')].find((el) => el.textContent.startsWith(starts))
        let scroller = p.parentElement
        while (scroller && !(/auto|scroll/.test(getComputedStyle(scroller).overflowY) && scroller.scrollHeight > scroller.clientHeight)) scroller = scroller.parentElement
        const field = document.querySelector(`${sel} input`)
        const before = field.getBoundingClientRect().y
        const tall = scroller ? scroller.scrollHeight > scroller.clientHeight : false
        if (scroller) scroller.scrollTop = 2000 // any distance past a screen: the long note is several thousand px
        return { inMain: !!scroller && scroller !== main && main.contains(scroller), tall, scrolled: scroller?.scrollTop ?? 0, field: field.getBoundingClientRect().y - before, mainScroll: main.scrollHeight, mainClient: main.clientHeight }
      }, [LIST, LONG_STARTS])
      return { ok: moved.inMain && moved.tall && moved.scrolled === 2000 && moved.field === 0 && moved.mainScroll === moved.mainClient, measured: `reader scrolled ${moved.scrolled}px, the list's search field moved ${moved.field}px, main scroll height ${moved.mainScroll}px in ${moved.mainClient}px`, limit: 'the reader scrolls 2000px, the list stays, main does not scroll' }
    }))
    if (r && g) info(`[${tag}] widths with the long note open: main ${g.main}px, content ${g.surface}px, list row ${r.list}px, note text column ${r.column}px`)
    await page.evaluate((starts) => {
      let s = [...document.querySelectorAll('p')].find((el) => el.textContent.startsWith(starts))?.parentElement
      while (s && !(/auto|scroll/.test(getComputedStyle(s).overflowY) && s.scrollHeight > s.clientHeight)) s = s.parentElement
      if (s) s.scrollTop = 0
    }, LONG_STARTS)
    await shot(page, `1568x900-long-open${suffix}`)
    await page.context().close()
  }

  // ---- 1568×900: a page widget above the panes, the note's own page, another page, the other looks ----
  {
    let page = await open(WIDE, 'long', 'widgets=1')
    await check('layout [wide]: a page widget sits above the panes, which keep the rest of the height', async () => {
      await page.locator('main').getByText('Reading queue').first().waitFor()
      await settled(page)
      const p = await page.evaluate(panesInPage, LIST)
      const w = await page.evaluate(() => document.querySelector('main section[aria-label="Custom widgets"]').getBoundingClientRect().bottom)
      return { ok: p.mainScroll === p.mainClient && p.list.y >= w - 1 && near(p.list.y + p.list.h, p.main.y + p.main.h, 2) && p.list.h >= FLOOR, measured: `widget ends at ${Math.round(w)}px, panes from ${p.list.y}px, ${p.list.h}px tall; main scroll height ${p.mainScroll}px in ${p.mainClient}px`, limit: `panes below the widget, down to the foot of main, ≥ ${FLOOR}px; main does not scroll` }
    })
    await shot(page, '1568x900-widget')
    await page.context().close()
    await crowded(WIDE, 8, 'layout [wide]: eight page widgets leave the panes their floor, and the page scrolls', '1568x900-widgets-8')
    await crowded(WIDE, 'tall', 'layout [wide]: one tall page widget leaves the panes their floor, and the page scrolls', '1568x900-widget-tall')
    await crowded(PHONE, 3, 'layout [phone]: three page widgets leave the list its floor, and the page scrolls', '390x844-widgets-3')

    page = await open(WIDE, 'long')
    await check('layout [wide]: a note arriving in the list does not change the note being read', async () => {
      await shown(page, LONG_TITLE)
      await longBody(page).waitFor()
      const at = (title) => page.evaluate(([readerSel, starts, t]) => {
        let s = [...document.querySelectorAll('p')].find((el) => el.textContent.startsWith(starts))?.parentElement
        while (s && !(/auto|scroll/.test(getComputedStyle(s).overflowY) && s.scrollHeight > s.clientHeight)) s = s.parentElement
        if (t === null && s) s.scrollTop = 1500 // any distance past a screen
        return { top: s?.scrollTop ?? -1, title: document.querySelector(`${readerSel} #note-title`)?.textContent.trim() ?? '' }
      }, [READER, LONG_STARTS, title])
      const before = await at(null)
      await page.evaluate(() => window.verify.addNote('Saved while reading'))
      await row(page, 'Saved while reading').waitFor()
      await settled(page)
      const after = await at('')
      const p = await page.evaluate(panesInPage, LIST)
      return { ok: before.top === 1500 && after.top === 1500 && after.title === LONG_TITLE && p.firstRow === 'Saved while reading' && p.current.length === 1 && p.current[0] === LONG_TITLE && !p.peek, measured: `new first row ${JSON.stringify(p.firstRow)}; reader title ${JSON.stringify(after.title)}, scrolled ${before.top}px → ${after.top}px; marked ${JSON.stringify(p.current)}; ?peek ${p.peek ? 'in' : 'not in'} the address`, limit: 'the same note, the same scroll position, its row still marked, no ?peek=' }
    })
    await page.context().close()

    page = await openAt(WIDE, `/doc/note_${LONG_ID}`, 'long')
    await check('layout [wide]: the note on its own page reads the same', async () => {
      await longBody(page).waitFor()
      await page.evaluate(() => document.fonts.ready)
      const r = await page.evaluate(readerInPage, [LONG_STARTS, OTHER_NOTE])
      return { ok: r.median >= MEASURE.min && r.median <= MEASURE.max && r.font === FONT && near(r.leading, LEADING, 0.1) && r.column <= COLUMN + 3, measured: `${measure(r)}, ${r.font}px, line height ${r.leading}px`, limit: `median ${MEASURE.min}–${MEASURE.max} characters a line, ${FONT}px, line height ${LEADING}px, column ≤ ${COLUMN}+3px` }
    })
    await shot(page, '1568x900-doc-page')
    await page.context().close()

    page = await openAt(WIDE, '/tasks', 'many')
    await check('guard [wide]: another page (Tasks) keeps the 1240px column', async () => {
      const box = await page.evaluate(() => {
        const el = document.querySelector('main > div')
        const cs = getComputedStyle(el)
        return { outer: Math.round(el.getBoundingClientRect().width), content: Math.round(el.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight)), main: document.querySelector('main').clientWidth }
      })
      return { ok: box.outer === 1240 && box.content === 1168, measured: `column ${box.outer}px, content box ${box.content}px, in a ${box.main}px main`, limit: 'column 1240px, content box 1168px (as before)' }
    })
    await page.context().close()

    if (shots) {
      page = await open(WIDE, 'many', 'q=garden')
      await reach(listOf(page).getByText('Buy seeds for the garden').first().waitFor())
      await reach(shown(page, LONG_TITLE))
      await shot(page, '1568x900-search')
      await page.context().close()
      page = await open(WIDE, 'many', 'view=reviews')
      await reach(shown(page, 'Morning brief, Wednesday'))
      await shot(page, '1568x900-reviews')
      await page.context().close()
      page = await open(WIDE, 'none')
      await shot(page, '1568x900-empty')
      await page.context().close()
      page = await open(WIDE, 'long', '', { dark: true })
      await reach(openLong(page))
      await shot(page, '1568x900-long-open-dark')
      await page.context().close()
      page = await open(PHONE, 'none')
      await shot(page, '390x844-empty')
      await page.context().close()
    }
  }

  // ---- layout, 390×844 ----
  {
    const fits = (o) => ({ ok: o.doc <= o.inner && o.main <= o.mainClient, measured: `document ${o.doc}px in a ${o.inner}px window, main ${o.main}px in ${o.mainClient}px`, limit: 'scroll width ≤ width' })
    let page = await open(PHONE, 'many')
    await check('layout [phone]: the list does not scroll sideways', async () => fits(await page.evaluate(overflowInPage)))
    await check('layout [phone]: the list is the page: no reader pane, rows a finger tall', async () => {
      const p = await page.evaluate(panesInPage, LIST)
      return { ok: !p.reader && !!p.list && near(p.list.w, PHONE.width, 1) && p.minRow >= MIN_ROW, measured: `reader pane ${p.reader ? 'in the page' : 'not in the page'}; list ${p.list?.w}px wide; ${p.rows} rows, the shortest ${p.minRow}px`, limit: `no reader pane, list ${PHONE.width}±1px, every row ≥ ${MIN_ROW}px` }
    })
    await shot(page, '390x844-many')
    await page.context().close()

    page = await open(PHONE, 'one')
    await check('layout [phone]: a list of one note has no column heads', () => noHeads(page))
    await shot(page, '390x844-one')
    await page.context().close()

    page = await open(PHONE, 'long')
    await check('kept [phone]: tapping a note opens it', async () => {
      await openLong(page)
      const at = new URL(page.url()).pathname
      return { ok: at === `/doc/note_${LONG_ID}`, measured: `note body showing at ${at}`, limit: `the note body shows at /doc/note_${LONG_ID}` }
    })
    await check('layout [phone]: the open long note does not scroll sideways', async () => fits(await page.evaluate(overflowInPage)))
    await check('layout [phone]: the open long note reads at full width', async () => {
      const r = await page.evaluate(readerInPage, [LONG_STARTS, OTHER_NOTE])
      return { ok: r.column >= PHONE_COLUMN.min && r.column <= PHONE_COLUMN.max && r.font === FONT && near(r.leading, LEADING, 0.1), measured: `text column ${r.column}px of ${PHONE.width}px, ${r.font}px, line height ${r.leading}px, ${measure(r)}`, limit: `column ${PHONE_COLUMN.min}–${PHONE_COLUMN.max}px, ${FONT}px, line height ${LEADING}px` }
    })
    await shot(page, '390x844-long-open')
    await check('kept [phone]: a way back from the open note to the list', async () => {
      const back = page.locator('main a[href^="/notes"]').or(page.getByRole('button', { name: /back|close|notes/i })).or(page.getByRole('link', { name: /back|close|all notes/i })).first()
      const label = (await back.textContent())?.trim() || (await back.getAttribute('aria-label')) || ''
      await back.click()
      await listOf(page).getByText(OTHER_NOTE).first().waitFor()
      const gone = !(await longBody(page).isVisible())
      return { ok: gone && new URL(page.url()).pathname === '/notes', measured: `"${label}" → ${new URL(page.url()).pathname}, list showing, note body ${gone ? 'gone' : 'still showing'}`, limit: 'back at /notes with the list' }
    })
    await page.context().close()

    page = await open(PHONE, 'many', 'q=garden')
    await check('kept [phone]: a note opened from a search leads back to the results', async () => {
      const hit = 'Meeting notes: garden club, November'
      await row(page, hit).locator('a').click()
      await page.waitForURL(/\/doc\//)
      const crumb = page.locator('main nav[aria-label="Breadcrumb"] a').first()
      const label = (await crumb.textContent())?.trim()
      await crumb.click()
      await row(page, hit).waitFor()
      const u = new URL(page.url())
      return { ok: label === 'Search' && u.pathname === '/notes' && u.searchParams.get('q') === 'garden', measured: `breadcrumb "${label}" → ${u.pathname}${u.search}, results showing`, limit: '"Search" → /notes?q=garden' }
    })
    await page.context().close()
  }

  // ---- what the page already does and must keep, 1568×900 ----
  {
    const page = await open(WIDE, 'many')
    const main = page.locator('main')
    const list = listOf(page)
    const query = () => new URL(page.url()).searchParams

    await check('kept: switch Notes ↔ Briefs & reviews', async () => {
      await named(main, /^Briefs (&|and) reviews/).click()
      await list.getByText(A_REVIEW).first().waitFor()
      const there = query().get('view')
      const notesHidden = !(await list.getByText(OTHER_NOTE).first().isVisible())
      const line = (await row(page, 'Morning brief, Wednesday').innerText()).split('\n').map((s) => s.trim()).filter(Boolean)
      await named(main, /^Notes/).click()
      await list.getByText(OTHER_NOTE).first().waitFor()
      const back = query().get('view')
      return { ok: there === 'reviews' && notesHidden && back !== 'reviews' && line[1] === 'Wednesday, November 25 · Brief', measured: `?view=${there} shows reviews${notesHidden ? '' : ' (notes still listed)'}, a row's second line ${JSON.stringify(line[1])}; back on Notes ?view=${back ?? '(none)'}`, limit: '?view=reviews with "day · kind" rows, then the notes again' }
    })

    await check('kept: search finds across kinds and clears', async () => {
      const box = main.locator('input[aria-label*="earch" i], input[placeholder*="earch" i], input[type=search]').first()
      await box.fill('garden')
      const hits = ['Buy seeds for the garden', 'Minh', 'Meeting notes: garden club, November', 'Morning brief, Wednesday']
      await list.getByText(hits[0]).first().waitFor()
      const q = query().get('q')
      const found = []
      for (const h of hits) if (await list.getByText(h, { exact: true }).first().isVisible()) found.push(h)
      const strays = await list.getByText(NO_GARDEN).first().isVisible()
      const heading = await list.getByText(/^Results for/).first().innerText()
      const snippet = (await row(page, hits[0]).innerText()).includes('Ask the garden club first')
      const lines = await page.evaluate((sel) => Math.max(...[...document.querySelectorAll(`${sel} li a > span:last-child > span:last-child`)].map((el) => Math.round(el.getBoundingClientRect().height / parseFloat(getComputedStyle(el).lineHeight)))), LIST)
      const clear = page.getByRole('button', { name: /clear/i }).first()
      if (await clear.isVisible()) await clear.click()
      else await box.fill('')
      await list.getByText(NO_GARDEN).first().waitFor()
      await page.waitForFunction(() => !new URL(location.href).searchParams.has('q'))
      const views = await named(main, /^Briefs (&|and) reviews/).isVisible()
      return { ok: q === 'garden' && found.length === hits.length && !strays && snippet && lines <= 2 && /^Results for “garden”/.test(heading) && views, measured: `?q=${q}: ${found.length} of ${hits.length} expected hits (task, person, note, review)${strays ? ', a note that does not match still listed' : ''}, ${JSON.stringify(heading.replace(/\s+/g, ' '))}, the match ${snippet ? 'shown' : 'not shown'} in the row on at most ${lines} lines; cleared → list and views back, ?q gone`, limit: 'all 4 hits with their match on ≤ 2 lines, ?q in the address, list back after clear' }
    })

    await check('kept: a row shows the keyboard focus', async () => {
      await named(main, /^Briefs (&|and) reviews/).focus()
      await page.keyboard.press('Tab')
      const f = await page.evaluate((sel) => {
        const el = document.activeElement
        const cs = getComputedStyle(el)
        return { row: !!el.closest(`${sel} li`), style: cs.outlineStyle, width: parseFloat(cs.outlineWidth), text: el.textContent.trim().slice(0, 40) }
      }, LIST)
      return { ok: f.row && f.style !== 'none' && f.width >= FOCUS_OUTLINE, measured: `Tab lands on ${f.row ? `the row “${f.text}”` : 'something else'}, outline ${f.style} ${f.width}px`, limit: `a row with a visible outline ≥ ${FOCUS_OUTLINE}px` }
    })

    await check('kept: New note drafts in the Assistant', async () => {
      await named(page.locator('#page-actions'), /^New note/).click()
      await page.waitForFunction(() => document.querySelector('textarea[aria-label^="Message"]')?.value.startsWith('Save a note'))
      return { ok: true, measured: `composer: ${JSON.stringify(await composer(page).inputValue())}`, limit: 'composer starts "Save a note"' }
    })
    await check('kept: Weekly review drafts in the Assistant', async () => {
      await named(page.locator('#page-actions'), /^Weekly review/).click()
      await page.waitForFunction(() => /weekly review/i.test(document.querySelector('textarea[aria-label^="Message"]')?.value ?? ''))
      return { ok: true, measured: `composer: ${JSON.stringify(await composer(page).inputValue())}`, limit: 'composer asks for the weekly review' }
    })

    await check('kept: click a note opens it, ?peek= in the address', async () => {
      await openLong(page)
      const peek = query().get('peek')
      return { ok: peek === `note_${LONG_ID}`, measured: `?peek=${peek}, note body showing`, limit: `?peek=note_${LONG_ID}` }
    })
    await check('kept: Edit drafts in the Assistant', async () => {
      await named(page.locator(READER), /^Edit/).click()
      await page.waitForFunction((id) => (document.querySelector('textarea[aria-label^="Message"]')?.value ?? '') === `Update note_${id}: `, LONG_ID)
      return { ok: true, measured: `composer: ${JSON.stringify(await composer(page).inputValue())}`, limit: 'composer names the note' }
    })
    await check('kept: Summarise drafts in the Assistant', async () => {
      await named(page.locator(READER), /^Summari[sz]e/).click()
      await page.waitForFunction((t) => (document.querySelector('textarea[aria-label^="Message"]')?.value ?? '').includes(t), LONG_TITLE)
      const value = await composer(page).inputValue()
      return { ok: true, measured: `composer: ${JSON.stringify(value.length > 90 ? value.slice(0, 90) + '…' : value)}`, limit: 'composer names the note by title' }
    })
    await check('kept: the chips only draft, nothing is sent', async () => {
      const said = await page.evaluate(() => window.calls.filter((c) => c[0] === 'say').length)
      return { ok: said === 0, measured: `${said} messages sent`, limit: '0 sent' }
    })

    await check('kept: Open page shows the note on its own page', async () => {
      const link = named(page.locator(READER), /open (the )?(full )?page|full page/i)
      const href = await link.getAttribute('href')
      await link.click()
      await page.waitForURL(/\/doc\//)
      await longBody(page).waitFor()
      const title = await page.locator('main h1', { hasText: LONG_TITLE }).first().isVisible()
      return { ok: title && href === `/doc/note_${LONG_ID}`, measured: `link ${href} → ${decodeURIComponent(new URL(page.url()).pathname)}: title ${title ? 'and body showing' : 'missing'}`, limit: '/doc/note_<id> with the title and body' }
    })
    await page.context().close()

    const again = await open(WIDE, 'many', 'view=reviews&peek=review_2026-11-23-weekly')
    await check('kept: a reload restores the view and the open review', async () => {
      await again.reload()
      await listOf(again).getByText('Morning brief, Friday').first().waitFor()
      await again.locator(READER).getByText('What went well').first().waitFor()
      const p = new URL(again.url()).searchParams
      const notesHidden = !(await listOf(again).getByText(OTHER_NOTE).first().isVisible())
      const marked = (await again.evaluate(panesInPage, LIST)).current
      return { ok: p.get('view') === 'reviews' && p.get('peek') === 'review_2026-11-23-weekly' && notesHidden && marked.length === 1 && marked[0] === A_REVIEW, measured: `after reload ?view=${p.get('view')}&peek=${p.get('peek')}: reviews listed, ${JSON.stringify(marked)} marked, the review's body showing`, limit: 'same view and open review' }
    })
    await again.context().close()
  }
} catch (err) {
  closing = true
  await browser.close().catch(() => {})
  fatal(`${why(err)}, while at: ${where()}`)
}

await end()
// An error on a page is a failure, whatever the assertions saw: one it threw, or one it logged with console.error.
// None is ignored: a run of the whole check on the harness logs no console error at all (measured at b5f4953).
await check('the pages threw and logged no errors', () => {
  const seen = [...new Set([...pageErrors.map((e) => `thrown: ${e}`), ...consoleErrors.map((e) => `console.error: ${e}`)])]
  return { ok: seen.length === 0, measured: seen.length ? `${pageErrors.length} thrown, ${consoleErrors.length} logged: ${seen.join(' | ')}` : 'none thrown, none logged', limit: '0 uncaught errors, 0 console errors' }
})
const secs = (ms) => Math.round(ms / 1000)
const busiest = [...cpu.entries()].sort((a, b) => b[1] - a[1])[0]
const own = process.cpuUsage()
info(`took ${secs(Date.now() - started)}s (building the page ${secs(built - started)}s); processor time: this process ${secs((own.user + own.system) / 1000)}s, the busiest of the browser's ${cpu.size} processes ${busiest ? `${Math.round(busiest[1])}s (${busiest[0].split(' ')[0]})` : 'not measured'}; ${elsewhere.length} requests to other addresses, all refused${elsewhere.length ? ` (${[...new Set(elsewhere)].join(', ')})` : ''}`)
if (shots) info(`screenshots in ${shots}`)
finish()
