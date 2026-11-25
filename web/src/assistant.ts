// The conversations: one with the Assistant and one with each crew member,
// in the panel beside every page. Pages never change data; they put words in
// a conversation's box (`say`) for the user to finish and send, or send them
// right away (`ask`). Nothing is decided here: turns come from the server.
import { nextTick, reactive } from 'vue'
import { ASSISTANT, brief, chat, history, resetChat, stop, type Step, type Turn } from './core/link'

export { ASSISTANT }

export interface Conversation {
  turns: Turn[]
  loaded: boolean
  /** The message being worked on, what the agent did so far, and when it started. */
  pending: null | { ask: string; steps: Step[]; started: number }
  error: string
  draft: string
}

export const panel = reactive({
  open: localStorage.getItem('lifeos.assistant') !== 'closed',
  /** Who the panel talks to: `assistant` or a crew member id. */
  agent: localStorage.getItem('lifeos.agent') ?? ASSISTANT,
  tab: 'chat' as 'chat' | 'memory' | 'about',
  focusTick: 0,
})

const conversations = reactive<Record<string, Conversation>>({})

export function conversation(agent: string): Conversation {
  if (!conversations[agent]) {
    conversations[agent] = { turns: [], loaded: false, pending: null, error: '', draft: '' }
    void load(agent)
  }
  return conversations[agent]
}

/** The conversation the panel shows. */
export function current(): Conversation {
  return conversation(panel.agent)
}

export function setOpen(open: boolean): void {
  panel.open = open
  localStorage.setItem('lifeos.assistant', open ? 'open' : 'closed')
}

export function talkTo(agent: string): void {
  panel.agent = agent
  panel.tab = 'chat'
  localStorage.setItem('lifeos.agent', agent)
  setOpen(true)
  void nextTick(() => panel.focusTick++)
}

/** Puts words in a conversation's box for the user to finish, and opens it. */
export function say(words: string, agent = ASSISTANT): void {
  conversation(agent).draft = words
  talkTo(agent)
}

export async function load(agent: string): Promise<void> {
  try {
    const c = conversations[agent]
    c.turns = await history(agent)
    c.loaded = true
  } catch {
    // The link redials; the next call loads it.
  }
}

function addStep(c: Conversation, step: Step): void {
  const steps = c.pending?.steps
  if (!steps) return
  const last = steps[steps.length - 1]
  if ('Text' in step && last && 'Text' in last) last.Text += step.Text
  else steps.push(step)
}

async function run(agent: string, ask: string, call: (onStep: (s: Step) => void) => Promise<unknown>): Promise<void> {
  const c = conversation(agent)
  if (c.pending) return
  c.error = ''
  c.pending = { ask, steps: [], started: Date.now() }
  try {
    await call((s) => addStep(c, s))
    c.turns = await history(agent)
  } catch (err) {
    c.error = err instanceof Error ? err.message : String(err)
    // Give the words back, so they can be sent again.
    if (!c.draft.trim() && ask !== BRIEF_ASK) c.draft = ask
  } finally {
    c.pending = null
  }
}

/** Sends a message now (to the Assistant unless told otherwise). */
export function ask(message: string, agent = ASSISTANT): Promise<void> {
  const text = message.trim()
  if (!text) return Promise.resolve()
  setOpen(true)
  return run(agent, text, (onStep) => chat(agent, text, onStep))
}

export function sendDraft(agent = panel.agent): void {
  const c = conversation(agent)
  const text = c.draft
  c.draft = ''
  void ask(text, agent)
}

export const BRIEF_ASK = 'Viết bản tin buổi sáng'

export function writeBrief(): Promise<void> {
  setOpen(true)
  return run(ASSISTANT, BRIEF_ASK, (onStep) => brief(onStep))
}

export async function stopAgent(agent = panel.agent): Promise<void> {
  try {
    await stop(agent)
  } catch (err) {
    conversation(agent).error = err instanceof Error ? err.message : String(err)
  }
}

export async function newConversation(agent = panel.agent): Promise<void> {
  await resetChat(agent)
  const c = conversation(agent)
  c.turns = []
  c.error = ''
}

/** Is any agent working (for buttons that would ask the Assistant)? */
export function busy(agent = ASSISTANT): boolean {
  return !!conversations[agent]?.pending
}

/** Tool names as the user reads them. */
const TOOL_WORDS: Record<string, string> = {
  dashboard: 'xem tổng quan',
  tasks_view: 'xem việc',
  list_tasks: 'xem việc',
  add_task: 'thêm việc',
  update_task: 'sửa việc',
  complete_task: 'xong việc',
  reopen_task: 'mở lại việc',
  list_goals: 'xem mục tiêu',
  add_goal: 'thêm mục tiêu',
  update_goal: 'cập nhật mục tiêu',
  habits: 'xem thói quen',
  define_habit: 'đặt thói quen',
  log_habit: 'ghi thói quen',
  archive_habit: 'dừng thói quen',
  list_people: 'xem mọi người',
  person: 'xem một người',
  add_person: 'thêm người',
  update_person: 'cập nhật người',
  log_interaction: 'ghi liên lạc',
  money_month: 'xem tài chính',
  add_transaction: 'ghi giao dịch',
  delete_transaction: 'xoá giao dịch',
  set_budget: 'đặt ngân sách',
  metric_series: 'xem sức khoẻ',
  log_metric: 'ghi chỉ số',
  read_journal: 'đọc nhật ký',
  write_journal: 'viết nhật ký',
  save_note: 'lưu ghi chú',
  save_review: 'lưu bản tin',
  search: 'tìm',
  read_doc: 'đọc',
  capture: 'ghi vào inbox',
  resolve_inbox: 'xử lý inbox',
  recall: 'nhớ lại',
  remember: 'ghi nhớ',
  forget: 'quên',
  list_memories: 'xem memory',
  list_crew: 'xem crew',
  add_member: 'tuyển thành viên',
  update_member: 'sửa thành viên',
  retire_member: 'cho thành viên nghỉ',
  Read: 'đọc file',
  Edit: 'sửa file',
  Write: 'viết file',
  Glob: 'tìm file',
  Grep: 'tìm trong file',
  WebSearch: 'tìm trên web',
  WebFetch: 'đọc trang web',
}

export function toolWords(name: string): string {
  return TOOL_WORDS[name] ?? name.replace(/_/g, ' ')
}
