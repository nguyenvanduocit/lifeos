# 0001. The agent backbone is remit's

Status: accepted, 2026-11-25. Being built in steps; each step below says where it stands.

## Context

lifeos ran its agents on `claude-code-agent` with one in-process SDK MCP server, kept a chat as whole turns in `turns`,
and had a simple memory (`memories` with FTS, written by the agent itself). remit (`~/projects/remit`) has spent far
longer on agents: how a session runs and recovers, how messages queue and steer, how agents talk to each other and to
the owner, what they remember and how it is recalled, and how the owner watches all of it. The owner decided that lifeos
takes remit's backbone, and that the Assistant panel looks and behaves as remit's does.

Decisions the owner made (2026-11-25):

- remit's crates are **copied** into lifeos, not shared: `claudecode`, `claudehooks`, `agent`, `workflow`, with what they
  stand on (`jail`, `procgroup`, `childenv`, `urlescape`) and the part of remit's `model` they speak (`crates/model`).
  They keep remit's names and lints so a later change in remit can be carried over by diff.
- Memory is remit's ADR-0117 as it is, embeddings and reranking through OpenRouter included: memory text leaves the
  machine for those two calls.
- remit's look (dark tokens, Inter, JetBrains Mono) applies to the Assistant panel only; lifeos's pages keep theirs.

A later decision of the owner (2026-11-25), which overrides "pages never write" for these acts alone:

- **Pages may write the owner's own acts on the agents, as remit's do**: his message, a pick in a block, an overturn or
  answer of a question, a mark on a reply, a stop. A life's data still changes only through an agent: `Call::Read` stays
  read-only for tools, and none of these calls runs a tool.

## Decision

lifeos keeps remit's concepts and names wherever lifeos has the same thing, and drops what belongs to remit's software
chains (chains, Chairs, judges, landings, forges, gates, signals).

### Sessions (step 1)

- `crates/agent` (`ClaudeLauncher`) starts every session. `lifeos-agent` implements `agent::Handler` and is the only
  caller of the launcher.
- **Card** = one agent's working record: the Assistant has one, each crew member has one. A card holds its session id
  (resumed with `--resume`), model, effort, account, status, what it is doing, plan, context use, turn count, last output
  and error (remit `model::Card`, without chain fields). Table `cards`.
- **Event** = one entry of a card's stream, remit's shape: kind + status (`model::EventStatus`), tool call id/kind/name,
  title, text, `by` (who sent a prompt), priority, `data` (JSON), times, `rev`. Table `events`. The stream is the
  conversation; `turns` goes.
- **Outbox**: a message to a card is a queued `prompt` event, stored in the transaction that sends it. A card's loop is
  woken by its **mailbox** (wake only) and takes the highest priority, then oldest, queued prompt. The owner's message is
  `PRIORITY_OWNER`; a new session's opening brief `PRIORITY_OPENING`.
- **Delivery** of the owner's message: `queue` (next turn), `steer` (into the running turn, `Session::steer`),
  `interrupt` (cancel the turn, then send).
- **Stream**: `Update`s become events; message and thought chunks join into one event per run, saved every 500 ms and
  pushed to pages as deltas (`EventDelta {id, offset, text}`). A turn ends with a `stop` event whose data is the turn's
  summary (duration, tools by group, failures).
- **Tools** reach a session over MCP on loopback HTTP (`/mcp`, rmcp streamable HTTP), the bearer token minted per run
  naming the card: each card is served exactly the tools its agent holds. The door lives in `lifeos-agent` so every host
  (web, Telegram, CLI) has it.
- Claude Code's own auto-memory is off; only project settings are read; MCP is strict; Bash is not given.

### Assistant and crew (step 3)

- The Assistant is a card with no employee. A new session of it opens with its brief and a recap of the conversation so
  far. "Start over" opens a new session.
- Crew follows remit's templates and employees: a **template** (built-in or added) is a loadout (instructions, tools,
  skills), model and effort; an **employee** is hired from one, keeps a copy of the loadout, and is active or retired.
  The Assistant hires, changes and retires on the owner's words.
- (Superseded by 0002: any agent asks or hands work to any other, and each hand-off is tracked.) Agents talk through messages, never directly: the Assistant `message_agent`s a member (the member's final message of
  that turn comes back to the Assistant as `[reply from …]`); a member `ask_assistant`s; anyone may `ask_owner`.
- **No agent waits on the owner** (remit ADR-0087): an `ask_owner` question goes to the Assistant, which answers it
  (`answer_question`) or leaves it to the owner (`leave_to_owner`) and records why. The owner reads the Assistant's
  decisions in its Decisions tab and may overturn one; an overturn becomes a teaching.

### The owner's own acts from the panel

- **Blocks** (remit's ADR-0091): every agent but the librarian has `show_block`; a block is a `System` event of status
  `block`, data `{block}`, typed `Activity::Block`. The owner's pick (`Call::BlockPick`) rewrites the block and queues
  one message from him; a later pick rewrites that message while it is unread.
- **Overturn** (remit's `FeedOverturn`): `Call::Overturn` gives the owner's word as `answer_question` does in a turn on
  his words: the asker gets it, and an overturn becomes a teaching of the Assistant. remit's dissent and overturn windows
  are not ported.
- **Reply teach** (remit's ADR-0093): `Call::Teach` marks an agent's reply helpful, not helpful or correct; the mark stays
  on the reply (`Activity::Marked`) and each change is a teaching of that agent.
- **Stop with a reason** (remit's Stop agent…): `Call::Stop` drops what waits for the agent, ends its turn, and keeps his
  why in its stream.

### Memory (step 4)

remit ADR-0117: typed memories (lesson, pitfall, preference, principle, decision, intent, fact, procedure) with scope,
trust, links and use counts, learned from **teachings** (the owner's corrections, overturns, replies rated) by a
**librarian** agent, the only writer. Recall each turn fuses FTS, embeddings and memory links, reranks, and puts at most
five in front of the message; more arrive after tool calls (`Handler::on_moment`).

### Workflows (step 5)

`crates/workflow` runs a script (Claude Code's Workflow shape) by replaying its journal; the Assistant starts one with
`run_workflow`, and routines fire prompts or workflows.

### The panel (step 2)

remit's Assistant drawer: header with the agent and its standing, tabs Activity / Decisions / Plan / Session / Memory,
`StreamBlocks` (tool calls grouped, turns folded, long runs folded), pinned questions, the composer (drafts kept on the
server, images, Send / Steer / Interrupt / Stop) and the context meter. Its components carry remit's tokens, scoped to
the panel.

## As built (2026-11-25)

Where the build differs from the decision above, the build says what holds:

- **Copied crates and their changes.** `agent`, `claudecode`, `claudehooks`, `workflow`, `jail`, `procgroup`, `childenv`, `urlescape`, `openrouter` are remit's; `model` is the subset of remit's `model` and `wire::runtime` they need; `telemetry` is remit's `otlp`, `summary` and `attr` (not `child` and `correlate`, which are Datadog's), its summary speaking `lifeos-wire`'s `CardTelemetry`. In `agent`, `Spec.outside` carries the owner's own MCP connections (`options::outside_by_reference` puts their secrets in env) and an empty account home means the machine's own Claude login (no `CLAUDE_CONFIG_DIR`, so its Keychain item is found). `jail::init()` runs first in `main`.
- **Crew stays lifeos's** (`crew/*.md` members with role, instructions, tools, connections). remit's templates and employees are not ported: a lifeos crew is a handful of members the owner names, not staff hired per chain.
- **Messages** (`lifeos-agent/src/messages.rs`; since 0002 `ask_agent`, `hand_off`, `cancel_handoff`, `team_status` for every agent, in `handoffs.rs`): the Assistant has `message_agent`; a member has `ask_assistant`, `ask_owner`, `withdraw_question`. A reply comes back to the sender only for a message that asked for one (`PromptData.reply`), so a reply is never answered back. remit's no-progress rail is kept: after 6 turns in a row on agents' replies the sender is reminded, twice, then refused.
- **Questions** (`questions` table, `list_questions`): the Assistant decides with `answer_question` (a reason required) or `leave_to_owner`. The owner answers or overturns from the panel's Decisions tab (`Call::Overturn`, remit's `FeedOverturn` box: "What do you decide instead?", or "Your answer" for one left to him), or through the Assistant: an `answer_question` in a turn on the owner's own words is recorded as theirs. Either way an overturn becomes a teaching of the Assistant and the asker gets his word. `AgentStanding.waiting` holds the asker's open questions.
- **Blocks** (`lifeos-agent/src/blocks.rs`, remit's ADR-0091): `show_block` takes `{spec: BlockSpec}`, whose parts are **externally tagged** (`{"choice": {…}}`, `{"chart": {…}}`) where remit's are tagged by `type`, since postcard takes no internally tagged enum; the tool's JSON schema is written by hand (lifeos-wire has no schemars). `check_block` is remit's (16 KB as JSON, ids unique, a choice of at least two with its picks among its options, one unless `many`, every series as long as `x` and finite; a refusal names the place). The pick's message is the owner's (`PromptData.block` names the block; the turn on it teaches like any turn on his words); a pick a retired member cannot take is refused before the block changes. A `Mutex` holds a pick from its read to its store. The panel draws `BlockView`, `BlockChoice`, `BlockChart` as remit's, errors said under the block (the panel has no toasts).
- **Reply teach** (`lifeos-agent/src/teach.rs`): the owner alone marks (remit's mentors do not exist here). An agent's reply is a `Message` event with no writer; one being written, and the librarian's, are refused; `correct` needs words. The marks live in the event's `data` (`{marks}`); the same mark and words again change nothing; each change is a `teach_text` teaching of that agent (role `teaching`, the reply quoted to 8000 characters), and the librarian is woken as for any teaching. remit's coach (`repeat_corrections`, `overturn_rates`) and faces are not ported.
- **Stop** (`Agents::stop`): lifeos has no stopped card, so a stop is remit's stop without "for good": the queued prompts are marked `dropped`, the turn is interrupted, and a `System` event of status `stopped` (data `{reason, dropped}`, `Activity::Stopped`) keeps the why. The session is kept. The agent reads it with its next message: `compose` puts `[the owner stopped you…] Why: …` before the message for every stop since the last prompt it took. The standing offers `Ask::Stop` with `stop_says` (what a stop does, in the server's words) while a turn runs or messages wait; the composer's "Stop agent…" asks for the why in remit's `ReasonDialog`.
- **Memory.** Retrieval is remit's (`recall.rs`: words, meaning under `qwen/qwen3-embedding-8b`, links one step, RRF, `qwen/qwen3-reranker-8b` with remit's floors, trust and strength), without remit's subjects (a life's memories are about people and things, not files and crates) and without scopes (a member recalls its own and the Assistant's). The key is `<vault>/.lifeos/openrouter.key` (or `LIFEOS_OPENROUTER_KEY_FILE`), hidden from sessions. **With no key written at all, a turn is given what words find**, as lifeos did before: remit gives nothing then. What a turn was given is kept on its prompt (`PromptData.recalled`). A lifeos tool call is a moment: what bears on its input is given beside it (`on_moment`, at most 3). The **librarian** is a hidden card (`librarian`) with only its tools and no built-in ones; teachings are every turn on the owner's words, an overturn, and a memory that misled more than it helped (a review). It is woken after 10 quiet minutes, or once 12 wait. Other agents have `recall`, `rate_memory`, `list_memories`; only the Assistant has `forget` (on the owner's word); none has `remember`. The rails are code (`lifeos-core/src/life/teachings.rs`).
- **Workflows** (`lifeos-agent/src/workflows.rs`): scripts are `workflows/<name>.js` in the vault or inline. `agent()` asks the Assistant or the member `agentType` on its own card, as a routine does; `askOwner()` is a question; `wait()` takes seconds, minutes, hours or until. `hire`, `accept`, `sendBack` and a nested `workflow()` fail saying why. A step an agent was on when lifeos stopped fails on start. A routine with `workflow` starts that script with `{instructions, trigger, payload}` and its run ends once the workflow started.
- **Session panel** (remit's `ContextDetails` and `CardTelemetry`): what lifeos gave a session as it started it (`session_launches`, with its opening message), Claude Code's newest init (`session_inits`) and its context as counted at rest (`session_contexts`), each the newest of its card, read with `Call::Context`, a live session counting afresh. Sessions export OpenTelemetry (remit's ADR-0085) to the door's `/otlp` with their run's token; records are kept as the token's card in `<vault>/.lifeos/telemetry.db` for 90 days and read summed up with `Call::Telemetry`. remit's capabilities, principles and missing-skill parts are not shown: lifeos has none of them. Nothing is forwarded to Datadog.
- **Images** (remit's ADR-0111): the composer fits an image (long side at most 1568 px, WebP when redrawn), puts it with `Call::ImagePut` under an id the page makes (idempotent; refused over 5 MiB) and names it in the draft (`Draft.images`); `Call::Say` carries the names, kept on the prompt (`PromptData.images`) until its turn or steer reads the bytes into `model::Prompt.images`; a page reads one with `Call::ImageGet`. The bytes are in table `images` of `.lifeos/lifeos.db`, never in the vault's documents, and kept: remit's home does not exist in lifeos, so nothing removes them yet. Telegram sends none.
- **Not built yet:** the Decisions and Memory-book views in the panel (the reads exist: `list_questions`, `list_teachings`, `list_memories`), remit's daily memory review (`consolidate`) and recall measurement.

## Consequences

- `claude-code-agent` and the SDK MCP server go; lifeos's account pool is re-pointed at the launcher.
- `turns` and the old `memories` migrate forward once (turns become events, memories become ADR-0117 memories).
- The wire grows remit's stream views (`EventRead`, `AgentStanding`, `TurnSummary`, `Draft`, `Delivery`) and the panel
  reads them through the Wasm client.

## Revisit when

- remit changes a copied crate: carry the diff over.
- Memory text must stay on the machine: put a local model behind the `MemoryModels` port.
