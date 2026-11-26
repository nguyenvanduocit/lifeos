#!/usr/bin/env node
// `npm run verify:notes:contract`: what the Notes page is made of, without a browser. The app's own router and its
// Notes and Doc pages are rendered to HTML in this process (Vue's server renderer, loaded through Vite with no server
// listening) on the made-up notes of the fake link (notesData.ts), and the app's stylesheet is compiled and parsed.
// One line per assertion (✓/✗ name, what was found, the limit); exit 1 when any assertion fails, exit 2 when the check
// itself could not run (one line says what was refused or missing).
//
// It is the check for a place that cannot start a browser (Remit's check jail may be one: see notes.mjs). It needs
// node and web/node_modules, opens no socket, starts no other program, watches no file, and writes only under
// os.tmpdir().
//
// What it CANNOT see, and `npm run verify:notes` (a real Chromium) does: anything drawn. No pixel is laid out here, so
// it does not know how wide a pane or a line of text is (characters per line), whether the page is filled, whether
// anything scrolls or sticks out, what a click, a key or a reload does, or how the page looks. It reads markup and CSS
// rules: that the list and the reader are there, that the classes on them have the rules the layout rests on, where
// links lead and which words a chip drafts.
// It also cannot see, though they are in the markup or the stylesheet:
// - the order the panes are drawn in (a class such as order-2 on the list draws it after the reader);
// - that picking a row replaces the entry in the history (the link's `replace`);
// - a rule that reaches a pane or the note's text by a selector that names neither a class on that element nor
//   .reader or .prose-life (`article p { font-size: … }`): only the rules named in "reading CSS" below are looked at.
// Two things differ from a browser, by the server renderer's nature:
// - a page reacts to nothing after its first read, so the note the reader shows with no ?peek= (the first row) is not
//   seen here: the pages are rendered with ?peek= naming the note;
// - the app's shell (App.vue) needs a window to set itself up, so it is not rendered: the class of its page container
//   is read from its template.
// Three browser-only pieces are swapped while rendering: an element's width (the page width asked for, so Notes shows
// one pane or two), the router's history (kept in memory) and DOMPurify (which needs a document; the notes are the
// check's own). The icons (@lucide/vue) are loaded as the package's one-file build: Vite would load its 2,120 separate
// files, which took 36s on a busy disk where the one file took 2.5s (measured, load 38 on 10 cores).
import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { A_REVIEW, A_REVIEW_ID, LONG_ID, LONG_STARTS, LONG_TITLE, begin, check, fatal, finish, info, scratch, started, step, where, why } from './run.mjs'

const SETUP = 'cd web && npm ci'
// The whole run. Remit's jail ends a check after 5 minutes (remit crates/shipping/src/repochecks.rs:22); this one says
// where it was before that.
const RUN_MS = 4 * 60_000

begin('verify:notes:contract', RUN_MS)

// ---- the page's sizes (docs/design.md, "List + reader") -------------------------------------------------------------
// A page wide enough for two panes (`main` at 1568px with the Assistant closed, as notes.mjs measures) and one that is
// not; Notes.vue shows the reader from 768px (48rem).
const TWO_PANES = 1310
const ONE_PANE = 390
const LIST_WIDTH = { '48rem': '300px', '64rem': '340px' }
// The panes' floor, the Reader column and the shell's column, as the design has them.
const FLOOR = '324px'
const READER = { 'max-width': '56ch', 'font-size': '16px', 'line-height': '1.7' }
const SHELL_COLUMN = '1240px'

// ---- the names only a browser has, which the fake link and the app's modules read when they load --------------------
const store = () => {
  const kept = new Map()
  return { getItem: (k) => kept.get(k) ?? null, setItem: (k, v) => void kept.set(k, String(v)), removeItem: (k) => void kept.delete(k), clear: () => kept.clear() }
}
for (const [name, value] of Object.entries({ window: globalThis, location: { search: '?as=idle&still=1' }, sessionStorage: store(), localStorage: store() })) {
  Object.defineProperty(globalThis, name, { value, configurable: true, writable: true })
}

let createServer
let parseHtml
let parseSfc
let postcss
let verifyConfig
let web
try {
  ;({ createServer } = await import('vite'))
  ;({ parse: parseHtml } = await import('@vue/compiler-dom'))
  ;({ parse: parseSfc } = await import('@vue/compiler-sfc'))
  ;({ default: postcss } = await import('postcss'))
  ;({ verifyConfig, web } = await import('./config.mjs'))
} catch (err) {
  fatal(`npm packages are missing or could not be loaded (${why(err)}). Set up once with: ${SETUP}`)
}

// The swaps (see the top): a source file's import of one of these packages gets the package as it is, but the one
// name; of the icons, the package's one-file build.
const ONE_FILE = '@lucide/vue'
const SWAPS = {
  [ONE_FILE]: [],
  '@vueuse/core': ['useElementSize', "import { ref as contractRef } from 'vue'\nexport function useElementSize() { return { width: contractRef(globalThis.contractWidth ?? 0), height: contractRef(0), stop() {} } }"],
  'vue-router': ['createWebHistory', "import { createMemoryHistory as contractHistory } from 'vue-router'\nexport const createWebHistory = contractHistory"],
  dompurify: ['default', 'export default { sanitize: (html) => html }'],
}
const IMPORTS = new RegExp(`from\\s+(['"])(${Object.keys(SWAPS).join('|')})\\1`, 'g')
const swaps = {
  name: 'contract-swaps',
  enforce: 'pre',
  transform(code, id) {
    if (!id.startsWith(web + path.sep) || id.includes('/node_modules/') || !code.match(IMPORTS)) return null
    return { code: code.replace(IMPORTS, "from '/@contract/$2'"), map: null }
  },
  resolveId: (source) => (source.startsWith('/@contract/') ? `\0contract:${source.slice('/@contract/'.length)}` : null),
  async load(id) {
    if (!id.startsWith('\0contract:')) return null
    const name = id.slice('\0contract:'.length)
    if (name === ONE_FILE) {
      const from = path.join(web, 'package.json')
      const names = Object.keys(createRequire(from)(name)).filter((n) => /^[A-Za-z_$][\w$]*$/.test(n) && n !== 'default')
      return `import { createRequire } from 'node:module'\nexport const { ${names.join(', ')} } = createRequire(${JSON.stringify(from)})(${JSON.stringify(name)})`
    }
    const [swapped, code] = SWAPS[name]
    if (swapped === 'default') return code
    const names = Object.keys(await import(name)).filter((n) => n !== 'default' && n !== swapped)
    return `export { ${names.join(', ')} } from '${name}'\n${code}`
  },
}

step('making a directory for Vite’s cache')
const out = scratch('lifeos-verify-contract-')
step('starting Vite without a server')
let vite
let render
let compiling
let css
try {
  const base = verifyConfig()
  vite = await createServer({
    ...base,
    plugins: [swaps, ...base.plugins],
    configFile: false,
    cacheDir: path.join(out, 'cache'),
    logLevel: 'silent',
    appType: 'custom',
    // No HTTP server, no WebSocket, no file watcher, no scan for packages to prepare: nothing listens and nothing is
    // started.
    server: { middlewareMode: true, hmr: false, ws: false, watch: null },
    optimizeDeps: { noDiscovery: true, include: [] },
  })
  step('loading the app’s router and pages')
  ;({ render } = await vite.ssrLoadModule('/verify/contractEntry.ts'))
  // The stylesheet compiles while the pages render: both mostly wait for files.
  compiling = vite.ssrLoadModule('/src/style.css?inline').then((m) => m.default)
  compiling.catch(() => {})
} catch (err) {
  await vite?.close().catch(() => {})
  fatal(`${why(err)}, while at: ${where()}`)
}

// ---- reading markup ------------------------------------------------------------------------------------------------
const ELEMENT = 1
const TEXT = 2
const ATTRIBUTE = 6
const DIRECTIVE = 7
/** HTML (or a Vue template) as a tree, by Vue's own parser. */
const parentOf = new WeakMap()
function tree(html) {
  const root = parseHtml(html, { comments: false, whitespace: 'preserve', onError() {} })
  const walk = (node) => {
    for (const child of node.children ?? []) {
      if (child.type !== ELEMENT) continue
      parentOf.set(child, node)
      walk(child)
    }
  }
  walk(root)
  return root
}
/** `el` and the elements around it, outwards: up to `top` (included) when given, else to the page's root. */
function upFrom(el, top) {
  const chain = []
  for (let at = el; at && at.type === ELEMENT; at = parentOf.get(at)) {
    chain.push(at)
    if (at === top) break
  }
  return chain
}
const named = (el) => `<${el.tag}${attr(el, 'aria-label') ? ` "${attr(el, 'aria-label')}"` : classes(el)[0] ? `.${classes(el)[0]}` : ''}>`
function* under(node) {
  for (const child of node?.children ?? []) {
    if (child.type !== ELEMENT) continue
    yield child
    yield* under(child)
  }
}
const all = (node, test) => [...under(node)].filter(test)
const first = (node, test) => all(node, test)[0] ?? null
const attr = (el, name) => {
  const a = el?.props.find((p) => p.type === ATTRIBUTE && p.name === name)
  return a ? a.value?.content ?? '' : null
}
const classes = (el) => (attr(el, 'class') ?? '').split(/\s+/).filter(Boolean)
const raw = (node) => (node.type === TEXT ? node.content : (node.children ?? []).map(raw).join(''))
const text = (node) => (node ? raw(node).replace(/\s+/g, ' ').trim() : '')
const tagged = (...tags) => (el) => tags.includes(el.tag)
const labelled = (tag, label) => (el) => el.tag === tag && attr(el, 'aria-label') === label
const at = (href) => new URL(href ?? '', 'http://page')
const show = (v) => JSON.stringify(v)

const listOf = (page) => first(page, labelled('section', 'Notes list'))
const readerOf = (page) => first(page, labelled('section', 'Note'))
/** The list's rows: where each leads, its title, its second line, whether it is marked as the open one. */
function rowsOf(page) {
  const list = listOf(page)
  return all(list, tagged('li')).map((li) => {
    const link = first(li, tagged('a'))
    const lines = all(link, (el) => el.tag === 'span' && !first(el, tagged('span', 'svg')) && text(el)).map(text)
    return { href: attr(link, 'href'), title: lines[0] ?? '', sub: lines[1] ?? '', marked: attr(link, 'aria-current') === 'true' }
  })
}
/** What makes a list a table: a table, a head cell, or the roles of one. */
const tableParts = (page) => all(page, (el) => ['table', 'thead', 'th'].includes(el.tag) || ['table', 'columnheader', 'grid'].includes(attr(el, 'role') ?? '')).map((el) => `<${el.tag}${attr(el, 'role') ? ` role=${attr(el, 'role')}` : ''}>${el.tag === 'th' ? text(el) : ''}`)
/** The Ask chips in `node`, by the name each button carries. */
const chipsIn = (node) => all(node, (el) => el.tag === 'button' && (attr(el, 'aria-label') ?? '').endsWith('drafts it in the Assistant')).map((el) => attr(el, 'aria-label').replace(/ · drafts it in the Assistant$/, ''))

// ---- the pages ------------------------------------------------------------------------------------------------------
/** The page at `at`, rendered `width` px wide on the notes of `scenario`: its tree, the header actions', and its chips. */
async function page(address, { scenario = 'many', width = TWO_PANES } = {}) {
  step(`rendering ${address} (${scenario}, ${width}px)`)
  globalThis.contractWidth = width
  const r = await render(address, scenario)
  return { ...r, body: tree(r.html), header: tree(r.actions) }
}
const LONG = `note_${LONG_ID}`
let wide
let one
let narrow
let reviews
let found
let own
let empty
try {
  wide = await page(`/notes?peek=${LONG}`)
  one = await page(`/notes?peek=${LONG}`, { scenario: 'one' })
  narrow = await page('/notes', { width: ONE_PANE })
  reviews = await page(`/notes?view=reviews&peek=review_${A_REVIEW_ID}`)
  found = await page(`/notes?q=garden&peek=${LONG}`)
  own = await page(`/doc/${LONG}`)
  empty = await page('/notes', { scenario: 'none' })
} catch (err) {
  await vite.close().catch(() => {})
  fatal(`a page could not be rendered: ${why(err)}, while at: ${where()}`)
}
const rendered = Date.now()

// ---- reading CSS ---------------------------------------------------------------------------------------------------
/** Every declaration of the compiled stylesheet, by the selector it is under, with the at-rules around it. What the
 * assertions look at of it, and nothing else:
 * - the rules of a selector they name (`.reader`, `.max-w-[1240px]`…);
 * - the rules of a class on an element (`rulesOf`): those whose selector is the class alone or the class and a
 *   pseudo-class or variant condition after it (`.hidden`, `.hover\:x:hover`);
 * - every rule whose selector names .reader or .prose-life (`naming`). */
const declared = new Map()
const everyRule = []
step('compiling and parsing the app’s stylesheet')
try {
  css = await compiling
  if (typeof css !== 'string' || !css) throw new Error('src/style.css compiled to nothing')
  postcss.parse(css).walkDecls((d) => {
    let selectors
    const within = []
    for (let p = d.parent; p && p.type !== 'root'; p = p.parent) {
      if (p.type === 'rule') selectors ??= p.selectors
      else if (p.type === 'atrule') within.push(`${p.name} ${p.params}`.replace(/\s+/g, ' ').trim())
    }
    for (const s of selectors ?? []) {
      const key = s.replace(/\\(.)/g, '$1').replace(/\s+/g, ' ').trim()
      if (!declared.has(key)) declared.set(key, [])
      const entry = { selector: key, written: s.trim(), prop: d.prop, value: d.value.replace(/\s+/g, ' ').replace(/,\s*/g, ',').trim(), important: !!d.important, within, layered: within.some((w) => w.startsWith('layer')) }
      declared.get(key).push(entry)
      everyRule.push(entry)
    }
  })
} catch (err) {
  await vite.close().catch(() => {})
  fatal(`the app’s stylesheet could not be compiled and parsed: ${why(err)}`)
}
/** The value `selector` gives `prop` (the last one written), where `inside` matches one of the at-rules around it. */
const rule = (selector, prop, inside) => (declared.get(selector) ?? []).filter((d) => d.prop === prop && (!inside || d.within.some((w) => inside.test(w)))).at(-1)
const valueOf = (selector, prop, inside) => rule(selector, prop, inside)?.value ?? 'no rule'
const layered = (selector, prop) => (rule(selector, prop)?.within ?? []).filter((w) => w.startsWith('layer'))
/** The declarations a class brings to the element it is on. */
function rulesOf(className) {
  const written = `.${className.replace(/[^a-zA-Z0-9_-]/g, (c) => `\\${c}`)}`
  return everyRule.filter((r) => r.written === written || (r.written.startsWith(written) && !/[\w\\-]/.test(r.written[written.length])))
}
/** The declarations of every rule whose selector names the class .reader or .prose-life. */
const naming = everyRule.filter((r) => /\.(reader|prose-life)(?![\w-])/.test(r.selector))
const say = (r) => `${r.selector} { ${r.prop}: ${r.value}${r.important ? ' !important' : ''} }${r.layered ? '' : ' (outside any @layer)'}`
// What sets the size of the note's text: the three properties of the Reader column (docs/design.md, "Reader").
const TYPE = ['max-width', 'font-size', 'line-height']
// The rules that may set them for .reader and .prose-life: the column, the text's line height (1.6 anywhere, 1.7 in the
// column), and a note's headings and code, sized against the text around them (src/style.css).
const TYPE_RULES = {
  '.prose-life': { 'line-height': '1.6' },
  '.reader': { 'max-width': READER['max-width'], 'font-size': READER['font-size'] },
  '.reader.prose-life': { 'line-height': READER['line-height'] },
  '.reader .prose-life': { 'line-height': READER['line-height'] },
  '.prose-life h1': { 'font-size': '1.15em' },
  '.prose-life h2': { 'font-size': '1.05em' },
  '.prose-life code': { 'font-size': '0.9em' },
  '.prose-life pre': { 'font-size': '0.85em' },
}
/** What on the way from the column to the note's text would undo the column's rules: a style attribute naming one of
 * the three properties; a class that sets one of them with !important or outside any @layer (either beats or ties
 * with the column's rules, which are outside any layer); below the column, a class that sets font-size at all (the
 * text inherits its 16px from the column). */
function undoing(column, body) {
  const found = []
  for (const el of upFrom(body, column)) {
    const style = attr(el, 'style') ?? ''
    if (TYPE.some((prop) => style.includes(prop))) found.push(`${named(el)} style="${style}"`)
    for (const c of classes(el)) {
      if (c === 'reader' || c === 'prose-life') continue
      for (const r of rulesOf(c)) {
        if (!TYPE.includes(r.prop)) continue
        if (r.important || !r.layered || (el !== column && r.prop === 'font-size')) found.push(`${named(el)} class ${c}: ${say(r)}`)
      }
    }
  }
  return found
}
/** What hides `el`: on it or on anything around it, a `hidden` attribute, a style with display: none or visibility:
 * hidden, or a class whose rule declares either (under any condition: a variant such as md:hidden counts). */
function hiding(el) {
  const found = []
  for (const at of upFrom(el)) {
    if (attr(at, 'hidden') !== null) found.push(`${named(at)} hidden`)
    const style = (attr(at, 'style') ?? '').replace(/\s+/g, '')
    if (/display:none|visibility:hidden/.test(style)) found.push(`${named(at)} style="${attr(at, 'style')}"`)
    for (const c of classes(at)) {
      for (const r of rulesOf(c)) if ((r.prop === 'display' && r.value === 'none') || (r.prop === 'visibility' && r.value === 'hidden')) found.push(`${named(at)} class ${c}: ${r.prop}: ${r.value}`)
    }
  }
  return found
}
const read = Date.now()

// ---- the shell ------------------------------------------------------------------------------------------------------
await check('shell: only the Notes route asks for the whole canvas', () => ({
  ok: wide.fills.length === 1 && wide.fills[0] === '/notes',
  measured: `routes with meta.fill: ${show(wide.fills)}`,
  limit: '["/notes"]',
}))
await check('shell: a page that fills has no 1240px column, another page keeps it', () => {
  const template = parseSfc(fs.readFileSync(path.join(web, 'src/App.vue'), 'utf8')).descriptor.template?.content ?? ''
  const box = first(first(tree(template), tagged('main')), (el) => classes(el).includes('@container'))
  if (!box) return { ok: false, measured: 'no @container element in <main> of App.vue', limit: 'the page container' }
  const bound = box.props.find((p) => p.type === DIRECTIVE && p.name === 'bind' && p.arg?.content === 'class')?.exp?.content
  const classFor = (fill) => [...classes(box), ...String(bound ? new Function('route', `return (${bound})`)({ meta: { fill } }) : '').split(/\s+/)].filter(Boolean)
  const cap = `max-w-[${SHELL_COLUMN}]`
  const [filling, other] = [classFor(true), classFor(false)]
  const means = valueOf(`.${cap}`, 'max-width')
  return {
    ok: !filling.includes(cap) && other.includes(cap) && means === SHELL_COLUMN,
    measured: `container of a fill page: ${show(filling.join(' '))}; of another page: ${show(other.join(' '))}; .${cap} is max-width ${means}`,
    limit: `${cap} on other pages only, and it means max-width ${SHELL_COLUMN}`,
  }
})

// ---- two panes ------------------------------------------------------------------------------------------------------
await check('panes: a list region and a reader region, side by side in one grid', () => {
  const list = listOf(wide.body)
  const reader = readerOf(wide.body)
  const grid = first(wide.body, (el) => el.children.includes(list) && el.children.includes(reader))
  const order = grid ? grid.children.filter((c) => c.type === ELEMENT).map((c) => attr(c, 'aria-label')) : []
  return {
    ok: !!list && !!reader && !!grid && classes(grid).includes('grid') && order[0] === 'Notes list' && order[1] === 'Note' && order.length === 2,
    measured: `list region ${list ? 'there' : 'missing'}, reader region ${reader ? 'there' : 'missing'}; ${grid ? `their parent's children: ${show(order)}, classes ${show(classes(grid).join(' '))}` : 'no element holds both'}`,
    limit: 'section "Notes list" then section "Note", the only children of a grid',
  }
})
const gridOf = (p) => first(p.body, (el) => el.children.includes(listOf(p.body)))
await check('panes: the list column is 300px from a 48rem page and 340px from a 64rem one, the reader takes the rest', () => {
  const grid = gridOf(wide)
  const steps = Object.entries(LIST_WIDTH).map(([step, width]) => {
    const name = classes(grid).find((c) => c.startsWith(`@min-[${step}]:grid-cols-`))
    return { step, width, name, value: name ? valueOf(`.${name}`, 'grid-template-columns', new RegExp(`^container .*${step}`)) : 'no class' }
  })
  const single = classes(grid).includes('grid-cols-1') ? valueOf('.grid-cols-1', 'grid-template-columns') : 'no class'
  return {
    ok: steps.every((s) => s.value === `${s.width} minmax(0,1fr)`) && single === 'repeat(1,minmax(0,1fr))',
    measured: `${steps.map((s) => `from ${s.step}: ${s.value}`).join('; ')}; below: ${single}`,
    limit: `${Object.entries(LIST_WIDTH).map(([step, width]) => `from ${step}: ${width} minmax(0,1fr) in a container rule`).join('; ')}; below: one column`,
  }
})
await check('panes: they keep a floor under page widgets', () => {
  const grid = gridOf(wide)
  const name = classes(grid).find((c) => c.startsWith('min-h-'))
  const value = name ? valueOf(`.${name}`, 'min-height') : 'no min-h class'
  return { ok: value === FLOOR && classes(grid).includes('flex-1'), measured: `panes' grid: ${name ?? 'no min-h class'} is min-height ${value}${classes(grid).includes('flex-1') ? ', and it takes the height left (flex-1)' : ', and no flex-1'}`, limit: `min-height ${FLOOR}, flex-1` }
})
await check('list: no table, no column heads (many notes)', () => {
  const parts = tableParts(wide.body)
  return { ok: parts.length === 0 && rowsOf(wide.body).length > 1, measured: `${parts.length ? `table parts: ${parts.join(' ')}` : 'no table parts'}; ${rowsOf(wide.body).length} rows in a list`, limit: '0 table parts, the notes as list rows' }
})
await check('list: with two panes a row opens its note in the reader (?peek=)', () => {
  const rows = rowsOf(wide.body)
  const peeks = rows.map((r) => (at(r.href).pathname === '/notes' ? at(r.href).searchParams.get('peek') : null))
  const good = peeks.filter((p) => /^note_[a-z0-9-]+$/.test(p ?? ''))
  return {
    ok: rows.length === 28 && good.length === rows.length && new Set(good).size === rows.length && peeks.includes(LONG),
    measured: `${rows.length} rows; ${good.length} lead to /notes?peek=note_<id>, ${new Set(good).size} different; first: ${rows[0]?.href}`,
    limit: '28 rows, each to /notes?peek=<kind>_<id>, its own',
  }
})
await check('list: the open note’s row is marked, and only it', () => {
  const marked = rowsOf(wide.body).filter((r) => r.marked).map((r) => r.title)
  return { ok: marked.length === 1 && marked[0] === LONG_TITLE, measured: `marked: ${show(marked)}`, limit: `only ${show(LONG_TITLE)}` }
})
await check('list: the search field and the view switch are at its top', () => {
  const list = listOf(wide.body)
  const field = first(list, (el) => el.tag === 'input' && attr(el, 'aria-label') === 'Search the vault')
  const type = field ? attr(field, 'type') ?? 'text' : null
  const off = field ? ['disabled', 'readonly'].filter((a) => attr(field, a) !== null) : []
  const views = all(first(list, (el) => attr(el, 'role') === 'radiogroup'), (el) => attr(el, 'role') === 'radio').map((el) => `${text(el)}${attr(el, 'aria-checked') === 'true' ? ' (chosen)' : ''}`)
  return { ok: !!field && ['text', 'search'].includes(type) && off.length === 0 && views.length === 2 && views[0] === 'Notes 28 (chosen)' && views[1] === 'Briefs & reviews 7', measured: `search field ${field ? `<input type=${type}${off.map((a) => ` ${a}`).join('')}>` : 'missing'}; views: ${show(views)}`, limit: 'a text or search input, not disabled or read-only; Notes 28 (chosen), Briefs & reviews 7' }
})
await check('shown: no class, attribute or style hides the panes or what they hold (two panes, a narrow page, the own page)', () => {
  const parts = [
    ['list region', listOf(wide.body)],
    ['rows', first(listOf(wide.body), tagged('ul'))],
    ['search field', first(listOf(wide.body), tagged('input'))],
    ['view switch', first(listOf(wide.body), (el) => attr(el, 'role') === 'radiogroup')],
    ['reader region', readerOf(wide.body)],
    ['note text', first(readerOf(wide.body), (el) => classes(el).includes('prose-life'))],
    ['list region (narrow)', listOf(narrow.body)],
    ['rows (narrow)', first(listOf(narrow.body), tagged('ul'))],
    ['note text (own page)', first(own.body, (el) => classes(el).includes('prose-life'))],
  ]
  const missing = parts.filter(([, el]) => !el).map(([name]) => name)
  const hidden = parts.filter(([, el]) => el).flatMap(([name, el]) => hiding(el).map((h) => `${name}: ${h}`))
  return {
    ok: missing.length === 0 && hidden.length === 0,
    measured: `${parts.length - missing.length} of ${parts.length} parts found${missing.length ? ` (missing: ${missing.join(', ')})` : ''}; ${hidden.length ? `hidden by: ${[...new Set(hidden)].join('; ')}` : 'on them and around them no hidden attribute, no display: none or visibility: hidden in a style or in a class\'s rule'}`,
    limit: 'all 9 parts there; nothing on or around them that hides',
  }
})

// ---- the reader -----------------------------------------------------------------------------------------------------
await check('reader: the note is in the Reader column: its title, its length, its text', () => {
  const column = first(readerOf(wide.body), (el) => classes(el).includes('reader'))
  const title = text(first(column, (el) => attr(el, 'id') === 'note-title'))
  const body = first(column, (el) => classes(el).includes('prose-life'))
  const words = text(column).match(/(\d+) words/)?.[1]
  return {
    ok: !!column && title === LONG_TITLE && !!body && text(body).startsWith(LONG_STARTS) && Number(words) >= 1500,
    measured: `${column ? 'a .reader element in the reader region' : 'no .reader element in the reader region'}; title ${show(title)}; ${words ?? 'no'} words; text ${body ? `starts ${show(text(body).slice(0, 24))}` : 'missing'}`,
    limit: `.reader holding the title, "<n> words" and the note's text`,
  }
})
await check('css: the rules .reader and .reader .prose-life say 16px, at most 56ch, line-height 1.7, outside any @layer', () => {
  const got = {
    'max-width': valueOf('.reader', 'max-width'),
    'font-size': valueOf('.reader', 'font-size'),
    'line-height': valueOf('.reader .prose-life', 'line-height'),
    'line-height (on the text itself)': valueOf('.reader.prose-life', 'line-height'),
  }
  const layers = [...layered('.reader', 'max-width'), ...layered('.reader', 'font-size'), ...layered('.reader .prose-life', 'line-height')]
  return {
    ok: got['max-width'] === READER['max-width'] && got['font-size'] === READER['font-size'] && got['line-height'] === READER['line-height'] && got['line-height (on the text itself)'] === READER['line-height'] && layers.length === 0,
    measured: `.reader: max-width ${got['max-width']}, font-size ${got['font-size']}; .reader .prose-life: line-height ${got['line-height']}; .reader.prose-life: line-height ${got['line-height (on the text itself)']}; ${layers.length ? `inside ${layers.join(', ')}` : 'outside any @layer'}`,
    limit: `max-width ${READER['max-width']}, font-size ${READER['font-size']}, line-height ${READER['line-height']}, outside any @layer`,
  }
})
await check('css: of the rules whose selector names .reader or .prose-life, only the known ones set max-width, font-size or line-height', () => {
  const setting = naming.filter((r) => TYPE.includes(r.prop))
  const other = setting.filter((r) => TYPE_RULES[r.selector]?.[r.prop] !== r.value || r.important || r.layered)
  return {
    ok: setting.length > 0 && other.length === 0,
    measured: `${naming.length} declarations under such selectors, ${setting.length} set one of the three; ${other.length ? `not known: ${other.map(say).join('; ')}` : 'all known'}`,
    limit: `only ${Object.entries(TYPE_RULES).map(([sel, d]) => `${sel} { ${Object.entries(d).map(([k, v]) => `${k}: ${v}`).join('; ')} }`).join(', ')}, none !important, none in a layer`,
  }
})
await check('reader: between the column and the note’s text, no class or style sets those three against the column’s rules (Notes, own page)', () => {
  const pages = [['Notes', readerOf(wide.body)], ['own page', own.body]].map(([name, within]) => {
    const column = first(within, (el) => classes(el).includes('reader'))
    const body = column && (classes(column).includes('prose-life') ? column : first(column, (el) => classes(el).includes('prose-life')))
    return { name, column, body, chain: body ? upFrom(body, column).reverse().map(named).join(' ') : '', found: body ? undoing(column, body) : [] }
  })
  const lost = pages.filter((p) => !p.body).map((p) => p.name)
  const found = pages.flatMap((p) => p.found.map((f) => `${p.name}: ${f}`))
  return {
    ok: lost.length === 0 && found.length === 0,
    measured: `${pages.map((p) => `${p.name}: ${p.body ? p.chain : 'no .reader column with the text in it'}`).join('; ')}; ${found.length ? `against the rules: ${found.join('; ')}` : 'on these elements no style naming the three, no class setting one with !important or outside a layer, no class setting font-size below the column'}`,
    limit: 'on the column, the text and the elements between: no style attribute naming them; no class setting one !important or outside any @layer; below the column no class setting font-size',
  }
})
await check('reader: Open page leads to the note’s own page', () => {
  const link = first(readerOf(wide.body), (el) => el.tag === 'a' && text(el) === 'Open page')
  return { ok: attr(link, 'href') === `/doc/${LONG}`, measured: link ? `"Open page" → ${attr(link, 'href')}` : 'no "Open page" link in the reader region', limit: `/doc/${LONG}` }
})
await check('chips: New note and Weekly review in the header, Edit and Summarise in the reader, each with its words', () => {
  const header = chipsIn(wide.header)
  const reader = chipsIn(readerOf(wide.body))
  const words = Object.fromEntries(wide.chips.map((c) => [c.label, c.words]))
  const want = { 'New note': 'Save a note: ', 'Weekly review': 'Write my weekly review.', Edit: `Update ${LONG}: ` }
  const ok = ['Weekly review', 'New note'].every((l) => header.includes(l)) && ['Edit', 'Summarise'].every((l) => reader.includes(l)) && Object.entries(want).every(([l, w]) => words[l] === w) && (words.Summarise ?? '').includes(LONG_TITLE)
  return { ok, measured: `header: ${show(header)}; reader: ${show(reader)}; words: ${show(words)}`, limit: `header New note ${show(want['New note'])} and Weekly review ${show(want['Weekly review'])}; reader Edit ${show(want.Edit)} and Summarise naming the note` }
})

// ---- one note, no note ----------------------------------------------------------------------------------------------
await check('one note: one row, marked, its note in the reader, no table', () => {
  const rows = rowsOf(one.body)
  const title = text(first(readerOf(one.body), (el) => attr(el, 'id') === 'note-title'))
  const parts = tableParts(one.body)
  return { ok: rows.length === 1 && rows[0].marked && rows[0].title === LONG_TITLE && title === LONG_TITLE && parts.length === 0, measured: `${rows.length} row(s), ${rows.filter((r) => r.marked).length} marked; reader title ${show(title)}; ${parts.length ? `table parts: ${parts.join(' ')}` : 'no table parts'}`, limit: '1 row, marked, its note in the reader, 0 table parts' }
})
await check('no notes: the page says so and offers a first note', () => {
  const says = text(listOf(empty.body)).includes('No notes yet.')
  const chips = chipsIn(readerOf(empty.body))
  return { ok: says && rowsOf(empty.body).length === 0 && chips.includes('New note'), measured: `list ${says ? 'says "No notes yet."' : `says ${show(text(listOf(empty.body)).slice(0, 60))}`}; ${rowsOf(empty.body).length} rows; chips in the reader region: ${show(chips)}`, limit: '"No notes yet.", 0 rows, a New note chip' }
})

// ---- a narrow page --------------------------------------------------------------------------------------------------
await check('narrow: the list is the page (no reader region) and a row opens the note’s own page', () => {
  const rows = rowsOf(narrow.body)
  const own = rows.filter((r) => /^\/doc\/note_[a-z0-9-]+$/.test(at(r.href).pathname) && !at(r.href).search)
  return { ok: !readerOf(narrow.body) && !!listOf(narrow.body) && rows.length === 28 && own.length === rows.length && rows.every((r) => !r.marked), measured: `reader region ${readerOf(narrow.body) ? 'in the page' : 'not in the page'}; ${rows.length} rows, ${own.length} lead to /doc/note_<id>, ${rows.filter((r) => r.marked).length} marked; first: ${rows[0]?.href}`, limit: 'no reader region; 28 rows, each to /doc/<kind>_<id>, none marked' }
})

// ---- briefs and reviews, search -------------------------------------------------------------------------------------
await check('reviews: each row says its day and its kind, the open one is marked and in the reader', () => {
  const rows = rowsOf(reviews.body)
  const good = rows.filter((r) => {
    const day = Number((at(r.href).searchParams.get('peek') ?? '').match(/^review_\d{4}-\d{2}-(\d{2})-/)?.[1])
    const [when, kind] = r.sub.split(' · ')
    return day && new RegExp(`\\b${day}\\b`).test(when ?? '') && ['Brief', 'Weekly'].includes(kind)
  })
  const marked = rows.filter((r) => r.marked).map((r) => r.title)
  const about = text(first(readerOf(reviews.body), tagged('span')))
  return { ok: rows.length === 7 && good.length === 7 && marked.length === 1 && marked[0] === A_REVIEW && about.startsWith('Review · '), measured: `${rows.length} rows, ${good.length} with "<day> · <kind>" (first: ${show(rows[0]?.sub)}); marked ${show(marked)}; the reader's bar says ${show(about)}`, limit: `7 rows, each "<day> · Brief|Weekly"; ${show(A_REVIEW)} marked; the reader's bar "Review · <day>"` }
})
await check('search: results across kinds, each with its kind and the match, each to where its kind lives', () => {
  const rows = rowsOf(found.body)
  const kinds = rows.map((r) => r.sub.split(' · ')[0])
  const leads = Object.fromEntries(rows.map((r) => [r.sub.split(' · ')[0], `${at(r.href).pathname}${at(r.href).search}`]))
  const matching = rows.filter((r) => /garden/i.test(`${r.title} ${r.sub}`) && r.sub.includes(' · '))
  const ok = ['Task', 'Person', 'Note', 'Review'].every((k) => kinds.includes(k)) && matching.length >= 4 && leads.Task === '/tasks/buy-seeds-for-the-garden' && leads.Person === '/people/minh' && /^\/notes\?q=garden&peek=note_/.test(leads.Note ?? '') && /^\/notes\?q=garden&peek=review_/.test(leads.Review ?? '')
  const heading = text(first(listOf(found.body), (el) => el.tag === 'p' && text(el).startsWith('Results for')))
  return { ok: ok && heading.includes('garden'), measured: `${rows.length} rows, kinds ${show([...new Set(kinds)])}, ${matching.length} name the match; ${show(heading)}; leads: ${show(leads)}`, limit: 'Task, Person, Note and Review rows naming "garden"; a task to /tasks/…, a person to /people/…, a note or review to ?q=garden&peek=…' }
})

// ---- the note's own page --------------------------------------------------------------------------------------------
await check('own page (/doc/:id): the same Reader column', () => {
  const title = text(first(own.body, tagged('h1')))
  const body = first(own.body, (el) => classes(el).includes('prose-life'))
  const column = body && (classes(body).includes('reader') ? body : null)
  return { ok: title === LONG_TITLE && !!column && text(body).startsWith(LONG_STARTS), measured: `title ${show(title)}; the note's text ${body ? `has classes ${show(classes(body).join(' '))}` : 'missing'}`, limit: 'the title, and its text in a .reader element' }
})

await vite.close().catch(() => {})
const secs = (ms) => Math.round(ms / 100) / 10
const used = process.cpuUsage()
info(`took ${secs(Date.now() - started)}s (rendering seven pages ${secs(rendered - started)}s, then the stylesheet ${secs(read - rendered)}s); processor time ${secs((used.user + used.system) / 1000)}s; no browser, no socket`)
finish()
