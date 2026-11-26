// Made-up notes, briefs and reviews for the Notes page (and a task, a person and a journal day, so a search crosses
// kinds). ?scenario= picks how full the vault is: `many` (28 notes, 7 briefs and reviews; the default), `one` (a single
// long note and nothing else), `long` (the long note among a few) or `none` (no notes, no reviews). The long note (longNote.md, about 1,900 words of
// Vietnamese with headings, lists and a quote) is in every scenario, as note_so-tay-vuon-rau-san-thuong-mot-nam-nhin-lai.
import type { DocFull } from '../src/core/generated/DocFull'
import type { Kind } from '../src/core/generated/Kind'
import type { SearchHit } from '../src/core/generated/SearchHit'
import { flag } from './flags'
import longBody from './longNote.md?raw'

const FOLDER: Record<string, string> = { note: 'notes', review: 'reviews', task: 'tasks', person: 'people', journal: 'journal' }
// lifeos-core's slugify: ASCII, Vietnamese kept readable.
const slug = (title: string) => fold(title).replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')
const fold = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/đ/g, 'd').replace(/Đ/g, 'D').toLowerCase()
const doc = (kind: Kind, id: string, title: string, tags: string[], body: string, status: string | null = null): DocFull => ({
  summary: { id, kind, title, status, due: null, scheduled: null, priority: null, goal: null, horizon: null, progress: null, tags, path: `${FOLDER[kind]}/${id}.md` },
  body,
})
const note = (title: string, tags: string[], body: string) => doc('note', slug(title), title, tags, body)

const LONG = note('Sổ tay vườn rau sân thượng: một năm nhìn lại', ['vườn', 'sổ-tay', 'garden'], longBody)

const NOTES: DocFull[] = [
  note('Ý tưởng quà sinh nhật cho mẹ', ['gia-đình', 'quà'], 'Mẹ thích trà hoa và khăn lụa.\n\n- Bộ ấm trà gốm men lam\n- Khăn lụa màu be\n- Một chuyến đi Đà Lạt hai ngày'),
  LONG,
  note('Packing list for the Hà Giang loop', ['travel', 'checklist'], '## Clothes\n\n- Rain jacket\n- Two warm layers\n\n## Papers\n\n- Driving licence\n- Cash for homestays'),
  note('Công thức phở gà của bà', ['nấu-ăn', 'gia-đình'], 'Nước dùng ninh từ xương gà ta trong ba tiếng, thêm gừng nướng và hành khô.\n\n1. Chần bánh phở\n2. Xé thịt gà\n3. Chan nước dùng thật nóng'),
  note('Books to read in 2027', ['reading'], '- A history of small gardens\n- Something on sleep\n- One novel a month, any language'),
  note('Meeting notes: garden club, November', ['garden', 'meeting'], 'Minh proposed a seed swap in December. I offered to bring morning glory and basil seeds.\n\nNext meeting: first Saturday of the month.'),
  note('Ghi chú học tiếng Nhật: trợ từ', ['học', 'tiếng-nhật'], 'は đánh dấu chủ đề, が đánh dấu chủ ngữ.\n\nVí dụ cần thuộc: 私は学生です。'),
  note('Kế hoạch sửa bếp', ['nhà', 'dự-án'], 'Đổi mặt đá, thêm một ổ điện cạnh bồn rửa, sơn lại tủ trên.\n\nNgân sách dự kiến: 35 triệu.'),
  note('Running form cues', ['health', 'running'], 'Short steps, land under the hips, relax the shoulders. Check cadence once per run, not all the time.'),
  note('Danh sách phim muốn xem', ['giải-trí'], '- Một phim tài liệu về biển\n- Phim hoạt hình mới của hãng quen\n- Xem lại bộ ba phim cũ với bố'),
  note('Wifi và mật khẩu nhà bà ngoại', ['gia-đình'], 'Hỏi cậu Tư, tờ giấy dán sau modem. Không ghi mật khẩu ở đây.'),
  note('Ideas for the balcony lights', ['home'], 'Warm white string along the rail. A timer plug so they turn off at 23:00.'),
  note('Câu hỏi cho buổi khám răng', ['sức-khỏe'], '1. Có nên bọc lại răng số sáu không?\n2. Bao lâu thì lấy cao răng một lần?'),
  note('Thoughts on the weekly review habit', ['reflection', 'habits'], 'Twenty minutes on Sunday evening works. Friday afternoon never did: too tired to be honest.'),
  note('Cách ủ phân từ rác bếp', ['vườn', 'garden'], 'Thùng có nắp, một lớp rác xanh, một lớp lá khô. Đảo mỗi tuần. Sau hai tháng thì dùng được.'),
  note('Gift ideas for the team', ['work'], 'Small notebooks, good pens, a card written by hand.'),
  note('Lịch tiêm phòng của Bông', ['thú-cưng'], 'Mũi nhắc lại vào tháng ba. Sổ tiêm để trong ngăn kéo bàn làm việc.'),
  note('Quotes worth keeping', ['reading', 'reflection'], '> Attention is the beginning of care.\n\nWritten down after a walk, source unknown.'),
  note('Những quán cà phê yên tĩnh để làm việc', ['công-việc', 'địa-điểm'], '- Quán trong hẻm gần hồ, tầng hai\n- Quán sách ở phố cũ, mở từ bảy giờ'),
  note('Bike maintenance log', ['bike', 'log'], 'Chain cleaned and oiled. Rear brake pads at half. Tyres at 5 bar.'),
  note('Ý tưởng bài viết về thói quen đọc sách', ['viết', 'reading'], 'Mở bài bằng chuyện cái kệ sách trống. Ba ý chính: đọc ít nhưng đều, ghi lại một câu, kể cho một người.'),
  note('Tax paperwork checklist', ['money', 'checklist'], '- Income statements\n- Rent contract\n- Receipts for the home office'),
  note('Sổ tay chăm cây lan', ['vườn'], 'Tưới khi rễ chuyển màu bạc. Không để nước đọng ở nách lá.'),
  note('Tên cho dự án phụ', ['ý-tưởng'], 'Sân Thượng, Mầm, Luống. Chưa cái nào thật ưng.'),
  note('Stretching routine after long sitting', ['health'], 'Hip flexors, hamstrings, upper back. Five minutes every two hours.'),
  note('Bài học từ chuyến đi Huế', ['du-lịch', 'reflection'], 'Đi chậm hơn. Mỗi ngày một nơi là đủ. Ăn ở chỗ người địa phương xếp hàng.'),
  note('Questions for the landlord', ['home'], 'Can we repaint the hallway? Who fixes the water heater?'),
  note('Danh sách việc trước Tết', ['nhà', 'checklist'], '- Dọn kho\n- Mua hoa\n- Gói bánh với cả nhà vào ngày hai mươi tám'),
]

const review = (day: string, kind: 'brief' | 'weekly', title: string, body: string) => doc('review', `${day}-${kind}`, title, [], body, kind)
const REVIEWS: DocFull[] = [
  review('2026-11-25', 'brief', 'Morning brief, Wednesday', '## Due today\n\n- Renew passport\n- Send invoice to Lan\n\n## Look forward to\n\nGarden club seed swap on Saturday.'),
  review('2026-11-24', 'brief', 'Morning brief, Tuesday', 'Two tasks due, one habit open. The balcony lights arrive today.'),
  review('2026-11-23', 'weekly', 'Weekly review, week 47', '## What went well\n\nThree runs, the garden notes are finally written up.\n\n## What to change\n\nSleep before midnight on work nights.'),
  review('2026-11-23', 'brief', 'Morning brief, Monday', 'A quiet day. One call with Minh about the garden club.'),
  review('2026-11-20', 'brief', 'Morning brief, Friday', 'Invoice reminders went out. Nothing overdue.'),
  review('2026-11-16', 'weekly', 'Weekly review, week 46', '## What went well\n\nKitchen plan agreed.\n\n## What to change\n\nFewer evening meetings.'),
  review('2026-11-09', 'weekly', 'Weekly review, week 45', 'Mostly travel. The Hà Giang packing list worked.'),
]

const OTHERS: DocFull[] = [
  doc('task', 'buy-seeds-for-the-garden', 'Buy seeds for the garden', [], 'Morning glory, basil, lettuce. Ask the garden club first.', 'open'),
  doc('person', 'minh', 'Minh', ['garden'], 'Runs the garden club. Knows where to find coconut fibre.'),
  doc('journal', '2026-11-20', '2026-11-20', [], '## 07:10\n\nWatered the garden before work. The lettuce is ready.'),
]

let notes: DocFull[] = []
let reviews: DocFull[] = []
let all: DocFull[] = []
/** Fills the vault as `scenario` says. A tab does it once, from its address; contract.mjs before each page it renders. */
export function useScenario(scenario: string): void {
  notes = scenario === 'none' ? [] : scenario === 'one' ? [LONG] : scenario === 'long' ? [LONG, ...NOTES.filter((n) => n !== LONG).slice(0, 7)] : [...NOTES]
  reviews = scenario === 'none' || scenario === 'one' ? [] : scenario === 'long' ? REVIEWS.slice(0, 2) : REVIEWS
  all = [...notes, ...reviews, ...OTHERS]
}
useScenario(flag('scenario') ?? 'many')

function find(id: string): DocFull {
  const bare = id.replace(/\.md$/, '')
  const hit = all.find((d) => d.summary.id === bare || `${d.summary.kind}_${d.summary.id}` === bare || d.summary.path === id)
  if (!hit) throw new Error(`no document ${id}`)
  return hit
}

// Words in any order, accents optional, across titles, tags and bodies; the snippet marks the match as the server does.
function search(query: string, limit: number): SearchHit[] {
  const words = fold(query).split(/\s+/).filter(Boolean)
  if (!words.length) return []
  const hits: SearchHit[] = []
  for (const d of all) {
    const text = `${d.summary.title}\n${d.summary.tags.join(' ')}\n${d.body}`
    const folded = fold(text)
    if (!words.every((w) => folded.includes(w))) continue
    const flat = d.body.replace(/\s+/g, ' ')
    const at = fold(flat).indexOf(words[0])
    const from = Math.max(0, at - 30)
    const snippet = at < 0 ? flat.slice(0, 90) : `${from ? '…' : ''}${flat.slice(from, at)}«${flat.slice(at, at + words[0].length)}»${flat.slice(at + words[0].length, at + 70)}…`
    hits.push({ id: d.summary.id, kind: d.summary.kind, title: d.summary.title, path: d.summary.path, snippet })
  }
  return hits.slice(0, limit)
}

/** A note saved while the page is open: it comes first, as the most recently changed does. */
export function addNote(title: string): void {
  const fresh = note(title, ['new'], 'Saved a moment ago.')
  notes.unshift(fresh)
  all.unshift(fresh)
}

/** What the fake link answers for the document tools; undefined for a tool that is not one of them. */
export function notesRead(tool: string, input: unknown): unknown {
  const i = (input ?? {}) as { limit?: number; query?: string; id?: string }
  if (tool === 'recent_notes') return notes.slice(0, i.limit ?? 20).map((d) => d.summary)
  if (tool === 'reviews') return reviews.slice(0, i.limit ?? 20).map((d) => d.summary)
  if (tool === 'search') return search(i.query ?? '', i.limit ?? 20)
  if (tool === 'read_doc') return find(i.id ?? '')
  return undefined
}
