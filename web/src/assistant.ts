// The Assistant: the one conversation with the agent, beside every page.
// Pages never change data; they put words in the Assistant's box (`say`) for
// the user to finish and send, or send them right away (`ask`).
import { nextTick, reactive } from 'vue'
import { brief, chat, history, resetChat, type Progress, type Turn } from './core/link'

export interface Step {
  kind: 'text' | 'tool'
  text: string
}

export const assistant = reactive({
  open: localStorage.getItem('lifeos.assistant') !== 'closed',
  draft: '',
  turns: [] as Turn[],
  /** The message being worked on, and what the agent did so far. */
  pending: null as null | { ask: string; steps: Step[] },
  error: '',
  focusTick: 0,
})

export function setOpen(open: boolean): void {
  assistant.open = open
  localStorage.setItem('lifeos.assistant', open ? 'open' : 'closed')
}

/** Puts words in the box for the user to finish, and opens the Assistant. */
export function say(words: string): void {
  assistant.draft = words
  setOpen(true)
  void nextTick(() => assistant.focusTick++)
}

function onProgress(p: Progress): void {
  const steps = assistant.pending?.steps
  if (!steps) return
  if ('Text' in p) {
    const last = steps[steps.length - 1]
    if (last?.kind === 'text') last.text += p.Text
    else steps.push({ kind: 'text', text: p.Text })
  } else {
    steps.push({ kind: 'tool', text: p.Tool })
  }
}

async function run(ask: string, call: () => Promise<unknown>): Promise<void> {
  if (assistant.pending) return
  assistant.error = ''
  assistant.pending = { ask, steps: [] }
  setOpen(true)
  try {
    await call()
    assistant.turns = await history()
  } catch (err) {
    assistant.error = err instanceof Error ? err.message : String(err)
    // Give the words back, so they can be sent again.
    if (!assistant.draft.trim() && ask !== BRIEF_ASK) assistant.draft = ask
  } finally {
    assistant.pending = null
  }
}

/** Sends a message to the agent now. */
export function ask(message: string): Promise<void> {
  const text = message.trim()
  if (!text) return Promise.resolve()
  return run(text, () => chat(text, onProgress))
}

export function sendDraft(): void {
  const text = assistant.draft
  assistant.draft = ''
  void ask(text)
}

export const BRIEF_ASK = 'Viết bản tin buổi sáng'

export function writeBrief(): Promise<void> {
  return run(BRIEF_ASK, () => brief(onProgress))
}

export async function loadHistory(): Promise<void> {
  try {
    assistant.turns = await history()
  } catch {
    // The link redials; the history comes with the next call.
  }
}

export async function newConversation(): Promise<void> {
  await resetChat()
  assistant.turns = []
  assistant.error = ''
}

/** Tool names as the user reads them. */
const TOOL_WORDS: Record<string, string> = {
  dashboard: 'xem tổng quan',
  add_task: 'thêm việc',
  update_task: 'sửa việc',
  complete_task: 'xong việc',
  reopen_task: 'mở lại việc',
  add_goal: 'thêm mục tiêu',
  update_goal: 'cập nhật mục tiêu',
  define_habit: 'đặt thói quen',
  log_habit: 'ghi thói quen',
  archive_habit: 'dừng thói quen',
  add_person: 'thêm người',
  update_person: 'cập nhật người',
  log_interaction: 'ghi liên lạc',
  add_transaction: 'ghi giao dịch',
  delete_transaction: 'xoá giao dịch',
  set_budget: 'đặt ngân sách',
  log_metric: 'ghi chỉ số',
  write_journal: 'viết nhật ký',
  save_note: 'lưu ghi chú',
  save_review: 'lưu bản tin',
  capture: 'ghi vào inbox',
  resolve_inbox: 'xử lý inbox',
}

export function toolWords(name: string): string {
  return TOOL_WORDS[name] ?? name.replace(/_/g, ' ')
}
