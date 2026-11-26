// What the headless checks (notes.mjs, contract.mjs) share: how a line of output reads, the exit codes, what happens
// when a check cannot run or is ended from outside, the run's time limit, and the names of the made-up notes.
//
// Output: one line per assertion, `✓|✗ name — what was found (limit: …)`, and `· …` for a note. Exit 0 when every
// assertion passed, 1 when any failed, 2 when the check itself could not run (never a pass, never a stack).
//
// The end of the output is written for Remit, which keeps of a check only the last 1500 characters of its stderr, a
// newline and its stdout (remit crates/shipping/src/repochecks.rs:250, `tail(…, 1500)`): a run that did not pass ends
// its stdout with the count, the failed assertions again (shortened to fit) and, if it could not run, the line saying
// why. A run that passed ends with `N passed, 0 failed`.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

export const started = Date.now()

// The made-up notes the checks name (notesData.ts, where they are written).
export const LONG_TITLE = 'Sổ tay vườn rau sân thượng: một năm nhìn lại'
export const LONG_ID = 'so-tay-vuon-rau-san-thuong-mot-nam-nhin-lai'
export const LONG_STARTS = 'Ghi chú này gom lại'
export const OTHER_NOTE = 'Ý tưởng quà sinh nhật cho mẹ'
export const A_REVIEW = 'Weekly review, week 47'
export const A_REVIEW_ID = '2026-11-23-weekly'
export const NO_GARDEN = 'Running form cues'

// Remit's record of a check, in bytes: its `tail` keeps the last n bytes (remit crates/chains/src/git.rs:174).
const RECORD = 1500
// Of that record: the line saying why a check could not run gets at most two thirds, so some failed assertions still
// fit beside it; and it is made of at most three parts (the error's first line and two lines naming what was refused),
// each short enough for the three to fit in that line.
const WHY_LINE = Math.floor((RECORD * 2) / 3)
const WHY_PART = 300
const WHY_NAMED = 2
// A failed assertion repeated at the end keeps at least this much, so that ten of them fit in the record beside the
// count (10 × 141 < 1500) and each still has its name (the longest is 131 characters) and the start of what was found.
// When more failed than fit, the rest are counted.
const FAILED_LINE = 140

let check_ = 'verify'
let doing = 'starting'
let passed = 0
const failures = []
const tidies = []
const bytes = (s) => Buffer.byteLength(s)
/** `line` in at most `max` bytes, with … where it was cut. */
function cut(line, max) {
  if (bytes(line) <= max) return line
  let kept = ''
  for (const ch of line) {
    if (bytes(kept) + bytes(ch) > max - 3) break
    kept += ch
  }
  return `${kept}…`
}
const oneLine = (text) => String(text).split('\n').map((l) => l.trim()).filter(Boolean).join(' | ')

/** Names the check, sets its time limit and what happens when it is ended from outside or throws where nothing
 * catches. */
export function begin(name, runMs) {
  check_ = name
  process.on('uncaughtException', (err) => fatal(`an error nothing caught, while at: ${doing}: ${why(err)}`))
  process.on('unhandledRejection', (err) => fatal(`an error nothing caught, while at: ${doing}: ${why(err)}`))
  // Ended from outside (Remit's jail ends a check at its time limit): say where, leave nothing in the temp directory.
  for (const signal of ['SIGTERM', 'SIGINT', 'SIGHUP']) process.on(signal, () => fatal(`ended by ${signal} after ${Math.round((Date.now() - started) / 1000)}s, while at: ${doing}`))
  setTimeout(() => fatal(`no end after ${runMs / 60_000} minutes, while at: ${doing}`), runMs).unref()
}
/** Says what the check is doing now, for the line that tells where it stopped. */
export function step(what) {
  doing = what
}
export const where = () => doing
/** Something to undo before the check ends, however it ends. */
export function onTidy(fn) {
  tidies.push(fn)
}
export function tidy() {
  for (const fn of tidies.splice(0)) {
    try {
      fn()
    } catch {
      // Nothing to do about a tidy-up that fails: the check's result stands.
    }
  }
}
/** A directory of the check's own under os.tmpdir(), removed when the check ends. A place that refuses the write
 * ends the check (exit 2). */
export function scratch(prefix) {
  try {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), prefix))
    onTidy(() => fs.rmSync(dir, { recursive: true, force: true }))
    return dir
  } catch (err) {
    return fatal(`could not make a directory in ${os.tmpdir()} (file write refused: ${why(err)})`)
  }
}

/** What an error says, in one line: its first line, its code, and the lines of it that name what was refused (the
 * place a check runs in may deny a write, a program, a socket or a Mach service). Playwright puts what a browser that
 * did not start wrote about itself in the error's message, after its first line (tried: 18 lines for a program that
 * exits at once); a browser that ends later says nothing through it. */
const REFUSED = /denied|not permitted|EPERM|EACCES|bootstrap_check_in|FATAL|Check failed/i
export function why(err) {
  const lines = String(err?.message ?? err).split('\n').map((l) => l.trim()).filter(Boolean)
  const named = lines.slice(1).filter((l) => REFUSED.test(l) && !l.includes('<launching>'))
  const told = [...new Set([lines[0] ?? 'no message', ...named.slice(0, WHY_NAMED)])].map((l) => cut(l, WHY_PART))
  return `${err?.code && !told[0].includes(err.code) ? `${err.code}: ` : ''}${told.join(' | ')}`
}

/** Runs one assertion: `fn` returns { ok, measured, limit }; a throw (something that is not there) fails it. */
export async function check(name, fn) {
  doing = name
  let r
  try {
    r = await fn()
  } catch (err) {
    r = { ok: false, measured: String(err?.message ?? err).split('\n')[0], limit: 'runs without error' }
  }
  const line = `${r.ok ? '✓' : '✗'} ${name} — ${r.measured} (limit: ${r.limit})`
  if (r.ok) passed++
  else failures.push(line)
  console.log(line)
}
export const info = (text) => console.log(`· ${text}`)

/** The end of stdout for a run that did not pass: the count, the failed assertions again and `last` (why it could not
 * run), within Remit's record. */
function closing(last) {
  const head = `${passed} passed, ${failures.length} failed${last ? ', then the check could not run' : ''}`
  let room = RECORD - bytes(head) - 1 - (last ? bytes(last) + 1 : 0)
  const more = (n) => `… and ${n} more ✗ above`
  const each = Math.max(FAILED_LINE, Math.floor(room / Math.max(1, failures.length)) - 1)
  const again = []
  for (const [i, line] of failures.entries()) {
    const short = cut(line, each)
    const left = failures.length - i - 1
    if (bytes(short) + 1 + (left ? bytes(more(left)) + 1 : 0) > room) {
      again.push(more(left + 1))
      break
    }
    again.push(short)
    room -= bytes(short) + 1
  }
  return [head, ...again, ...(last ? [last] : [])].join('\n')
}

/** The check could not run: one line saying why, on stderr and as the last line of stdout, and exit 2. */
export function fatal(message) {
  tidy()
  const line = cut(`${check_} could not run: ${oneLine(message)}`, WHY_LINE)
  console.error(line)
  console.log(closing(line))
  process.exit(2)
}

/** The count, and the exit: 0 when every assertion passed, 1 when not. */
export function finish() {
  tidy()
  console.log(failures.length ? closing('') : `${passed} passed, 0 failed`)
  process.exit(failures.length ? 1 : 0)
}
