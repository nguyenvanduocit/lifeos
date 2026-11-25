# LifeOS design: "Ink + signal"

The owner chose this direction on 2026-11-25, after a study of Dribbble references and of the information architecture
(site map, list views, detail views), and approved the prototype (shell, ⌘K, Today, Tasks, the task page) the same
day. The north star is Relay (Usman Qureshi, dribbble.com/shots/27845962): a warm stone frame, a white or near-black
canvas, ink and grey, and one lime signal. Light and dark follow the system (`prefers-color-scheme`); both are
designed, neither is an afterthought.

## Principles

1. **Lists are rows, not cards.** One container per list, a light head row with an icon and a word per column, rows
   divided by hairlines. Panels hold summaries of different kinds, or a detail.
2. **Every number carries its frame.** A value comes with its target, its period or a comparison:
   "0/5 tasks done today · 2 late", "₫14.5M spent this month · 55% of budgets", "+2 vs last week". A bare number is a
   bug.
3. **One signal.** Lime marks only what needs the owner now: the overdue count in the navigation, today in a week
   strip, "today" chips. Everything else is ink and grey. Status is a dot and a word on a soft tint (`StatusTag`),
   never a solid saturated pill.
4. **Sans for words and figures.** Geist everywhere, figures tabular. Geist Mono only for file paths and ids.
5. **Icons say what a thing is or does.** Every navigation item, column head, panel title and button carries a
   Lucide icon. An area's hue appears only on small marks.
6. **Nothing decorative.** No gradients, glass, glows, mascots, rings or watermarks. Charts are bars, lines and
   heatmaps in one hue, with grey for comparison.
7. **Pages read, the Assistant writes.** Page actions draft a prompt into the Assistant (`AskChip`, `say`): the icon
   says what it does, the small ✦ at the end says the Assistant does it. One-outcome acts (complete a task, log a
   habit today) ask directly (`ask`).

## Tokens (style.css)

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#f1efea` | `#0d0d0c` | the frame behind the canvas, the sidebar |
| `--panel` | `#ffffff` | `#161615` | the canvas, containers |
| `--panel2` | `#f7f6f3` | `#1c1c1a` | head rows, wells, the details card, hover |
| `--panel3` | `#efede8` | `#232321` | pressed, chips |
| `--line` / `--line2` | `#e9e6e0` / `#dcd8d0` | `#262624` / `#33322f` | hairlines / control borders |
| `--ink` / `--ink2` / `--ink3` | `#141413` / `#66635d` / `#9a968f` | `#f2f1ee` / `#a8a59f` / `#75726c` | text; the solid of primary buttons and the active nav pill |
| `--signal` | `#dcf37a` | `#dcf37a` | what needs the owner |
| `--good` / `--warning` / `--critical` / `--info` | `#2e8a4e` `#b86e00` `#d0532a` `#3a63d8` | `#6fd394` `#f2b84b` `#f08a66` `#8fb0ff` | status dots, words, tints |

Tones: `data-tone="<area|good|warning|critical|info|signal|neutral>"` sets `--tone`, and `--tone-wash`, `--tone-soft`,
`--tone-line`, `--tone-ink` derive from it.

Radius: 16px containers, 10–12px controls, 6px chips. Shadow only on what floats (popovers, the command menu, the
peek).

## Type

- Geist Variable, 14px base. Wordmark in Instrument Serif.
- Page title 28–42px/600, tracking −0.03em. Panel title 17px/600. Row title 14–14.5px/500.
- Eyebrow and metadata: 13px `--ink2`/`--ink3`. Column heads 13px `--ink3` with a 14px icon.
- Stat value 19px/600 tabular, followed by what it counts.

## Shell (App.vue)

- Sidebar 248px on `--bg`: the serif wordmark with a lime dot; **Today** (Today, Inbox), **Life** (Tasks, Goals,
  Habits, People, Money, Health, Journal, Notes), **Agents** (Crew, Team talk, Automation, Memory); "Your day" and
  Integrations at the foot. The active item is a solid ink pill; counts are grey chips, lime when overdue.
- Header 64px: the command field "✦ Ask LifeOS or jump to… ⌘K", the link's freshness ("● Live · updated 2m ago"),
  the Assistant button.
- Canvas: `--panel`, rounded 16px inside the `--bg` frame on wide screens. Mobile: a bottom bar (Today, Inbox, Tasks,
  More).
- Command menu (⌘K, `/`): ask the Assistant (drafts), results from `search`, go to a page.

## Site map

Each kind of thing has its own page; `links.ts` (`docLink`) knows where each lives, and `/doc/:id` is the reader for
the rest: `/tasks/:id`, `/goals/:id`, `/habits/:name`, `/people/:id`, `/health/:metric`, `/journal/:day`, `/crew/:id`.
Views and filters live in the address (`?view=`, `?goal=`, `?peek=`).

## Templates

- **Dashboard** (Today): eyebrow with date and week, a greeting, the stat strip, then panels of different kinds.
- **List + peek**: page header with the stat strip; one list container with a toolbar (`Segmented`, filters), a head
  row, groups with counts, hairline rows; on wide screens a row opens beside the list (`?peek=id`, 420px, "Open
  page"), below 64rem it goes to the page.
- **Detail**: breadcrumb (Area › item), the title, then two columns: the body (notes, related lists, Ask chips) and a
  sticky "Details" card (`PropertyList`, rows shown only when they have a value).
- **Reader** (journal, notes, reviews): a 68ch column, 16px/1.7.

## Components (web/src/components/ds)

`PageHeader`, `StatStrip`, `Delta`, `Panel`, `TableHead` (with `data-cols` grids that collapse in narrow
containers), `Segmented`, `StatusTag`, `PropertyList`, `IconTile`, `Kbd`, `AreaMark`; with the shared `AskChip`,
`CommandMenu`, `DayMeter`, and `tasks/` (`TaskLine`, `TaskCheck`, `TaskDetail`, `Priority`).

## Anti-patterns

Grids of identical cards; numbers without a frame; solid saturated pills; gradient text or AI mascots; neon on navy;
mono for everything; floating chat over content; decorative charts (gauges, donuts, blobs); upsell slots.
